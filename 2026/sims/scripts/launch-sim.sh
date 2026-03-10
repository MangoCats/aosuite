#!/usr/bin/env bash
# launch-sim.sh — Launch a simulation + viewer in one command
# Usage: bash scripts/launch-sim.sh [scenario]
#
# Example: bash scripts/launch-sim.sh minimal
set -euo pipefail

SCENARIO="${1:-minimal}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SIMS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
VIEWER_DIR="$SIMS_DIR/viewer"

# Find binary
SIMS_BIN=""
for f in "$SIMS_DIR/target/release/ao-sims" "$SIMS_DIR/target/release/ao-sims.exe" \
         "$SIMS_DIR/target/debug/ao-sims" "$SIMS_DIR/target/debug/ao-sims.exe"; do
    [[ -f "$f" ]] && SIMS_BIN="$f" && break
done
[[ -z "$SIMS_BIN" ]] && echo "ERROR: ao-sims not found. Build first." && exit 1

TOML="$SIMS_DIR/scenarios/${SCENARIO}.toml"
[[ ! -f "$TOML" ]] && echo "ERROR: $TOML not found" && exit 1

PIDS=()
cleanup() {
    echo ""
    echo "Shutting down..."
    for pid in "${PIDS[@]}"; do
        kill "$pid" 2>/dev/null || true
    done
    wait 2>/dev/null || true
    echo "Done."
}
trap cleanup EXIT INT TERM

# 1. Start simulation (viewer API on 4200)
echo "Starting simulation: $SCENARIO"
cd "$SIMS_DIR"
"$SIMS_BIN" "$TOML" --viewer-port 4200 &
PIDS+=($!)
sleep 2

# 2. Start viewer PWA (UI on 5174)
echo "Starting viewer UI..."
cd "$VIEWER_DIR"
npm run dev &
PIDS+=($!)
sleep 3

echo ""
echo "=================================================="
echo "  Sim '$SCENARIO' is running!"
echo "  Open: http://127.0.0.1:5174"
echo "  Press Ctrl+C to stop"
echo "=================================================="
echo ""

# Wait for sim to finish
wait "${PIDS[0]}"
