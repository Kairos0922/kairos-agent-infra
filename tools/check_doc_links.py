#!/usr/bin/env python3
"""校验 docs/ 下所有 Markdown 的内部链接与标题锚点是否有效。

用途:文档改动收尾时运行,防止断链、失效锚点、引用错位。
用法:python3 tools/check_doc_links.py
退出码:全部有效返回 0;存在断链/失效锚点返回 1 并逐条打印。

校验范围:
- 仅检查相对链接(以 http/https 开头的外部链接跳过)。
- 检查目标文件是否存在。
- 检查 `#锚点` 是否匹配目标文件的某个标题(slug 规则支持中文)。
"""

from __future__ import annotations

import os
import re
import sys
import urllib.parse

DOCS_ROOT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "docs")

# 提取 Markdown 标题(# ~ ######)
HEADING_RE = re.compile(r"^#{1,6}\s+(.*)$", re.M)
# 提取 [文本](链接) 中的链接
LINK_RE = re.compile(r"\]\(([^)]+)\)")
# slug 化时保留的字符:单词字符、中文、空格、连字符
SLUG_KEEP_RE = re.compile(r"[^\w一-鿿 \-]")


def slug(heading: str) -> str:
    """把标题转成 GitHub 风格锚点 slug(小写、去标点、空格转连字符)。"""
    s = heading.strip().lower()
    s = SLUG_KEEP_RE.sub("", s)
    return s.replace(" ", "-")


def headings_of(text: str) -> set[str]:
    return {slug(m.group(1)) for m in HEADING_RE.finditer(text)}


def check() -> list[str]:
    broken: list[str] = []
    for dirpath, _, filenames in os.walk(DOCS_ROOT):
        for fn in filenames:
            if not fn.endswith(".md"):
                continue
            path = os.path.join(dirpath, fn)
            with open(path, encoding="utf-8") as f:
                text = f.read()
            self_headings = headings_of(text)

            for m in LINK_RE.finditer(text):
                link = m.group(1).strip()
                if link.startswith("http"):
                    continue

                anchor = None
                if "#" in link:
                    link, anchor = link.split("#", 1)
                    anchor = urllib.parse.unquote(anchor)

                if link == "":
                    # 纯锚点,指向本文件
                    target_headings = self_headings
                else:
                    target = os.path.normpath(os.path.join(dirpath, link))
                    if not os.path.exists(target):
                        broken.append(f"{path}: 目标文件不存在 -> {link}")
                        continue
                    if anchor:
                        with open(target, encoding="utf-8") as tf:
                            target_headings = headings_of(tf.read())
                    else:
                        target_headings = set()

                if anchor and slug(anchor) not in target_headings:
                    broken.append(f"{path}: 锚点不存在 -> {link}#{anchor}")
    return broken


def main() -> int:
    broken = check()
    if broken:
        print("发现断链/失效锚点:")
        for b in broken:
            print(" ", b)
        return 1
    print("ALL LINKS OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
