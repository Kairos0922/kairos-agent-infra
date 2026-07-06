//! check-docs 任务:校验全仓 Markdown 的内部链接与标题锚点。
//!
//! 相较原 Python 脚本的改进:除 `docs/` 外,还覆盖仓库根级 Markdown(README / AGENTS /
//! PROGRESS / CLAUDE),这些文件有大量指向 `docs/` 的相对链接,原脚本漏检。
//!
//! 校验规则(与原脚本一致):
//! - 仅检查相对链接(http/https 开头的外部链接跳过)。
//! - 检查目标文件是否存在。
//! - 检查 `#锚点` 是否匹配目标文件某个标题的 GitHub 风格 slug(支持中文)。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// 遍历时跳过的目录(构建产物、依赖、VCS、工具配置)。
const SKIP_DIRS: &[&str] = &["target", "node_modules", ".git", ".claude", ".cargo"];

pub fn run() -> ExitCode {
    let root = repo_root();
    let mut md_files = Vec::new();
    collect_md(&root, &mut md_files);
    md_files.sort();

    let mut broken: Vec<String> = Vec::new();
    for path in &md_files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let self_headings = headings_of(&text);
        let dir = path.parent().unwrap_or(&root);

        for (link, anchor) in links_of(&text) {
            // 外部链接跳过。
            if link.starts_with("http") {
                continue;
            }

            let target_headings: HashSet<String> = if link.is_empty() {
                // 纯锚点,指向本文件。
                self_headings.clone()
            } else {
                let target = normalize(&dir.join(&link));
                if !target.exists() {
                    broken.push(format!("{}: 目标文件不存在 -> {}", rel(path, &root), link));
                    continue;
                }
                match (&anchor, std::fs::read_to_string(&target)) {
                    (Some(_), Ok(t)) => headings_of(&t),
                    _ => HashSet::new(),
                }
            };

            if let Some(anchor) = &anchor {
                if !target_headings.contains(&slug(anchor)) {
                    broken.push(format!(
                        "{}: 锚点不存在 -> {}#{}",
                        rel(path, &root),
                        link,
                        anchor
                    ));
                }
            }
        }
    }

    if broken.is_empty() {
        println!("ALL LINKS OK");
        ExitCode::SUCCESS
    } else {
        println!("发现断链/失效锚点:");
        for b in &broken {
            println!("  {b}");
        }
        ExitCode::FAILURE
    }
}

/// 仓库根 = xtask crate 的上一级(CARGO_MANIFEST_DIR/..)。
fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map(Path::to_path_buf).unwrap_or(manifest)
}

/// 递归收集所有 .md 文件,跳过 SKIP_DIRS。
fn collect_md(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if SKIP_DIRS.contains(&name) {
                continue;
            }
            collect_md(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
            out.push(path);
        }
    }
}

/// 提取所有 Markdown 标题的 slug 集合(# ~ ######)。
fn headings_of(text: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let hashes = trimmed.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&hashes) {
            let rest = &trimmed[hashes..];
            // 标题需在 # 之后紧跟空白(排除 `#[derive]` 等代码行)。
            if rest.starts_with([' ', '\t']) {
                set.insert(slug(rest.trim()));
            }
        }
    }
    set
}

/// 提取 `](链接)` 中的链接,拆出锚点(百分号解码)。返回 (link_without_anchor, anchor)。
fn links_of(text: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        // 找 `](` 起始。
        if bytes[i] == b']' && bytes[i + 1] == b'(' {
            let start = i + 2;
            if let Some(rel_end) = text[start..].find(')') {
                let raw = text[start..start + rel_end].trim();
                let (link, anchor) = match raw.split_once('#') {
                    Some((l, a)) => (l.to_string(), Some(percent_decode(a))),
                    None => (raw.to_string(), None),
                };
                out.push((link, anchor));
                i = start + rel_end + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// 把标题转成 GitHub 风格锚点 slug(小写、去标点、空格转连字符、保留中文)。
/// 与原 Python 脚本的 slug 规则一致:保留 unicode 字母/数字、下划线、空格、连字符,余皆删除。
fn slug(heading: &str) -> String {
    let lowered = heading.trim().to_lowercase();
    let kept: String = lowered
        .chars()
        .filter(|&c| c.is_alphanumeric() || c == '_' || c == ' ' || c == '-')
        .collect();
    kept.replace(' ', "-")
}

/// 规范化路径:消解 `.` 与 `..`(不触碰文件系统,纯词法)。
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        use std::path::Component::*;
        match comp {
            ParentDir => {
                out.pop();
            }
            CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// 相对仓库根显示路径,便于阅读。
fn rel(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// 最小 UTF-8 百分号解码(%XX → 字节),用于锚点中的转义(如空格 %20、中文)。
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_matches_github_style() {
        assert_eq!(slug("Hello World"), "hello-world");
        assert_eq!(slug("## 配置管理"), "-配置管理"); // 前导 ## 被 trim 前已由调用方去掉;此处仅测标点/空格
        assert_eq!(slug("L0 foundation"), "l0-foundation");
        assert_eq!(slug("机制 vs 策略"), "机制-vs-策略");
        assert_eq!(slug("标题(带括号)"), "标题带括号"); // 括号被删
    }

    #[test]
    fn headings_skip_code_attrs() {
        let text = "# 真标题\n#[derive(Debug)]\n```\n### 代码内\n```\n## 二级\n";
        let hs = headings_of(text);
        assert!(hs.contains("真标题"));
        assert!(hs.contains("二级"));
        // #[derive] 后无空白,不算标题
        assert!(!hs.iter().any(|h| h.contains("derive")));
    }

    #[test]
    fn links_extract_and_split_anchor() {
        let text = "见 [架构](../project/architecture.md#分层总览) 和 [外部](https://x.com)。";
        let links = links_of(text);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].0, "../project/architecture.md");
        assert_eq!(links[0].1.as_deref(), Some("分层总览"));
        assert_eq!(links[1].0, "https://x.com");
    }

    #[test]
    fn normalize_resolves_parent() {
        assert_eq!(
            normalize(Path::new("docs/modules/../project/x.md")),
            PathBuf::from("docs/project/x.md")
        );
    }

    #[test]
    fn percent_decode_roundtrip() {
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("plain"), "plain");
    }
}
