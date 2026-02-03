#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
DEVSPACE="$PROJECT_DIR/target/debug/devspace"
TEST_NAME="test-init-output-$$"
CONTAINER_NAME="devspace-$TEST_NAME"
TMPDIR=""
PASSED=0
FAILED=0

cleanup() {
    if [ -n "$TMPDIR" ] && [ -d "$TMPDIR" ]; then
        cd "$TMPDIR"
        "$DEVSPACE" remove 2>/dev/null || true
        cd /
        rm -rf "$TMPDIR"
    fi
}
trap cleanup EXIT

fail() {
    echo "FAIL: $1"
    FAILED=$((FAILED + 1))
}

pass() {
    echo "PASS: $1"
    PASSED=$((PASSED + 1))
}

# Build devspace
echo "Building devspace..."
cargo build --manifest-path "$PROJECT_DIR/Cargo.toml"

# Create temp directory with config
TMPDIR="$(mktemp -d)"
cat > "$TMPDIR/.devspace.toml" <<'TOMLEOF'
name = "PLACEHOLDER"
image = "alpine:latest"
init = """
echo INIT_LINE_ONE
echo INIT_LINE_TWO
MULTI="hello \
world"
echo "INIT_LINE_THREE $MULTI"
"""
shell = "true"
TOMLEOF
sed -i "s/^name = .*/name = \"$TEST_NAME\"/" "$TMPDIR/.devspace.toml"

cd "$TMPDIR"
OUTPUT_FILE="$TMPDIR/output.txt"

# Note: We use `script` to provide a PTY, since docker containers created with
# -it require a TTY for output to be captured. Without a PTY, docker start -ai
# does not forward the container's stdout.

# ---------- Test 1: quiet mode (no -v) ----------
echo ""
echo "=== Test: quiet mode ==="

"$DEVSPACE" remove 2>/dev/null || true

script -qec "$DEVSPACE enter" "$OUTPUT_FILE" >/dev/null 2>&1 || true

if grep -q "INIT_LINE_ONE" "$OUTPUT_FILE"; then
    fail "quiet mode: init output appeared in stdout"
else
    pass "quiet mode: init output suppressed"
fi

if grep -q "INIT_LINE_TWO" "$OUTPUT_FILE"; then
    fail "quiet mode: second init line appeared in stdout"
else
    pass "quiet mode: second init line suppressed"
fi

"$DEVSPACE" remove 2>/dev/null || true

# ---------- Test 2: verbose mode (-v) ----------
echo ""
echo "=== Test: verbose mode ==="

script -qec "$DEVSPACE enter -v" "$OUTPUT_FILE" >/dev/null 2>&1 || true

if grep -q "INIT_LINE_ONE" "$OUTPUT_FILE"; then
    pass "verbose mode: first init line visible"
else
    fail "verbose mode: first init line missing"
fi

if grep -q "INIT_LINE_TWO" "$OUTPUT_FILE"; then
    pass "verbose mode: second init line visible"
else
    fail "verbose mode: second init line missing"
fi

if grep -q "INIT_LINE_THREE" "$OUTPUT_FILE"; then
    pass "verbose mode: third init line visible"
else
    fail "verbose mode: third init line missing"
fi

"$DEVSPACE" remove 2>/dev/null || true

# ---------- Test 3: backslash continuations ----------
echo ""
echo "=== Test: backslash continuations ==="

cat > "$TMPDIR/.devspace.toml" <<'TOMLEOF'
name = "PLACEHOLDER"
image = "alpine:latest"
init = """
apk add --update --no-cache \
    less \
    bash
echo BACKSLASH_INIT_OK
"""
shell = "true"
TOMLEOF
sed -i "s/^name = .*/name = \"$TEST_NAME\"/" "$TMPDIR/.devspace.toml"

script -qec "$DEVSPACE enter -v" "$OUTPUT_FILE" >/dev/null 2>&1 || true

if grep -q "BACKSLASH_INIT_OK" "$OUTPUT_FILE"; then
    pass "backslash continuations: init completed successfully"
else
    fail "backslash continuations: init did not complete (syntax error?)"
fi

"$DEVSPACE" remove 2>/dev/null || true

# ---------- Summary ----------
echo ""
echo "=== Results ==="
echo "Passed: $PASSED"
echo "Failed: $FAILED"

if [ "$FAILED" -gt 0 ]; then
    exit 1
fi
