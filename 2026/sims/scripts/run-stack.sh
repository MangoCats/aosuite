#!/usr/bin/env bash
# run-stack.sh — Launch the full Assign Onward component stack on localhost
#
# Starts all server components on fixed, non-conflicting ports:
#   ao-recorder  A  →  http://127.0.0.1:3000
#   ao-recorder  B  →  http://127.0.0.1:3010
#   ao-validator    →  http://127.0.0.1:4000
#   ao-exchange     →  http://127.0.0.1:3100
#   ao-relay        →  ws://127.0.0.1:3200
#   ao-pwa (dev)    →  http://127.0.0.1:5173
#
# Creates temporary config files in $DATA_DIR (default: /tmp/ao-stack).
# Press Ctrl+C to stop all components.
#
# Usage: ./scripts/run-stack.sh [--data-dir DIR] [--no-pwa]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SIMS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT_2026="$(cd "$SIMS_DIR/.." && pwd)"
SRC_DIR="$ROOT_2026/src"

# ── Defaults ──────────────────────────────────────────────────────────
DATA_DIR="/tmp/ao-stack"
START_PWA=true

# Port assignments
PORT_RECORDER_A=3000
PORT_RECORDER_B=3010
PORT_VALIDATOR=4000
PORT_EXCHANGE=3100
PORT_RELAY=3200
PORT_PWA=5173

# ── Parse arguments ───────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --data-dir) DATA_DIR="$2"; shift 2 ;;
        --no-pwa)   START_PWA=false; shift ;;
        --help|-h)
            echo "Usage: $0 [--data-dir DIR] [--no-pwa]"
            echo ""
            echo "Launches the full AO stack on localhost with these ports:"
            echo "  Recorder A:  $PORT_RECORDER_A"
            echo "  Recorder B:  $PORT_RECORDER_B"
            echo "  Validator:   $PORT_VALIDATOR"
            echo "  Exchange:    $PORT_EXCHANGE"
            echo "  Relay:       $PORT_RELAY"
            echo "  PWA (dev):   $PORT_PWA"
            exit 0
            ;;
        *) echo "Unknown argument: $1"; exit 1 ;;
    esac
done

# ── Find binaries ────────────────────────────────────────────────────
BIN_DIR="$SRC_DIR/target/release"
if [[ ! -f "$BIN_DIR/ao-recorder" ]] && [[ ! -f "$BIN_DIR/ao-recorder.exe" ]]; then
    BIN_DIR="$ROOT_2026/target/release"
fi

find_bin() {
    local name="$1"
    for ext in "" ".exe"; do
        if [[ -f "$BIN_DIR/$name$ext" ]]; then
            echo "$BIN_DIR/$name$ext"
            return
        fi
    done
    echo ""
}

RECORDER_BIN=$(find_bin ao-recorder)
VALIDATOR_BIN=$(find_bin ao-validator)
EXCHANGE_BIN=$(find_bin ao-exchange)
RELAY_BIN=$(find_bin ao-relay)

if [[ -z "$RECORDER_BIN" ]]; then
    echo "ERROR: ao-recorder binary not found. Run ./scripts/build.sh first."
    exit 1
fi

echo "=== Assign Onward — Full Stack Launcher ==="
echo "Data directory: $DATA_DIR"
echo ""

# ── Create data directories ──────────────────────────────────────────
mkdir -p "$DATA_DIR/recorder-a" "$DATA_DIR/recorder-b" "$DATA_DIR/validator" "$DATA_DIR/exchange"

# ── Generate Ed25519 seeds (deterministic from fixed values for reproducibility) ──
# Use ao-cli keygen if available, otherwise use openssl
gen_seed() {
    openssl rand -hex 32 2>/dev/null || python3 -c "import secrets; print(secrets.token_hex(32))" 2>/dev/null || head -c 32 /dev/urandom | xxd -p -c 32
}

SEED_A="${AO_SEED_A:-$(gen_seed)}"
SEED_B="${AO_SEED_B:-$(gen_seed)}"
SEED_V="${AO_SEED_V:-$(gen_seed)}"
SEED_X="${AO_SEED_X:-$(gen_seed)}"

# ── Write recorder A config ──────────────────────────────────────────
cat > "$DATA_DIR/recorder-a/recorder.toml" <<EOF
host = "127.0.0.1"
port = $PORT_RECORDER_A
blockmaker_seed = "$SEED_A"
data_dir = "$DATA_DIR/recorder-a/data"
dashboard = true

[[validators]]
url = "http://127.0.0.1:$PORT_VALIDATOR"
label = "local-validator"
EOF

# ── Write recorder B config ──────────────────────────────────────────
cat > "$DATA_DIR/recorder-b/recorder.toml" <<EOF
host = "127.0.0.1"
port = $PORT_RECORDER_B
blockmaker_seed = "$SEED_B"
data_dir = "$DATA_DIR/recorder-b/data"
dashboard = true

[[validators]]
url = "http://127.0.0.1:$PORT_VALIDATOR"
label = "local-validator"
EOF

# ── Write validator config ───────────────────────────────────────────
cat > "$DATA_DIR/validator/validator.toml" <<EOF
host = "127.0.0.1"
port = $PORT_VALIDATOR
db_path = "$DATA_DIR/validator/validator.db"
validator_seed = "$SEED_V"
poll_interval_secs = 10

# Chains will be added dynamically as they're created on the recorders.
# For now, start with no monitored chains.
# Add chains with:
#   [[chains]]
#   recorder_url = "http://127.0.0.1:$PORT_RECORDER_A"
#   chain_id = "<hex>"
#   label = "my-chain"
EOF

# ── Write exchange config ────────────────────────────────────────────
cat > "$DATA_DIR/exchange/exchange.toml" <<EOF
# Exchange agent — configure trading pairs after creating chains.
# Start with empty config; add pairs and chains as needed.

db_path = "$DATA_DIR/exchange/exchange_trades.db"
poll_interval_secs = 5
deposit_detection = "sse"
trade_ttl_secs = 300

# Example pair (uncomment and fill in after creating chains):
# [[pairs]]
# sell = "BCG"
# buy = "CCC"
# rate = 12.0
# spread = 0.02

# [[chains]]
# symbol = "BCG"
# recorder_url = "http://127.0.0.1:$PORT_RECORDER_A"
# chain_id = "..."
# key_seed = "..."
EOF

# ── PID tracking for cleanup ─────────────────────────────────────────
PIDS=()

cleanup() {
    echo ""
    echo "Shutting down all components..."
    for pid in "${PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
        fi
    done
    wait 2>/dev/null || true
    echo "All components stopped."
}

trap cleanup EXIT INT TERM

# ── Start components ─────────────────────────────────────────────────
echo "Starting components..."
echo ""

# Recorder A
echo "  [1/5] Recorder A → http://127.0.0.1:$PORT_RECORDER_A"
"$RECORDER_BIN" "$DATA_DIR/recorder-a/recorder.toml" &
PIDS+=($!)

# Recorder B
echo "  [2/5] Recorder B → http://127.0.0.1:$PORT_RECORDER_B"
"$RECORDER_BIN" "$DATA_DIR/recorder-b/recorder.toml" &
PIDS+=($!)

# Validator
if [[ -n "$VALIDATOR_BIN" ]]; then
    echo "  [3/5] Validator  → http://127.0.0.1:$PORT_VALIDATOR"
    "$VALIDATOR_BIN" run "$DATA_DIR/validator/validator.toml" &
    PIDS+=($!)
else
    echo "  [3/5] Validator  → SKIPPED (binary not found)"
fi

# Relay
if [[ -n "$RELAY_BIN" ]]; then
    echo "  [4/5] Relay      → ws://127.0.0.1:$PORT_RELAY"
    "$RELAY_BIN" --listen "127.0.0.1:$PORT_RELAY" &
    PIDS+=($!)
else
    echo "  [4/5] Relay      → SKIPPED (binary not found)"
fi

# PWA dev server
if [[ "$START_PWA" == true ]] && [[ -d "$SRC_DIR/ao-pwa" ]] && command -v npm &>/dev/null; then
    echo "  [5/5] PWA (dev)  → http://127.0.0.1:$PORT_PWA"
    cd "$SRC_DIR/ao-pwa"
    npm run dev -- --port "$PORT_PWA" --host 127.0.0.1 &
    PIDS+=($!)
    cd "$SIMS_DIR"
else
    echo "  [5/5] PWA (dev)  → SKIPPED (--no-pwa or npm not found)"
fi

echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  Assign Onward stack is running                             ║"
echo "║                                                             ║"
echo "║  Recorder A:  http://127.0.0.1:$PORT_RECORDER_A                   ║"
echo "║  Recorder B:  http://127.0.0.1:$PORT_RECORDER_B                   ║"
echo "║  Validator:   http://127.0.0.1:$PORT_VALIDATOR                   ║"
echo "║  Exchange:    (configure $DATA_DIR/exchange/exchange.toml)  ║"
echo "║  Relay:       ws://127.0.0.1:$PORT_RELAY                    ║"
echo "║  PWA:         http://127.0.0.1:$PORT_PWA                   ║"
echo "║                                                             ║"
echo "║  Dashboard:   http://127.0.0.1:$PORT_RECORDER_A/dashboard         ║"
echo "║  Health:      http://127.0.0.1:$PORT_RECORDER_A/health            ║"
echo "║                                                             ║"
echo "║  Config dir:  $DATA_DIR"
echo "║                                                             ║"
echo "║  Press Ctrl+C to stop all components                        ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

# Wait for any process to exit
wait -n 2>/dev/null || wait
