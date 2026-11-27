#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFIER="$ROOT_DIR/scripts/verify-docs.sh"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

make_valid_repo() {
    local d="$1"
    rm -rf "$d"
    mkdir -p "$d/docs/sub" "$d/docs/archive/plans" "$d/resources" "$d/crates/demo"
    touch "$d/resources/Icon.png" "$d/resources/my file (v1).png" "$d/resources/decor.png"
    cat > "$d/README.md" <<'EOF'
# Sample

<img src="resources/Icon.png" width="24" height="24" alt="Sample Icon">
<img src="resources/decor.png" alt="">

[Docs](docs/README.md#section-one)
[Dup](docs/README.md#dup-1)
[Dir](docs/sub#sub-anchor)
[Explicit](docs/README.md#custom-id)
[SpaceParens](resources/my%20file%20(v1).png)

````markdown
```markdown
[literal-broken-in-4-backticks](does-not-exist.md)
````

```mermaid
flowchart LR
    A["Start"] --> B["End"]
```

```mermaid
sequenceDiagram
    participant C as Client
    C->>S: Hello
```

```mermaid
stateDiagram-v2
    [*] --> Idle
    Idle --> Active: go
```
EOF
    cat > "$d/docs/tilde.md" <<'EOF'
# Tilde Fence

~~~bash
[literal-in-tilde](also-not-real.md)
~~~
EOF
    cat > "$d/docs/README.md" <<'EOF'
# Docs Home

## Section One

<a id="custom-id"></a>

### Dup

## Section Two

### Dup

```markdown
[literal-broken-in-code](does-not-exist.md)
```
EOF
    cat > "$d/docs/sub/README.md" <<'EOF'
# Sub Anchor
EOF
    cat > "$d/docs/archive/README.md" <<'EOF'
# Archive
EOF
    cat > "$d/docs/archive/plans/old.md" <<'EOF'
> [!NOTE]
> **Historical Archive Record**
> - **Authoritative Replacement:** [Docs](../../README.md)

# Old Plan

[historical-dead-link](old-missing.md)
EOF
    cat > "$d/crates/demo/README.md" <<'EOF'
# Demo Crate

See [Docs](../../docs/README.md).
EOF
}

expect_fail() {
    local name="$1"
    local dir="$2"
    local pattern="${3:-}"
    local out
    if out="$("$VERIFIER" --root "$dir" 2>&1)"; then
        echo "FAIL: expected validator to reject '$name', but it passed" >&2
        exit 1
    fi
    if [[ -n "$pattern" ]] && ! echo "$out" | grep -qE "$pattern"; then
        echo "FAIL: '$name' failed, but output did not match /$pattern/:" >&2
        echo "$out" >&2
        exit 1
    fi
    echo "PASS (rejected): $name"
}

# 1. Valid positive fixture (HTML, decorative alt, 4-backtick & tilde fences, 3 Mermaid types, URL-encoded/parens, dup heading, dir anchor, explicit anchor, archive)
make_valid_repo "$TMP_DIR/valid"
"$VERIFIER" --root "$TMP_DIR/valid" >/dev/null
echo "PASS (accepted): valid comprehensive fixture"

# 2-11. Original 10 negative cases
make_valid_repo "$TMP_DIR/bad-md"
echo "[broken](missing.md)" >> "$TMP_DIR/bad-md/README.md"
expect_fail "broken markdown link" "$TMP_DIR/bad-md" "missing.md"

make_valid_repo "$TMP_DIR/bad-href"
echo '<a href="docs/missing.md">Missing</a>' >> "$TMP_DIR/bad-href/README.md"
expect_fail "broken HTML href" "$TMP_DIR/bad-href" "docs/missing.md"

make_valid_repo "$TMP_DIR/bad-img"
echo '<img src="resources/missing.png" alt="Missing">' >> "$TMP_DIR/bad-img/README.md"
expect_fail "broken HTML img src" "$TMP_DIR/bad-img" "resources/missing.png"

make_valid_repo "$TMP_DIR/bad-case"
echo '<img src="resources/icon.png" alt="Wrong case">' >> "$TMP_DIR/bad-case/README.md"
expect_fail "incorrect filename case" "$TMP_DIR/bad-case" "case mismatch"

make_valid_repo "$TMP_DIR/bad-anchor"
echo "[bad](#nonexistent-anchor)" >> "$TMP_DIR/bad-anchor/README.md"
expect_fail "invalid internal anchor" "$TMP_DIR/bad-anchor" "Invalid internal anchor"

make_valid_repo "$TMP_DIR/bad-mermaid"
printf '\n```mermaid\ninvalidDiagramType\n    A -> B\n```\n' >> "$TMP_DIR/bad-mermaid/README.md"
expect_fail "invalid Mermaid diagram type" "$TMP_DIR/bad-mermaid" "Unsupported Mermaid"

make_valid_repo "$TMP_DIR/bad-index"
mkdir -p "$TMP_DIR/bad-index/docs/noindex"
touch "$TMP_DIR/bad-index/docs/noindex/page.md"
expect_fail "missing directory index" "$TMP_DIR/bad-index" "Missing directory index"

make_valid_repo "$TMP_DIR/bad-fence"
printf '\n```bash\necho unclosed\n' >> "$TMP_DIR/bad-fence/README.md"
expect_fail "unclosed code fence" "$TMP_DIR/bad-fence" "Unclosed code/Mermaid fence"

make_valid_repo "$TMP_DIR/bad-todo"
echo "TODO: fill this in" >> "$TMP_DIR/bad-todo/README.md"
expect_fail "unresolved placeholder" "$TMP_DIR/bad-todo" "Unresolved placeholder"

make_valid_repo "$TMP_DIR/bad-secret"
SECRET_VAL="-----BEGIN OPENSSH PRIVATE KEY-----"
echo "$SECRET_VAL" >> "$TMP_DIR/bad-secret/README.md"
out_secret="$("$VERIFIER" --root "$TMP_DIR/bad-secret" 2>&1 || true)"
if echo "$out_secret" | grep -q "BEGIN OPENSSH PRIVATE KEY"; then
    echo "FAIL: validator echoed secret value in output" >&2
    exit 1
fi
echo "PASS (rejected without echo): potential secret leakage"

# 12+. Additional §6.3 regression cases
make_valid_repo "$TMP_DIR/bad-mermaid-flow"
printf '\n```mermaid\nflowchart LR\n    A["unterminated\n```\n' >> "$TMP_DIR/bad-mermaid-flow/README.md"
expect_fail "Mermaid valid header + unterminated label" "$TMP_DIR/bad-mermaid-flow" "Unterminated string quote"

make_valid_repo "$TMP_DIR/bad-mermaid-seq"
printf '\n```mermaid\nsequenceDiagram\n    not valid sequence line\n```\n' >> "$TMP_DIR/bad-mermaid-seq/README.md"
expect_fail "Mermaid invalid sequenceDiagram" "$TMP_DIR/bad-mermaid-seq" "Invalid Mermaid sequenceDiagram"

make_valid_repo "$TMP_DIR/bad-mermaid-state"
printf '\n```mermaid\nstateDiagram-v2\n    broken state transition\n```\n' >> "$TMP_DIR/bad-mermaid-state/README.md"
expect_fail "Mermaid invalid stateDiagram" "$TMP_DIR/bad-mermaid-state" "Invalid Mermaid stateDiagram"

make_valid_repo "$TMP_DIR/bad-table"
printf '\n| A | B |\n| --- | --- |\n| 1 |\n' >> "$TMP_DIR/bad-table/README.md"
expect_fail "malformed Markdown table" "$TMP_DIR/bad-table" "Malformed table row"

make_valid_repo "$TMP_DIR/bad-heading"
printf '\n### Skipped H2\n' >> "$TMP_DIR/bad-heading/README.md"
expect_fail "skipped heading level" "$TMP_DIR/bad-heading" "Skipped heading level"

make_valid_repo "$TMP_DIR/bad-ref-img"
printf '\n![alt][missing-img]\n' >> "$TMP_DIR/bad-ref-img/README.md"
expect_fail "undefined reference link/image" "$TMP_DIR/bad-ref-img" "Undefined reference link"

make_valid_repo "$TMP_DIR/bad-html-alt"
printf '\n<img\n  src="resources/Icon.png"\n  width="24">\n' >> "$TMP_DIR/bad-html-alt/README.md"
expect_fail "multiline HTML img missing alt with accurate line" "$TMP_DIR/bad-html-alt" "README.md:[0-9]{2,}:"

make_valid_repo "$TMP_DIR/bad-crate-readme"
echo "[bad](nope.md)" >> "$TMP_DIR/bad-crate-readme/crates/demo/README.md"
expect_fail "broken path in crate README" "$TMP_DIR/bad-crate-readme" "crates/demo/README.md"

make_valid_repo "$TMP_DIR/bad-symlink"
ln -s /etc/hosts "$TMP_DIR/bad-symlink/docs/escape.md"
echo "[escape](docs/escape.md)" >> "$TMP_DIR/bad-symlink/README.md"
expect_fail "symlink escape outside repo root" "$TMP_DIR/bad-symlink" "escapes repository root"

make_valid_repo "$TMP_DIR/bad-archive-banner"
printf '> [!NOTE]\n> **Historical Archive Record**\n> - **Replacement:** [Broken](../../docs/nope.md)\n\n# Old\n' > "$TMP_DIR/bad-archive-banner/docs/archive/plans/old.md"
expect_fail "archive banner with broken replacement link" "$TMP_DIR/bad-archive-banner" "docs/archive/plans/old.md"

echo "All 21 validator regression tests PASSED."
