#!/usr/bin/env bash
# Build Debian packages for Assign Onward suite.
# Usage: ./build.sh [--target TARGET] [--release]
#
# Produces .deb packages for: ao-recorder, ao-validator, ao-exchange, ao-cli
# Requires: cargo, dpkg-deb, strip (binutils)
#
# Options:
#   --target TARGET   Rust target triple (default: host)
#   --release         Build in release mode (default)
#   --debug           Build in debug mode
#
# Examples:
#   ./build.sh                                  # native release build
#   ./build.sh --target aarch64-unknown-linux-gnu  # cross-compile for Pi

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKSPACE_DIR="$(cd "$SCRIPT_DIR/../../src" && pwd)"
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

# Determine binary output path
if [[ -n "$TARGET" ]]; then
    BIN_DIR="$WORKSPACE_DIR/target/$TARGET/$PROFILE"
else
    BIN_DIR="$WORKSPACE_DIR/target/$PROFILE"
fi

ARCH=$(dpkg --print-architecture 2>/dev/null || echo "amd64")
if [[ "$TARGET" == *"aarch64"* ]]; then
    ARCH="arm64"
elif [[ "$TARGET" == *"armv7"* ]]; then
    ARCH="armhf"
fi

# --- Build all binaries ---
echo "==> Building workspace ($PROFILE, ${TARGET:-native})..."
(cd "$WORKSPACE_DIR" && cargo build --$PROFILE $CARGO_TARGET_FLAG)

mkdir -p "$OUT_DIR"

# --- Package definitions ---
# Each entry: binary_name  package_name  version  description  has_service
PACKAGES=(
    "ao-recorder|ao-recorder|0.1.0|Assign Onward chain recording server|yes"
    "ao-validator|ao-validator|0.1.0|Assign Onward chain validator daemon|yes"
    "ao-exchange|ao-exchange|0.1.0|Assign Onward exchange agent|yes"
    "ao|ao-cli|0.1.0|Assign Onward command-line interface|no"
)

for entry in "${PACKAGES[@]}"; do
    IFS='|' read -r BIN_NAME PKG_NAME VERSION DESCRIPTION HAS_SERVICE <<< "$entry"

    echo "==> Packaging $PKG_NAME ($VERSION)..."

    PKG_DIR="$OUT_DIR/${PKG_NAME}_${VERSION}_${ARCH}"
    rm -rf "$PKG_DIR"

    # Binary
    mkdir -p "$PKG_DIR/usr/bin"
    cp "$BIN_DIR/$BIN_NAME" "$PKG_DIR/usr/bin/$BIN_NAME"
    strip "$PKG_DIR/usr/bin/$BIN_NAME" 2>/dev/null || true

    # DEBIAN control files
    mkdir -p "$PKG_DIR/DEBIAN"

    INSTALLED_SIZE=$(du -sk "$PKG_DIR/usr" | cut -f1)

    cat > "$PKG_DIR/DEBIAN/control" <<CTRL
Package: $PKG_NAME
Version: $VERSION
Section: net
Priority: optional
Architecture: $ARCH
Installed-Size: $INSTALLED_SIZE
Maintainer: Assign Onward Project <noreply@assignonward.com>
Description: $DESCRIPTION
 Part of the Assign Onward lightweight federated blockchain suite.
CTRL

    if [[ "$HAS_SERVICE" == "yes" ]]; then
        # Config directory
        mkdir -p "$PKG_DIR/etc/$PKG_NAME"

        # Data directory
        mkdir -p "$PKG_DIR/var/lib/$PKG_NAME"

        # systemd service
        mkdir -p "$PKG_DIR/lib/systemd/system"
        cp "$SCRIPT_DIR/${PKG_NAME}.service" "$PKG_DIR/lib/systemd/system/"

        # postinst
        cp "$SCRIPT_DIR/postinst-${PKG_NAME}" "$PKG_DIR/DEBIAN/postinst"
        chmod 755 "$PKG_DIR/DEBIAN/postinst"

        # prerm
        cp "$SCRIPT_DIR/prerm-service" "$PKG_DIR/DEBIAN/prerm"
        sed -i "s/@@PKG_NAME@@/$PKG_NAME/g" "$PKG_DIR/DEBIAN/prerm"
        chmod 755 "$PKG_DIR/DEBIAN/prerm"

        # postrm
        cp "$SCRIPT_DIR/postrm-service" "$PKG_DIR/DEBIAN/postrm"
        sed -i "s/@@PKG_NAME@@/$PKG_NAME/g" "$PKG_DIR/DEBIAN/postrm"
        chmod 755 "$PKG_DIR/DEBIAN/postrm"

        # conffiles (mark config as user-editable)
        echo "/etc/$PKG_NAME/" > "$PKG_DIR/DEBIAN/conffiles"
    fi

    dpkg-deb --build --root-owner-group "$PKG_DIR" "$OUT_DIR/${PKG_NAME}_${VERSION}_${ARCH}.deb"
    echo "    -> $OUT_DIR/${PKG_NAME}_${VERSION}_${ARCH}.deb"
done

echo ""
echo "==> All packages built in $OUT_DIR/"
ls -lh "$OUT_DIR/"*.deb
