#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
if [[ "${1:-}" == "--root" && -n "${2:-}" ]]; then
    ROOT_DIR="$(cd "$2" && pwd)"
fi

cd "$ROOT_DIR"

python3 - <<'PY'
import os
import re
import sys
import urllib.parse
from html.parser import HTMLParser
from pathlib import Path
from markdown_it import MarkdownIt

root = Path(".").resolve()
errors = []

md_parser = MarkdownIt("commonmark").enable(["table", "strikethrough"])

def check_case_and_escape(path: Path) -> tuple[bool, str]:
    """Verify path stays inside repo root (even via symlinks) and matches exact disk case."""
    if not path.exists():
        return False, "missing"
    try:
        real_path = path.resolve(strict=True)
        real_path.relative_to(root)
    except (ValueError, OSError):
        return False, "escapes repository root"

    # Walk unresolved normalized path components inside root for exact case check
    try:
        rel = Path(os.path.normpath(str(path))).relative_to(root)
    except ValueError:
        return False, "escapes repository root"
    cur = root
    for part in rel.parts:
        entries = os.listdir(cur)
        if part not in entries:
            return False, "case mismatch"
        cur = cur / part
    return True, "ok"

def github_slug(heading: str, seen: dict) -> str:
    text = re.sub(r"<[^>]+>", "", heading)
    text = re.sub(r"!\[([^\]]*)\]\([^)]+\)", r"\1", text)
    text = re.sub(r"\[([^\]]+)\]\([^)]+\)", r"\1", text)
    text = text.strip().lower()
    slug = []
    for ch in text:
        if ch.isalnum() or ch in ("-", "_"):
            slug.append(ch)
        elif ch == " ":
            slug.append("-")
    base = "".join(slug)
    count = seen.get(base, 0)
    seen[base] = count + 1
    return base if count == 0 else f"{base}-{count}"

def extract_anchors(md_text: str) -> set:
    anchors = set()
    seen = {}
    tokens = md_parser.parse(md_text)
    for i, tok in enumerate(tokens):
        if tok.type == "heading_open" and i + 1 < len(tokens):
            inline_tok = tokens[i + 1]
            anchors.add(github_slug(inline_tok.content, seen))
        elif tok.type in ("html_block", "html_inline"):
            for attr in re.findall(r'\b(?:id|name)=["\']([^"\']+)["\']', tok.content):
                anchors.add(attr)
        elif tok.type == "inline" and tok.children:
            for child in tok.children:
                if child.type == "html_inline":
                    for attr in re.findall(r'\b(?:id|name)=["\']([^"\']+)["\']', child.content):
                        anchors.add(attr)
    return anchors

class DocHTMLParser(HTMLParser):
    def __init__(self, base_line: int):
        super().__init__()
        self.base_line = base_line
        self.links = []  # (abs_line, kind, target, alt)

    def handle_starttag(self, tag, attrs):
        attr_map = dict(attrs)
        rel_line, _ = self.getpos()
        abs_line = self.base_line + rel_line - 1
        if tag == "a" and "href" in attr_map:
            self.links.append((abs_line, "href", attr_map["href"], None))
        elif tag == "img":
            src = attr_map.get("src")
            alt = attr_map.get("alt")
            if src is not None:
                self.links.append((abs_line, "img", src, alt))

def validate_mermaid_grammar(rel_file: Path, fence_line: int, content: str):
    lines = [l.strip() for l in content.splitlines() if l.strip() and not l.strip().startswith("%%")]
    if not lines:
        errors.append(f"Empty Mermaid diagram at {rel_file}:{fence_line}")
        return
    header = lines[0]
    first_tok = header.split()[0].lower()
    allowed = {
        "flowchart", "graph", "sequencediagram", "statediagram", "statediagram-v2",
        "classdiagram", "erdiagram", "gantt", "pie", "gitgraph", "mindmap",
        "timeline", "xychart-beta",
    }
    if first_tok not in allowed:
        errors.append(f"Unsupported Mermaid diagram type '{header}' at {rel_file}:{fence_line}")
        return

    body = lines[1:]
    if not body:
        errors.append(f"Mermaid diagram has header but empty body at {rel_file}:{fence_line}")
        return

    # Balanced quotes and brackets check across body lines
    for offset, l in enumerate(body, start=1):
        if l.count('"') % 2 != 0:
            errors.append(f"Unterminated string quote in Mermaid diagram at {rel_file}:{fence_line + offset}")
        # Check balanced brackets on flowchart node definitions
        if first_tok in ("flowchart", "graph"):
            # Strip quoted labels before bracket counting
            stripped = re.sub(r'"[^"]*"', '""', l)
            if stripped.count("[") != stripped.count("]") or stripped.count("(") != stripped.count(")"):
                errors.append(f"Unbalanced brackets/parens in Mermaid flowchart at {rel_file}:{fence_line + offset}")

    if first_tok in ("flowchart", "graph"):
        subgraphs = sum(1 for l in body if re.match(r"^subgraph\b", l))
        ends = sum(1 for l in body if l == "end")
        if subgraphs != ends:
            errors.append(f"Unbalanced Mermaid subgraph/end ({subgraphs} vs {ends}) at {rel_file}:{fence_line}")
    elif first_tok == "sequencediagram":
        valid_seq_re = re.compile(
            r"^(?:participant\b|actor\b|activate\b|deactivate\b|note\b|loop\b|alt\b|else\b|opt\b|par\b|and\b|critical\b|option\b|break\b|rect\b|end\b|autonumber\b|box\b|link\b|links\b|properties\b|[A-Za-z0-9_.\"-]+\s*(?:->>|-->>|->|-->|-x|--x|-\)|--\))\s*[A-Za-z0-9_.\"-]+\s*:)"
        )
        for offset, l in enumerate(body, start=1):
            if not valid_seq_re.match(l):
                errors.append(f"Invalid Mermaid sequenceDiagram syntax at {rel_file}:{fence_line + offset}: {l}")
    elif first_tok in ("statediagram", "statediagram-v2"):
        valid_state_re = re.compile(
            r"^(?:state\b|note\b|end\s+note\b|direction\b|--|accTitle|accDescr|\}|(?:\[\*\]|[A-Za-z0-9_.-]+)\s*-->\s*(?:\[\*\]|[A-Za-z0-9_.-]+)(?:\s*:.*)?$|[A-Za-z0-9_.-]+\s*:\s*.+$)"
        )
        for offset, l in enumerate(body, start=1):
            if not valid_state_re.match(l):
                errors.append(f"Invalid Mermaid stateDiagram syntax at {rel_file}:{fence_line + offset}: {l}")

# 1. Directory README.md indexes under docs/
docs_root = root / "docs"
if docs_root.exists():
    for dirpath, _, filenames in os.walk(docs_root):
        p = Path(dirpath)
        rel = p.relative_to(root)
        if rel.parts[:2] == ("docs", "archive") and len(rel.parts) > 2:
            continue
        if "README.md" not in filenames:
            errors.append(f"Missing directory index README.md in {rel}")

# Collect markdown files: root docs + docs/ + project-owned crates/*/README.md & crates/*/src/**/README.md
root_doc_names = [
    "README.md", "AGENTS.md", "AGENT.md", "PROJECT.md",
    "CONTRIBUTING.md", "SECURITY.md", "CHANGELOG.md", "SYNC_DESIGN.md",
]
root_docs = [root / name for name in root_doc_names if (root / name).exists()]
docs_md = sorted(docs_root.rglob("*.md")) if docs_root.exists() else []
crates_md = sorted((root / "crates").rglob("*.md")) if (root / "crates").exists() else []
all_md = root_docs + docs_md + crates_md

anchor_cache = {md.resolve(): extract_anchors(md.read_text(encoding="utf-8")) for md in all_md}

placeholder_re = re.compile(r"\b(?:TODO|TBD|FIXME)\b")
secret_patterns = [
    ("private key block", re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----")),
    ("GitHub token", re.compile(r"\b(?:ghp|github_pat)_[A-Za-z0-9_]{20,}\b")),
    ("AWS access key", re.compile(r"\bAKIA[0-9A-Z]{16}\b")),
    ("Google API key", re.compile(r"\bAIza[0-9A-Za-z_-]{35}\b")),
    ("Bearer token", re.compile(r"Authorization:\s*Bearer\s+[A-Za-z0-9._-]{20,}")),
]

def validate_target(src_file: Path, lineno: int, target: str, is_image: bool, alt_text=None):
    rel_src = src_file.relative_to(root)
    if target.startswith(("http://", "https://", "mailto:")):
        return
    if is_image and alt_text is None:
        errors.append(f"Image missing alt attribute in {rel_src}:{lineno}: {target}")
    if target.startswith("#"):
        frag = urllib.parse.unquote(target[1:])
        if frag and frag not in anchor_cache.get(src_file.resolve(), set()):
            errors.append(f"Invalid internal anchor in {rel_src}:{lineno}: {target}")
        return

    path_part, _, frag = target.partition("#")
    path_part = urllib.parse.unquote(path_part.split("?", 1)[0])
    if not path_part:
        return

    candidate = src_file.parent / path_part
    ok, reason = check_case_and_escape(candidate)
    if not ok:
        errors.append(f"Invalid target ({reason}) in {rel_src}:{lineno}: {target}")
        return

    resolved = candidate.resolve()
    if is_image and "/hover/" in path_part.replace("\\", "/"):
        errors.append(f"Static doc references hover icon variant in {rel_src}:{lineno}: {target}")

    if resolved.is_dir():
        readme = resolved / "README.md"
        if frag and readme.exists():
            if urllib.parse.unquote(frag) not in anchor_cache.get(readme.resolve(), set()):
                errors.append(f"Invalid directory README anchor in {rel_src}:{lineno}: {target}")
        return

    if frag and resolved.suffix == ".md":
        if urllib.parse.unquote(frag) not in anchor_cache.get(resolved, set()):
            errors.append(f"Invalid cross-document anchor in {rel_src}:{lineno}: {target}")

for md_file in all_md:
    rel_file = md_file.relative_to(root)
    is_archive = rel_file.parts[:2] == ("docs", "archive")
    is_changelog = rel_file.name == "CHANGELOG.md"
    text = md_file.read_text(encoding="utf-8")
    lines = text.splitlines()

    # Secret checks (never print matched secret!)
    for idx, line in enumerate(lines, start=1):
        if "Redacted::new(" in line:
            continue
        for label, pat in secret_patterns:
            if pat.search(line):
                errors.append(f"Potential secret ({label}) detected in {rel_file}:{idx}")

    # Check unclosed fences accurately (respecting fence char and length)
    in_fence = False
    fence_char = ""
    fence_len = 0
    fence_start = 0
    for idx, line in enumerate(lines, start=1):
        s = line.strip()
        if not in_fence:
            m = re.match(r"^(`{3,}|~{3,})", s)
            if m:
                in_fence = True
                fence_char = m.group(1)[0]
                fence_len = len(m.group(1))
                fence_start = idx
        else:
            if re.match(rf"^{re.escape(fence_char)}{{{fence_len},}}\s*$", s):
                in_fence = False
    if in_fence:
        errors.append(f"Unclosed code/Mermaid fence starting at {rel_file}:{fence_start}")

    # Archive banner validation (stop at first non-blockquote line after Historical Archive Record)
    if is_archive and len(rel_file.parts) > 3:
        top_lines = lines[:15]
        if "Historical Archive Record" not in "\n".join(top_lines):
            errors.append(f"Archived document missing Historical Archive Record banner: {rel_file}")
        else:
            in_banner = False
            for idx, line in enumerate(top_lines, start=1):
                if line.lstrip().startswith(">"):
                    in_banner = True
                    for m in re.finditer(r'(!?)\[([^\]]*)\]\(([^)\s]+)\)', line):
                        validate_target(md_file, idx, m.group(3), is_image=(m.group(1) == "!"), alt_text=m.group(2))
                elif in_banner:
                    break
        continue
    if is_archive or is_changelog:
        continue

    # Parse with markdown-it-py AST
    env = {}
    tokens = md_parser.parse(text, env)
    ref_defs = set((env.get("references") or {}).keys())

    prev_heading_level = 0
    table_header_cols = None
    in_table_body = False
    cur_row_cols = 0

    for idx, tok in enumerate(tokens):
        line_no = (tok.map[0] + 1) if tok.map else 1

        # Heading hierarchy check
        if tok.type == "heading_open":
            level = int(tok.tag[1:])
            if prev_heading_level > 0 and level > prev_heading_level + 1:
                errors.append(f"Skipped heading level (h{prev_heading_level} -> h{level}) in {rel_file}:{line_no}")
            prev_heading_level = level

        # Table column consistency check (raw source rows, since GFM parser pads short rows)
        elif tok.type == "table_open" and tok.map:
            t_lines = lines[tok.map[0]:tok.map[1]]
            if len(t_lines) >= 2:
                def count_cols(row_str: str) -> int:
                    s = row_str.strip()
                    if s.startswith("|"):
                        s = s[1:]
                    if s.endswith("|"):
                        s = s[:-1]
                    # Count unescaped pipes
                    return len(re.split(r"(?<!\\)\|", s))
                hdr_cols = count_cols(t_lines[0])
                for r_off, r_line in enumerate(t_lines[1:], start=1):
                    c = count_cols(r_line)
                    if c != hdr_cols:
                        errors.append(f"Malformed table row ({c} cols vs {hdr_cols} header cols) in {rel_file}:{tok.map[0] + 1 + r_off}")

        # Mermaid fence validation
        elif tok.type == "fence" and tok.info.strip().lower().startswith("mermaid"):
            validate_mermaid_grammar(rel_file, line_no, tok.content)

        # HTML blocks
        elif tok.type == "html_block":
            parser = DocHTMLParser(line_no)
            parser.feed(tok.content)
            for abs_l, kind, target, alt in parser.links:
                validate_target(md_file, abs_l, target, is_image=(kind == "img"), alt_text=alt)

        # Inline tokens (links, images, inline HTML, undefined refs, placeholders)
        elif tok.type == "inline" and tok.children:
            if placeholder_re.search(tok.content) and "verify-docs" not in tok.content and "unresolved" not in tok.content.lower():
                errors.append(f"Unresolved placeholder in {rel_file}:{line_no}: {tok.content.strip()}")

            for child in tok.children:
                if child.type == "link_open":
                    href = child.attrGet("href")
                    if href:
                        validate_target(md_file, line_no, href, is_image=False)
                elif child.type == "image":
                    src = child.attrGet("src")
                    alt = child.content
                    if src:
                        validate_target(md_file, line_no, src, is_image=True, alt_text=alt)
                elif child.type == "html_inline":
                    parser = DocHTMLParser(line_no)
                    parser.feed(child.content)
                    for abs_l, kind, target, alt in parser.links:
                        validate_target(md_file, abs_l, target, is_image=(kind == "img"), alt_text=alt)
                elif child.type == "text":
                    for m in re.finditer(r'\[([^\]]+)\]\[([^\]]*)\]', child.content):
                        ref_key = (m.group(2) or m.group(1)).strip().upper()
                        if ref_key not in ref_defs and not re.match(r"^[xX ]$", m.group(1)):
                            errors.append(f"Undefined reference link [{m.group(1)}][{m.group(2)}] in {rel_file}:{line_no}")

# Environment-variable drift check (only on main repo when crates/ and env doc exist)
env_doc = root / "docs/reference/environment-variables.md"
crates_dir = root / "crates"
if env_doc.exists() and crates_dir.exists():
    env_doc_text = env_doc.read_text(encoding="utf-8")
    test_prefixes = ("SHELLUMI_TEST_", "SHELLUMI_CRASH_", "SHELLUMI_RECOVERY_", "SHELLUMI_EXPECT_", "SHELLUMI_SSH_TEST_")
    for rs_file in crates_dir.rglob("*.rs"):
        if "tests" in rs_file.parts:
            continue
        rs_text = rs_file.read_text(encoding="utf-8")
        for var in set(re.findall(r'env::var(?:_os)?\(\s*"(SHELLUMI_[A-Z0-9_]+)"', rs_text)):
            if var.startswith(test_prefixes):
                continue
            if var not in env_doc_text:
                errors.append(f"Undocumented environment variable {var} (found in {rs_file.relative_to(root)}) missing from docs/reference/environment-variables.md")

if errors:
    print("Documentation verification FAILED:", file=sys.stderr)
    for err in errors:
        print(f"  - {err}", file=sys.stderr)
    sys.exit(1)

print(f"Documentation verification PASSED ({len(all_md)} markdown files checked).")
PY

# Run pinned Node markdownlint + mermaid.parse() when installed (e.g. in CI)
if [[ -d "$SCRIPT_DIR/docs-tools/node_modules" ]]; then
    node "$SCRIPT_DIR/docs-tools/lint-and-mermaid.mjs" "$ROOT_DIR"
fi
