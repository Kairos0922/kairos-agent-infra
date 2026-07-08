//! doc-sync 任务:文档同步一致性校验(收尾「三同步」里可确定性自动化的机械检查部分)。
//!
//! 定位:这是「一致性校验器」,帮你查「该同步的地方漏没漏」,而非「自动帮你写同步内容」。
//! PROGRESS.md 是人写内容、fmt/clippy/test 已由 pre-push 钩子覆盖,均不在此范围。
//!
//! 校验规则:
//! - **ADR 索引一致性**:`docs/adr/` 下的 `NNNN-*.md` 文件集合,必须与 `docs/adr/README.md`
//!   索引表列出的条目一一对应。漏登记(有文件无索引)或悬空(有索引无文件)都报错。
//!   对治复盘里反复出现的「新增 ADR 忘更新索引表」。
//! - **废弃术语残留**:`DEPRECATED_TERMS` 登记「旧标识符 → 新标识符」的改名对,全仓扫描
//!   Markdown 与 Rust 源码,发现旧标识符残留即报错。按标识符边界精确匹配(前后非
//!   字母/数字/下划线),避免通用词误报。初始为空——有真实改名时往表里加一行。
//!
//! 退出码:成功 0;发现不一致 1 并逐条打印。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// 遍历术语残留时跳过的目录(构建产物、依赖、VCS、工具配置)。
const SKIP_DIRS: &[&str] = &["target", "node_modules", ".git", ".claude", ".cargo"];

/// 已废弃标识符 →(替代标识符,仅作提示)。有重大改名时在此登记一行,
/// doc-sync 便会全仓拦截旧标识符残留。初始为空:不预测未来改名,只登记已发生的。
///
/// 注意:只登记**专有的复合标识符**(如 `session_kind`、`SessionMemory`),
/// 不要登记 `session` 这类通用词——会大量误报。
const DEPRECATED_TERMS: &[(&str, &str)] = &[];

pub fn run() -> ExitCode {
    let root = repo_root();
    let mut problems: Vec<String> = Vec::new();

    check_adr_index(&root, &mut problems);
    check_deprecated_terms(&root, &mut problems);

    if problems.is_empty() {
        println!("DOC SYNC OK");
        ExitCode::SUCCESS
    } else {
        println!("发现文档同步问题:");
        for p in &problems {
            println!("  {p}");
        }
        ExitCode::FAILURE
    }
}

/// 校验 ADR 索引表与 `docs/adr/` 实际文件一一对应。
fn check_adr_index(root: &Path, problems: &mut Vec<String>) {
    let adr_dir = root.join("docs/adr");
    let readme = adr_dir.join("README.md");

    let Ok(readme_text) = std::fs::read_to_string(&readme) else {
        problems.push("ADR 索引:无法读取 docs/adr/README.md".to_string());
        return;
    };

    // 索引表登记的文件名集合(从 `| [NNNN](./file.md) | ...` 行解析)。
    let indexed: BTreeSet<String> = readme_text.lines().filter_map(parse_index_link).collect();

    // 目录下实际的 ADR 文件集合(NNNN-*.md,排除 README.md)。
    let mut actual: BTreeSet<String> = BTreeSet::new();
    if let Ok(entries) = std::fs::read_dir(&adr_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_adr_file(&name) {
                actual.insert(name);
            }
        }
    } else {
        problems.push("ADR 索引:无法读取 docs/adr/ 目录".to_string());
        return;
    }

    for missing in actual.difference(&indexed) {
        problems.push(format!(
            "ADR 索引:存在文件 docs/adr/{missing} 但 README 索引表未登记"
        ));
    }
    for dangling in indexed.difference(&actual) {
        problems.push(format!(
            "ADR 索引:README 索引表登记了 {dangling} 但 docs/adr/ 下无此文件"
        ));
    }
}

/// 从索引表一行中提取链接的文件名。匹配形如 `| [0001](./0001-xxx.md) | ...`。
/// 返回不带 `./` 前缀的文件名(如 `0001-xxx.md`);非索引行返回 None。
fn parse_index_link(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    // 索引行以表格分隔符 + 链接起始 `[` 开头。
    let after_bracket = trimmed.strip_prefix("| [")?;
    // 跳过 `NNNN]`,定位到 `](`。
    let paren = after_bracket.find("](")?;
    let link_start = paren + 2;
    let link_end = after_bracket[link_start..].find(')')? + link_start;
    let link = &after_bracket[link_start..link_end];
    // 仅接受指向 .md 的相对链接;剥掉 `./` 前缀,只留文件名。
    let file = link.trim_start_matches("./");
    if file.ends_with(".md") && !file.contains('/') {
        Some(file.to_string())
    } else {
        None
    }
}

/// 判断文件名是否为 ADR 文件(`NNNN-*.md`,四位数字前缀)。
fn is_adr_file(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".md") else {
        return false;
    };
    let mut parts = stem.splitn(2, '-');
    let num = parts.next().unwrap_or("");
    num.len() == 4 && num.chars().all(|c| c.is_ascii_digit()) && parts.next().is_some()
}

/// 全仓扫描废弃标识符残留。
fn check_deprecated_terms(root: &Path, problems: &mut Vec<String>) {
    if DEPRECATED_TERMS.is_empty() {
        return;
    }
    let mut files = Vec::new();
    collect_source(root, &mut files);
    files.sort();

    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for (old, new) in DEPRECATED_TERMS {
            if contains_identifier(&text, old) {
                problems.push(format!(
                    "废弃术语:{} 仍含已废弃标识符 `{old}`(应改为 `{new}`)",
                    rel(path, root)
                ));
            }
        }
    }
}

/// 递归收集 Markdown 与 Rust 源码,跳过 SKIP_DIRS。
fn collect_source(dir: &Path, out: &mut Vec<PathBuf>) {
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
            collect_source(&path, out);
        } else {
            let ext = path.extension().and_then(|s| s.to_str());
            if matches!(ext, Some("md") | Some("rs")) {
                out.push(path);
            }
        }
    }
}

/// 判断 `text` 是否含作为独立标识符出现的 `needle`(前后字符非字母/数字/下划线)。
fn contains_identifier(text: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let bytes = text.as_bytes();
    let nlen = needle.len();
    let mut from = 0;
    while let Some(rel) = text[from..].find(needle) {
        let start = from + rel;
        let end = start + nlen;
        let before_ok = start == 0 || !is_ident_byte(bytes[start - 1]);
        let after_ok = end >= bytes.len() || !is_ident_byte(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

/// 标识符字节:ASCII 字母/数字/下划线。
fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// 仓库根 = xtask crate 的上一级(CARGO_MANIFEST_DIR/..)。
fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map(Path::to_path_buf).unwrap_or(manifest)
}

/// 相对仓库根显示路径,便于阅读。
fn rel(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_index_link_extracts_filename() {
        assert_eq!(
            parse_index_link("| [0001](./0001-vector-store-lancedb.md) | 标题 | 已接受 |"),
            Some("0001-vector-store-lancedb.md".to_string())
        );
        // 带前导空白也可解析。
        assert_eq!(
            parse_index_link("  | [0024](./0024-foo.md) | x | y |"),
            Some("0024-foo.md".to_string())
        );
    }

    #[test]
    fn parse_index_link_ignores_non_index_rows() {
        assert_eq!(parse_index_link("| 编号 | 标题 | 状态 |"), None);
        assert_eq!(parse_index_link("|------|------|------|"), None);
        assert_eq!(parse_index_link("普通段落文字"), None);
        // 外部链接或含子路径的链接不算 ADR 文件。
        assert_eq!(parse_index_link("| [x](https://a.com) | y | z |"), None);
        assert_eq!(parse_index_link("| [x](./sub/dir.md) | y | z |"), None);
    }

    #[test]
    fn is_adr_file_matches_nnnn_prefix() {
        assert!(is_adr_file("0001-vector-store.md"));
        assert!(is_adr_file("0024-x.md"));
        assert!(!is_adr_file("README.md"));
        assert!(!is_adr_file("001-too-short.md"));
        assert!(!is_adr_file("0001.md")); // 无标题段
        assert!(!is_adr_file("abcd-x.md")); // 非数字前缀
    }

    #[test]
    fn contains_identifier_respects_word_boundary() {
        // 独立标识符命中。
        assert!(contains_identifier("let session_kind = 1;", "session_kind"));
        assert!(contains_identifier("见 session_kind 字段", "session_kind"));
        // 作为更长标识符的一部分不命中。
        assert!(!contains_identifier(
            "let session_kind_new = 1;",
            "session_kind"
        ));
        assert!(!contains_identifier("prev_session_kind", "session_kind"));
        // 通用子串在复合词内不误报。
        assert!(!contains_identifier("http_session_id", "session"));
        assert!(contains_identifier("a session here", "session"));
    }

    #[test]
    fn contains_identifier_empty_needle_is_false() {
        assert!(!contains_identifier("anything", ""));
    }
}
