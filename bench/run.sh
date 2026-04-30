#!/usr/bin/env bash
# cable-bench runner
# Usage: ./run.sh [option]

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}"

BINARY="${SCRIPT_DIR}/target/release/cable-bench"
OLLAMA_URL="${OLLAMA_URL:-http://localhost:11434}"
MODEL="${MODEL:-gemma4:e4b}"
THINKING_MODEL="${THINKING_MODEL:-lfm2.5-thinking:latest}"
NODES_CONFIG="${NODES_CONFIG:-nodes.json}"

build_binary() {
  echo "🔨 Building cable-bench (release)..."
  cargo build --release
  echo "✅ Built: ${BINARY}"
}

ensure_built() {
  local needs_build=0
  if [ ! -f "${BINARY}" ]; then
    needs_build=1
  else
    if find src Cargo.toml Cargo.lock -newer "${BINARY}" 2>/dev/null | grep -q .; then
      needs_build=1
    fi
  fi
  if [ "${needs_build}" -eq 1 ]; then
    echo "🔄 Source changed — rebuilding..."
    build_binary
  fi
}

require_nodes() {
  if [ ! -f "${NODES_CONFIG}" ]; then
    echo "❌ Node config not found: ${NODES_CONFIG}"
    echo "   Edit nodes.json with your node IPs and re-run."
    exit 1
  fi
}

show_menu() {
  echo ""
  echo "╔══════════════════════════════════════════════╗"
  echo "║       Cable Mode Benchmark Runner        ║"
  echo "╚══════════════════════════════════════════════╝"
  echo ""
  echo "  1) Build binary"
  echo "  ── Single node ──────────────────────────────"
  echo "  2) Run benchmark (standard model)"
  echo "  3) Run benchmark (thinking model)"
  echo "  ── Distributed (nodes.json) ─────────────────"
  echo "  4) Run benchmark — distributed"
  echo "  5) Run benchmark — distributed (thinking model)"
  echo "  6) Run both — distributed, then compare"
  echo "  ─────────────────────────────────────────────"
  echo "  7) Compare two result files"
  echo "  8) Exit"
  echo ""
  echo "  Env: MODEL=${MODEL}  THINKING_MODEL=${THINKING_MODEL}"
  echo "  Env: OLLAMA_URL=${OLLAMA_URL}  NODES_CONFIG=${NODES_CONFIG}"
  echo ""
}

cmd_run() {
  ensure_built
  echo ""
  echo "🚀 Running benchmark — model: ${MODEL}"
  echo "════════════════════════════════════════════════"
  "${BINARY}" --ollama "${OLLAMA_URL}" --model "${MODEL}" "$@"
}

cmd_run_thinking() {
  ensure_built
  echo ""
  echo "🚀 Running benchmark (thinking) — model: ${THINKING_MODEL}"
  echo "══════════════════════════════════════════════════════════"
  "${BINARY}" --ollama "${OLLAMA_URL}" --model "${THINKING_MODEL}" --thinking-model "$@"
}

cmd_dist() {
  require_nodes; ensure_built
  echo ""
  echo "🚀 Running benchmark — distributed (${NODES_CONFIG})"
  echo "════════════════════════════════════════════════"
  "${BINARY}" --model "${MODEL}" --nodes-config "${NODES_CONFIG}" "$@"
}

cmd_dist_thinking() {
  require_nodes; ensure_built
  echo ""
  echo "🚀 Running benchmark — distributed, thinking (${NODES_CONFIG})"
  echo "══════════════════════════════════════════════════════════════"
  "${BINARY}" --model "${THINKING_MODEL}" --nodes-config "${NODES_CONFIG}" --thinking-model "$@"
}

cmd_both() {
  require_nodes; ensure_built
  local safe_model; safe_model="${MODEL//:/_}"; safe_model="${safe_model////_}"
  local safe_think; safe_think="${THINKING_MODEL//:/_}"; safe_think="${safe_think////_}"
  local file_a="cable_bench_${safe_model}.json"
  local file_b="cable_bench_${safe_think}_thinking.json"

  echo ""
  echo "🚀 Step 1/2 — Standard model (${MODEL})"
  echo "════════════════════════════════════════════════"
  "${BINARY}" --model "${MODEL}" --nodes-config "${NODES_CONFIG}"

  echo ""
  echo "🚀 Step 2/2 — Thinking model (${THINKING_MODEL})"
  echo "════════════════════════════════════════════════"
  "${BINARY}" --model "${THINKING_MODEL}" --nodes-config "${NODES_CONFIG}" --thinking-model

  echo ""
  echo "📊 Comparison"
  echo "════════════════════════════════════════════════"
  "${BINARY}" compare "${file_a}" "${file_b}"
}

cmd_compare() {
  ensure_built
  read -rp "  File A (standard model result): " file_a
  read -rp "  File B (thinking model result): " file_b
  echo ""
  "${BINARY}" compare "${file_a}" "${file_b}"
}

case "${1:-}" in
  "")
    show_menu
    read -rp "Enter your choice: " choice
    case "${choice}" in
      1) build_binary ;;
      2) cmd_run ;;
      3) cmd_run_thinking ;;
      4) cmd_dist ;;
      5) cmd_dist_thinking ;;
      6) cmd_both ;;
      7) cmd_compare ;;
      8) exit 0 ;;
      *) echo "Invalid option"; show_menu ;;
    esac
    ;;
  1) build_binary ;;
  2) shift; cmd_run "$@" ;;
  3) shift; cmd_run_thinking "$@" ;;
  4) shift; cmd_dist "$@" ;;
  5) shift; cmd_dist_thinking "$@" ;;
  6) cmd_both ;;
  7) cmd_compare ;;
  8) exit 0 ;;
  compare) shift; "${BINARY}" compare "$@" ;;
  *)
    ensure_built
    "${BINARY}" "$@"
    ;;
esac
