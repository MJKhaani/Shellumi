#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

python3 - <<'PY'
import os
import re
import sys
from pathlib import Path

root = Path(".").resolve()
errors = []

# 1. Check that every directory in docs/ (excluding archive subdirs) has README.md
docs_root = root / "docs"
for dirpath, dirnames, filenames in os.walk(docs_root):
    p = Path(dirpath)
    rel = p.relative_to(root)
    if rel.parts[:2] == ("docs", "archive") and len(rel.parts) > 2:
        continue
    if "README.md" not in filenames:
        errors.append(f"Missing directory index README.md in {rel}")

# Collect active markdown files and all markdown files
root_docs = [
    root / name
    for name in [
        "README.md",
        "AGENTS.md",
        "AGENT.md",
        "PROJECT.md",
        "CONTRIBUTING.md",
        "SECURITY.md",
        "CHANGELOG.md",
    ]
    if (root / name).exists()
]
docs_md = sorted(docs_root.rglob("*.md"))
all_md = root_docs + docs_md

link_re = re.compile(r'(!?\[[^\]]*\]\(([^)]+)\))')
secret_patterns = [
    re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
    re.compile(r"\bghp_[A-Za-z0-9]{20,}\b"),
    re.compile(r"\bAIza[0-9A-Za-z_-]{35}\b"),
]
placeholder_re = re.compile(r"\b(?:TODO|TBD|FIXME)\b")

for md_file in all_md:
    rel_file = md_file.relative_to(root)
    is_archive = rel_file.parts[:2] == ("docs", "archive")
    is_changelog = rel_file.name == "CHANGELOG.md"
    text = md_file.read_text(encoding="utf-8")

    # 2. Check secrets across all docs (skip illustrative Redacted::new examples)
    for idx, line in enumerate(text.splitlines(), start=1):
        if "Redacted::new(" in line:
            continue
        for pat in secret_patterns:
            if pat.search(line):
                errors.append(f"Potential secret pattern ({pat.pattern}) found in {rel_file}:{idx}")

    # 3. Check balanced code fences / Mermaid blocks
    in_fence = False
    fence_start_line = 0
    for idx, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        if stripped.startswith("```"):
            if not in_fence:
                in_fence = True
                fence_start_line = idx
            else:
                in_fence = False
    if in_fence:
        errors.append(f"Unclosed code/Mermaid fence starting at {rel_file}:{fence_start_line}")

    if is_archive or is_changelog:
        continue

    # 4. Check unresolved placeholders in active docs (skip code blocks)
    in_code = False
    for idx, line in enumerate(text.splitlines(), start=1):
        if line.strip().startswith("```"):
            in_code = not in_code
            continue
        if in_code:
            continue
        # Ignore AGENTS.md/testing rule mentions if phrased as rule; check standalone markers
        if placeholder_re.search(line) and "verify-docs" not in line and "unresolved" not in line.lower():
            errors.append(f"Unresolved placeholder in {rel_file}:{idx}: {line.strip()}")

    # 5. Check relative links and images in active docs (skip fenced code blocks)
    in_code = False
    for idx, line in enumerate(text.splitlines(), start=1):
        if line.strip().startswith("```"):
            in_code = not in_code
            continue
        if in_code:
            continue
        for _, target in link_re.findall(line):
            target = target.strip()
            if not target or target.startswith("#"):
                continue
            if re.match(r"^[a-zA-Z][a-zA-Z0-9+.-]*://", target) or target.startswith("mailto:"):
                continue
            path_part = target.split("#", 1)[0].split("?", 1)[0]
            if not path_part:
                continue
            resolved = (md_file.parent / path_part).resolve()
            if not resolved.exists():
                errors.append(f"Broken relative link in {rel_file}:{idx}: {target} -> {resolved}")

if errors:
    print("Documentation verification FAILED:", file=sys.stderr)
    for err in errors:
        print(f"  - {err}", file=sys.stderr)
    sys.exit(1)

print(f"Documentation verification PASSED ({len(all_md)} markdown files checked).")
PY
