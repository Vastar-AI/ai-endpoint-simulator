#!/bin/bash

# ============================================
# AI Endpoint Simulator - Native Runner
# ============================================
# Zero external dependencies — no Redis needed
# Uses CPU cores 0-7, leaving 8-15 for workflow
# ============================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="${SCRIPT_DIR}/target/release/ai-endpoint-simulator"
PORT="${PORT:-4545}"
LOG_LEVEL="${LOG_LEVEL:-info}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${GREEN}=== AI Endpoint Simulator ===${NC}"
echo ""

# Kill any existing simulator process
echo -e "${YELLOW}Checking for existing processes...${NC}"
EXISTING_PID=$(lsof -ti:${PORT} 2>/dev/null || true)
if [ -n "$EXISTING_PID" ]; then
    echo -e "${YELLOW}Killing process on port ${PORT}: PID ${EXISTING_PID}${NC}"
    kill -9 $EXISTING_PID 2>/dev/null || true
    sleep 1
    echo -e "${GREEN}✓ Port ${PORT} freed${NC}"
else
    echo -e "${GREEN}✓ Port ${PORT} is free${NC}"
fi

# Build if binary not found
if [ ! -f "$BINARY" ]; then
    echo -e "${YELLOW}Binary not found. Building...${NC}"
    cd "$SCRIPT_DIR"
    cargo build --release
fi

echo ""
echo -e "${GREEN}Configuration:${NC}"
echo "  - Port: ${PORT}"
echo "  - Workers: 4"
echo "  - Cache: in-memory (DashMap)"
echo "  - Data: embedded (130 response files)"
echo ""

echo -e "${CYAN}Endpoints:${NC}"
echo "  OpenAI     POST http://localhost:${PORT}/v1/chat/completions"
echo "  Anthropic  POST http://localhost:${PORT}/v1/messages"
echo "  Ollama     POST http://localhost:${PORT}/api/chat"
echo "  Cohere     POST http://localhost:${PORT}/v1/chat"
echo "  Gemini     POST http://localhost:${PORT}/v1beta/models/{model}:streamGenerateContent"
echo "  Health     GET  http://localhost:${PORT}/health"
echo ""
echo -e "${CYAN}Quick test:${NC}"
echo "  curl -N -X POST http://localhost:${PORT}/v1/chat/completions \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -d '{\"model\":\"gpt-4\",\"messages\":[{\"role\":\"user\",\"content\":\"Hello\"}],\"stream\":true}'"
echo ""
echo -e "${CYAN}Benchmark:${NC}"
echo "  oha -m POST -H 'Content-Type: application/json' \\"
echo "    -d '{\"model\":\"gpt-4\",\"messages\":[{\"role\":\"user\",\"content\":\"bench\"}]}' \\"
echo "    -z 10s -c 200 http://localhost:${PORT}/v1/chat/completions"
echo ""

# Run with CPU affinity if available
cd "$SCRIPT_DIR"
if command -v taskset &> /dev/null; then
    exec taskset -c 0-7 "$BINARY"
else
    exec "$BINARY"
fi
