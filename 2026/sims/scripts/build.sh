#!/usr/bin/env bash
# build.sh — Build all Assign Onward 2026 components
# Works on Linux/macOS/WSL/Git Bash on Windows
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SIMS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT_2026="$(cd "$SIMS_DIR/.." && pwd)"
SRC_DIR="$ROOT_2026/src"

echo "=== Assign Onward — Build All Components ==="
echo "Root: $ROOT_2026"
echo ""

# ── 1. Rust toolchain check ────────────────────────────────────────────
if ! command -v cargo &>/dev/null; then
    echo "ERROR: cargo not found. Install Rust from https://rustup.rs"
    exit 1
fi

RUST_VER=$(rustc --version)
echo "Rust: $RUST_VER"
echo ""

# ── 2. Build all Rust binaries (workspace) ─────────────────────────────
echo "── Building Rust workspace (release mode)..."
cd "$SRC_DIR"
cargo build --release --bin ao-recorder --bin ao-validator --bin ao-exchange --bin ao-relay --bin ao-cli 2>&1

echo ""
echo "── Building ao-sims..."
cd "$SIMS_DIR"
cargo build --release 2>&1

echo ""

# ── 3. Locate built binaries ───────────────────────────────────────────
# Workspace target may be at 2026/src/target or 2026/sims/target
if [[ -f "$SRC_DIR/target/release/ao-recorder" ]] || [[ -f "$SRC_DIR/target/release/ao-recorder.exe" ]]; then
    BIN_DIR="$SRC_DIR/target/release"
elif [[ -f "$ROOT_2026/target/release/ao-recorder" ]] || [[ -f "$ROOT_2026/target/release/ao-recorder.exe" ]]; then
    BIN_DIR="$ROOT_2026/target/release"
else
    echo "WARNING: Could not locate built binaries. Check cargo output above."
    BIN_DIR="$SRC_DIR/target/release"
fi

if [[ -f "$SIMS_DIR/target/release/ao-sims" ]] || [[ -f "$SIMS_DIR/target/release/ao-sims.exe" ]]; then
    SIMS_BIN_DIR="$SIMS_DIR/target/release"
else
    SIMS_BIN_DIR="$BIN_DIR"
fi

echo "Binaries:"
for bin in ao-recorder ao-validator ao-exchange ao-relay ao-cli; do
    for dir in "$BIN_DIR" "$SIMS_BIN_DIR"; do
        for ext in "" ".exe"; do
            if [[ -f "$dir/$bin$ext" ]]; then
                echo "  $bin  →  $dir/$bin$ext"
                break 2
            fi
        done
    done
done
for ext in "" ".exe"; do
    if [[ -f "$SIMS_BIN_DIR/ao-sims$ext" ]]; then
        echo "  ao-sims  →  $SIMS_BIN_DIR/ao-sims$ext"
        break
    fi
done

echo ""

# ── 4. PWA dependencies ───────────────────────────────────────────────
PWA_DIR="$SRC_DIR/ao-pwa"
if [[ -d "$PWA_DIR" ]]; then
    echo "── Installing ao-pwa dependencies..."
    cd "$PWA_DIR"
    if command -v npm &>/dev/null; then
        npm install --silent 2>&1
        echo "  ao-pwa: npm install complete"
    else
        echo "  WARNING: npm not found. Skipping PWA setup."
        echo "  Install Node.js from https://nodejs.org"
    fi
fi

echo ""

# ── 5. Viewer PWA dependencies ────────────────────────────────────────
VIEWER_DIR="$SIMS_DIR/viewer"
if [[ -d "$VIEWER_DIR/package.json" ]] || [[ -f "$VIEWER_DIR/package.json" ]]; then
    echo "── Installing viewer PWA dependencies..."
    cd "$VIEWER_DIR"
    if command -v npm &>/dev/null; then
        npm install --silent 2>&1
        echo "  viewer: npm install complete"
    fi
fi

echo ""
echo "=== Build complete ==="
echo ""
echo "Next steps:"
echo "  Run a simulation:  ./scripts/run-sim.sh minimal"
echo "  Run full stack:    ./scripts/run-stack.sh"
echo "  See the guide:     cat GUIDE.md"
