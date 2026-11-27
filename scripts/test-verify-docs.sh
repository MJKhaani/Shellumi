#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFIER="$ROOT_DIR/scripts/verify-docs.sh"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

make_valid_repo() {
    local d="$1"
    rm -rf "$d"
    mkdir -p "$d/docs" "$d/resources"
    touch "$d/resources/Icon.png"
    cat > "$d/README.md" <<'EOF'
# Sample

<img src="resources/Icon.png" width="24" height="24" alt="Sample Icon">

[Docs](docs/README.md#section-one)

```mermaid
flowchart LR
    A["Start"] --> B["End"]
```
EOF
    cat > "$d/docs/README.md" <<'EOF'
# Docs Home

## Section One

```markdown
[literal-broken-in-code](does-not-exist.md)
```
EOF
}

expect_fail() {
    local name="$1"
    local dir="$2"
    if "$VERIFIER" --root "$dir" >/dev/null 2>&1; then
        echo "FAIL: expected validator to reject '$name', but it passed" >&2
        exit 1
    fi
    echo "PASS (rejected): $name"
}

# 1. Valid fixture passes
make_valid_repo "$TMP_DIR/valid"
"$VERIFIER" --root "$TMP_DIR/valid" >/dev/null
echo "PASS (accepted): valid HTML, Mermaid, and code-block fixture"

# 2. Broken Markdown link
make_valid_repo "$TMP_DIR/bad-md"
echo "[broken](missing.md)" >> "$TMP_DIR/bad-md/README.md"
expect_fail "broken markdown link" "$TMP_DIR/bad-md"

# 3. Broken HTML href
make_valid_repo "$TMP_DIR/bad-href"
echo '<a href="docs/missing.md">Missing</a>' >> "$TMP_DIR/bad-href/README.md"
expect_fail "broken HTML href" "$TMP_DIR/bad-href"

# 4. Broken HTML img src
make_valid_repo "$TMP_DIR/bad-img"
echo '<img src="resources/missing.png" alt="Missing">' >> "$TMP_DIR/bad-img/README.md"
expect_fail "broken HTML img src" "$TMP_DIR/bad-img"

# 5. Incorrect filename case
make_valid_repo "$TMP_DIR/bad-case"
echo '<img src="resources/icon.png" alt="Wrong case">' >> "$TMP_DIR/bad-case/README.md"
expect_fail "incorrect filename case" "$TMP_DIR/bad-case"

# 6. Invalid internal anchor
make_valid_repo "$TMP_DIR/bad-anchor"
echo "[bad](#nonexistent-anchor)" >> "$TMP_DIR/bad-anchor/README.md"
expect_fail "invalid internal anchor" "$TMP_DIR/bad-anchor"

# 7. Invalid Mermaid diagram
make_valid_repo "$TMP_DIR/bad-mermaid"
printf '\n```mermaid\ninvalidDiagramType\n    A -> B\n```\n' >> "$TMP_DIR/bad-mermaid/README.md"
expect_fail "invalid Mermaid diagram" "$TMP_DIR/bad-mermaid"

# 8. Missing directory index README.md
make_valid_repo "$TMP_DIR/bad-index"
mkdir -p "$TMP_DIR/bad-index/docs/sub"
touch "$TMP_DIR/bad-index/docs/sub/page.md"
expect_fail "missing directory index" "$TMP_DIR/bad-index"

# 9. Unclosed code fence
make_valid_repo "$TMP_DIR/bad-fence"
printf '\n```bash\necho unclosed\n' >> "$TMP_DIR/bad-fence/README.md"
expect_fail "unclosed code fence" "$TMP_DIR/bad-fence"

# 10. Unresolved placeholder
make_valid_repo "$TMP_DIR/bad-todo"
echo "TODO: fill this in" >> "$TMP_DIR/bad-todo/README.md"
expect_fail "unresolved placeholder" "$TMP_DIR/bad-todo"

# 11. Potential secret leakage
make_valid_repo "$TMP_DIR/bad-secret"
echo "-----BEGIN OPENSSH PRIVATE KEY-----" >> "$TMP_DIR/bad-secret/README.md"
expect_fail "potential secret leakage" "$TMP_DIR/bad-secret"

echo "All 11 validator regression tests PASSED."
