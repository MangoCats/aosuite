#!/usr/bin/env bash
# Build Debian package for Assign Onward simulation suite.
# Usage: ./build-sims.sh [--target TARGET] [--release]
#
# Produces: ao-sims_VERSION_ARCH.deb
# Includes: ao-sims binary, scenario files, pre-built viewer PWA
# Requires: cargo, dpkg-deb, strip, node/npm (for viewer build)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SIMS_DIR="$(cd "$SCRIPT_DIR/../../sims" && pwd)"
OUT_DIR="$SCRIPT_DIR/out"

PROFILE="release"
TARGET=""
CARGO_TARGET_FLAG=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --target)  TARGET="$2"; CARGO_TARGET_FLAG="--target $2"; shift 2 ;;
        --release) PROFILE="release"; shift ;;
        --debug)   PROFILE="debug"; shift ;;
        *)         echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

if [[ -n "$TARGET" ]]; then
    BIN_DIR="$SIMS_DIR/target/$TARGET/$PROFILE"
else
    BIN_DIR="$SIMS_DIR/target/$PROFILE"
fi

ARCH=$(dpkg --print-architecture 2>/dev/null || echo "amd64")
if [[ "$TARGET" == *"aarch64"* ]]; then
    ARCH="arm64"
elif [[ "$TARGET" == *"armv7"* ]]; then
    ARCH="armhf"
fi

VERSION="0.1.0"
PKG_NAME="ao-sims"

# --- Build binary ---
echo "==> Building ao-sims ($PROFILE, ${TARGET:-native})..."
(cd "$SIMS_DIR" && cargo build --$PROFILE $CARGO_TARGET_FLAG)

# --- Build viewer PWA ---
echo "==> Building sims viewer PWA..."
if [[ -d "$SIMS_DIR/viewer/node_modules" ]]; then
    (cd "$SIMS_DIR/viewer" && npm run build)
else
    (cd "$SIMS_DIR/viewer" && npm install && npm run build)
fi

mkdir -p "$OUT_DIR"

# --- Assemble package ---
echo "==> Packaging $PKG_NAME ($VERSION)..."

PKG_DIR="$OUT_DIR/${PKG_NAME}_${VERSION}_${ARCH}"
rm -rf "$PKG_DIR"

# Binary
mkdir -p "$PKG_DIR/usr/bin"
cp "$BIN_DIR/ao-sims" "$PKG_DIR/usr/bin/ao-sims"
strip "$PKG_DIR/usr/bin/ao-sims" 2>/dev/null || true

# Scenario files
mkdir -p "$PKG_DIR/usr/share/ao-sims/scenarios"
cp "$SIMS_DIR/scenarios/"*.toml "$PKG_DIR/usr/share/ao-sims/scenarios/"

# Viewer PWA (pre-built static files)
mkdir -p "$PKG_DIR/usr/share/ao-sims/viewer"
cp -r "$SIMS_DIR/viewer/dist/"* "$PKG_DIR/usr/share/ao-sims/viewer/"

# Docs
if [[ -d "$SIMS_DIR/docs" ]]; then
    mkdir -p "$PKG_DIR/usr/share/doc/ao-sims"
    cp "$SIMS_DIR/docs/"*.md "$PKG_DIR/usr/share/doc/ao-sims/" 2>/dev/null || true
fi

# DEBIAN control
mkdir -p "$PKG_DIR/DEBIAN"
INSTALLED_SIZE=$(du -sk "$PKG_DIR/usr" | cut -f1)

cat > "$PKG_DIR/DEBIAN/control" <<CTRL
Package: $PKG_NAME
Version: $VERSION
Section: misc
Priority: optional
Architecture: $ARCH
Installed-Size: $INSTALLED_SIZE
Depends: ao-recorder (>= 0.1.0)
Suggests: ao-validator, ao-exchange
Maintainer: Assign Onward Project <noreply@assignonward.com>
Description: Assign Onward simulation suite
 CLI agent simulations and browser-based viewer for the Assign Onward
 blockchain platform. Includes scenario files for island economies,
 exchange testing, and adversarial auditing.
CTRL

dpkg-deb --build --root-owner-group "$PKG_DIR" "$OUT_DIR/${PKG_NAME}_${VERSION}_${ARCH}.deb"

echo ""
echo "==> Built: $OUT_DIR/${PKG_NAME}_${VERSION}_${ARCH}.deb"
echo "    Scenarios installed to: /usr/share/ao-sims/scenarios/"
echo "    Viewer PWA installed to: /usr/share/ao-sims/viewer/"
ls -lh "$OUT_DIR/${PKG_NAME}_${VERSION}_${ARCH}.deb"
