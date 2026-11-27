#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
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

root = Path(".").resolve()
errors = []

def check_case_exact(path: Path) -> bool:
    """Verify every path component matches exact filesystem case (catches macOS APFS vs Linux)."""
    if not path.exists():
        return False
    try:
        rel = path.resolve().relative_to(root)
    except ValueError:
        return False
    cur = root
    for part in rel.parts:
        entries = os.listdir(cur)
        if part not in entries:
            return False
        cur = cur / part
    return True

def github_slug(heading: str, seen: dict) -> str:
    # Strip inline HTML tags and markdown link syntax
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
    in_code = False
    for line in md_text.splitlines():
        s = line.strip()
        if s.startswith("```") or s.startswith("~~~"):
            in_code = not in_code
            continue
        if in_code:
            continue
        m = re.match(r"^#{1,6}\s+(.*)$", line)
        if m:
            anchors.add(github_slug(m.group(1), seen))
        for attr in re.findall(r'\b(?:id|name)=["\']([^"\']+)["\']', line):
            anchors.add(attr)
    return anchors

class DocHTMLParser(HTMLParser):
    def __init__(self):
        super().__init__()
        self.links = []   # (lineno, attr_type, target, alt)

    def handle_starttag(self, tag, attrs):
        attr_map = dict(attrs)
        lineno, _ = self.getpos()
        if tag == "a" and "href" in attr_map:
            self.links.append((lineno, "href", attr_map["href"], None))
        elif tag == "img":
            src = attr_map.get("src")
            alt = attr_map.get("alt")
            if src is not None:
                self.links.append((lineno, "img", src, alt))

# 1. Check directory README.md indexes under docs/
docs_root = root / "docs"
if docs_root.exists():
    for dirpath, dirnames, filenames in os.walk(docs_root):
        p = Path(dirpath)
        rel = p.relative_to(root)
        if rel.parts[:2] == ("docs", "archive") and len(rel.parts) > 2:
            continue
        if "README.md" not in filenames:
            errors.append(f"Missing directory index README.md in {rel}")

# Collect markdown files
root_doc_names = [
    "README.md", "AGENTS.md", "AGENT.md", "PROJECT.md",
    "CONTRIBUTING.md", "SECURITY.md", "CHANGELOG.md", "SYNC_DESIGN.md",
]
root_docs = [root / name for name in root_doc_names if (root / name).exists()]
docs_md = sorted(docs_root.rglob("*.md")) if docs_root.exists() else []
all_md = root_docs + docs_md

# Pre-compute anchors for all markdown files
anchor_cache = {}
for md_file in all_md:
    anchor_cache[md_file.resolve()] = extract_anchors(md_file.read_text(encoding="utf-8"))

inline_link_re = re.compile(r'(!?)\[([^\]]*)\]\(([^)\s]+)(?:\s+"[^"]*")?\)')
ref_def_re = re.compile(r'^\s*\[([^\]]+)\]:\s*(\S+)')
ref_use_re = re.compile(r'(?<!!)\[([^\]]+)\]\[([^\]]*)\]')
placeholder_re = re.compile(r"\b(?:TODO|TBD|FIXME)\b")

secret_patterns = [
    ("private key block", re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----")),
    ("GitHub token", re.compile(r"\b(?:ghp|github_pat)_[A-Za-z0-9_]{20,}\b")),
    ("AWS access key", re.compile(r"\bAKIA[0-9A-Z]{16}\b")),
    ("Google API key", re.compile(r"\bAIza[0-9A-Za-z_-]{35}\b")),
    ("Bearer token", re.compile(r"Authorization:\s*Bearer\s+[A-Za-z0-9._-]{20,}")),
]

allowed_mermaid_types = {
    "flowchart", "graph", "sequencediagram", "statediagram", "statediagram-v2",
    "classdiagram", "erdiagram", "gantt", "pie", "gitgraph", "mindmap",
    "timeline", "xychart-beta",
}

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
    resolved = (src_file.parent / path_part).resolve()
    try:
        resolved.relative_to(root)
    except ValueError:
        errors.append(f"Path escapes repository root in {rel_src}:{lineno}: {target}")
        return

    if not check_case_exact(resolved):
        errors.append(f"Broken or case-mismatched path in {rel_src}:{lineno}: {target}")
        return

    if is_image and "/hover/" in path_part.replace("\\", "/"):
        errors.append(f"Static doc references hover icon variant in {rel_src}:{lineno}: {target}")

    if frag and resolved.suffix == ".md":
        if frag not in anchor_cache.get(resolved, set()):
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

    # Code fence & Mermaid checks
    in_fence = False
    fence_lang = ""
    fence_start = 0
    mermaid_lines = []
    for idx, line in enumerate(lines, start=1):
        stripped = line.strip()
        if stripped.startswith("```") or stripped.startswith("~~~"):
            if not in_fence:
                in_fence = True
                fence_lang = stripped[3:].strip().lower()
                fence_start = idx
                mermaid_lines = []
            else:
                if fence_lang == "mermaid":
                    non_empty = [
                        l.strip() for l in mermaid_lines
                        if l.strip() and not l.strip().startswith("%%")
                    ]
                    if not non_empty:
                        errors.append(f"Empty Mermaid block at {rel_file}:{fence_start}")
                    else:
                        first_tok = non_empty[0].split()[0].lower()
                        if first_tok not in allowed_mermaid_types:
                            errors.append(f"Unsupported/invalid Mermaid diagram type '{non_empty[0]}' at {rel_file}:{fence_start}")
                        subgraphs = sum(1 for l in non_empty if re.match(r"^subgraph\b", l))
                        ends = sum(1 for l in non_empty if l == "end")
                        if subgraphs != ends:
                            errors.append(f"Unbalanced Mermaid subgraph/end ({subgraphs} vs {ends}) at {rel_file}:{fence_start}")
                in_fence = False
                fence_lang = ""
        elif in_fence and fence_lang == "mermaid":
            mermaid_lines.append(line)

    if in_fence:
        errors.append(f"Unclosed code/Mermaid fence starting at {rel_file}:{fence_start}")

    # Archive banner check for archived docs (inside docs/archive/<category>/*.md)
    if is_archive and len(rel_file.parts) > 3:
        header_block = "\n".join(lines[:10])
        if "Historical Archive Record" not in header_block:
            errors.append(f"Archived document missing Historical Archive Record banner: {rel_file}")
        # Validate links inside the banner only (blockquote lines at top of file)
        for idx, line in enumerate(lines[:10], start=1):
            if not line.lstrip().startswith(">"):
                break
            for bang, alt, target in inline_link_re.findall(line):
                validate_target(md_file, idx, target, is_image=(bang == "!"), alt_text=alt)
        continue
    if is_archive:
        continue

    if is_changelog:
        continue

    # Active doc checks: strip fenced code blocks
    non_code_lines = []
    in_code = False
    for idx, line in enumerate(lines, start=1):
        s = line.strip()
        if s.startswith("```") or s.startswith("~~~"):
            in_code = not in_code
            continue
        if not in_code:
            non_code_lines.append((idx, line))

    # Reference definitions
    ref_defs = {}
    for idx, line in non_code_lines:
        m = ref_def_re.match(line)
        if m:
            ref_id = m.group(1).lower()
            ref_target = m.group(2)
            ref_defs[ref_id] = (idx, ref_target)
            validate_target(md_file, idx, ref_target, is_image=False)

    # Inline links, reference uses, placeholders
    for idx, line in non_code_lines:
        if placeholder_re.search(line) and "verify-docs" not in line and "unresolved" not in line.lower():
            errors.append(f"Unresolved placeholder in {rel_file}:{idx}: {line.strip()}")

        # Remove inline code spans before scanning markdown links
        cleaned = re.sub(r"`[^`]*`", "", line)
        for bang, alt, target in inline_link_re.findall(cleaned):
            validate_target(md_file, idx, target, is_image=(bang == "!"), alt_text=alt)
        for text_part, ref_id in ref_use_re.findall(cleaned):
            key = (ref_id or text_part).lower()
            if key not in ref_defs and not re.match(r"^[xX ]$", text_part):
                errors.append(f"Broken reference link [{text_part}][{ref_id}] in {rel_file}:{idx}")

    # HTML links and images outside code blocks
    html_text = "\n".join(line for _, line in non_code_lines)
    parser = DocHTMLParser()
    try:
        parser.feed(html_text)
    except Exception as exc:
        errors.append(f"HTML parse error in {rel_file}: {exc}")
    for _, kind, target, alt in parser.links:
        validate_target(md_file, 1, target, is_image=(kind == "img"), alt_text=alt)

if errors:
    print("Documentation verification FAILED:", file=sys.stderr)
    for err in errors:
        print(f"  - {err}", file=sys.stderr)
    sys.exit(1)

print(f"Documentation verification PASSED ({len(all_md)} markdown files checked).")
PY
