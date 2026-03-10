#!/usr/bin/env bash
# run-sim.sh — Run an Assign Onward simulation by name
# Usage: ./scripts/run-sim.sh <scenario-name> [--viewer-port PORT]
#
# Examples:
#   ./scripts/run-sim.sh minimal
#   ./scripts/run-sim.sh island-life --viewer-port 4200
#   ./scripts/run-sim.sh all          # run every simulation sequentially
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SIMS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── Parse arguments ────────────────────────────────────────────────────
SCENARIO="${1:-}"
VIEWER_PORT="${3:-4200}"

if [[ "$SCENARIO" == "" ]] || [[ "$SCENARIO" == "--help" ]] || [[ "$SCENARIO" == "-h" ]]; then
    echo "Usage: $0 <scenario-name> [--viewer-port PORT]"
    echo ""
    echo "Available simulations:"
    echo "  minimal           Basic single-chain buy-redeem cycle (2 min)"
    echo "  three-chain       Multi-vendor trading on one recorder (3 min)"
    echo "  exchange-3chain   Cross-chain exchange mechanics (3 min)"
    echo "  price-war         Competitive exchange pricing dynamics (5 min)"
    echo "  atomic-exchange   CAA atomic cross-chain swaps (3 min)"
    echo "  island-life       Beach economy with map visualization (5 min)"
    echo "  island-life-full  Full island + validator + attacker (5 min)"
    echo "  audit-adversarial Five attack types vs validator (3 min)"
    echo "  infra-resilience  Server hardening verification (2 min)"
    echo "  recorder-switch   Recorder migration & owner key rotation (3 min)"
    echo "  all               Run every simulation sequentially"
    echo ""
    echo "Options:"
    echo "  --viewer-port PORT  Viewer API port (default: 4200)"
    echo ""
    echo "While running, open http://127.0.0.1:PORT in your browser for the viewer."
    exit 0
fi

# Handle --viewer-port flag in any position
EXTRA_ARGS=()
while [[ $# -gt 1 ]]; do
    shift
    case "${1:-}" in
        --viewer-port|--viewer_port)
            shift
            VIEWER_PORT="${1:-4200}"
            ;;
        *)
            EXTRA_ARGS+=("$1")
            ;;
    esac
done

# ── Find the binary ───────────────────────────────────────────────────
SIMS_BIN=""
for candidate in \
    "$SIMS_DIR/target/release/ao-sims" \
    "$SIMS_DIR/target/release/ao-sims.exe" \
    "$SIMS_DIR/target/debug/ao-sims" \
    "$SIMS_DIR/target/debug/ao-sims.exe"; do
    if [[ -f "$candidate" ]]; then
        SIMS_BIN="$candidate"
        break
    fi
done

if [[ -z "$SIMS_BIN" ]]; then
    echo "ERROR: ao-sims binary not found. Run ./scripts/build.sh first."
    exit 1
fi

# ── Start viewer PWA if available ─────────────────────────────────────
VIEWER_PID=""
start_viewer_pwa() {
    local viewer_dir="$SIMS_DIR/viewer"
    if [[ -f "$viewer_dir/package.json" ]] && command -v npm &>/dev/null; then
        cd "$viewer_dir"
        npm run dev &>/dev/null &
        VIEWER_PID=$!
        cd "$SIMS_DIR"
    fi
}

stop_viewer_pwa() {
    if [[ -n "$VIEWER_PID" ]] && kill -0 "$VIEWER_PID" 2>/dev/null; then
        kill "$VIEWER_PID" 2>/dev/null || true
        wait "$VIEWER_PID" 2>/dev/null || true
        VIEWER_PID=""
    fi
}

# ── Run simulation(s) ─────────────────────────────────────────────────
run_scenario() {
    local name="$1"
    local toml="$SIMS_DIR/scenarios/${name}.toml"

    if [[ ! -f "$toml" ]]; then
        echo "ERROR: Scenario file not found: $toml"
        echo "Run '$0 --help' to see available simulations."
        return 1
    fi

    echo ""
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "  Simulation: $name"
    echo "  Viewer UI:  http://127.0.0.1:5174"
    echo "  Viewer API: http://127.0.0.1:$VIEWER_PORT"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo ""

    start_viewer_pwa
    trap stop_viewer_pwa EXIT INT TERM

    cd "$SIMS_DIR"
    "$SIMS_BIN" "$toml" --viewer-port "$VIEWER_PORT"

    stop_viewer_pwa

    echo ""
    echo "── $name complete ──"
}

if [[ "$SCENARIO" == "all" ]]; then
    echo "=== Running all simulations sequentially ==="
    for toml in "$SIMS_DIR"/scenarios/*.toml; do
        name="$(basename "$toml" .toml)"
        run_scenario "$name"
        echo ""
        echo "Pausing 3 seconds before next simulation..."
        sleep 3
    done
    echo ""
    echo "=== All simulations complete ==="
else
    run_scenario "$SCENARIO"
fi
