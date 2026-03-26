#!/bin/bash

# =============================================================================
# AI Endpoint Simulator — Multi-Dialect Benchmark
# =============================================================================
# Tests all 5 SSE dialects: OpenAI, Anthropic, Ollama, Cohere, Gemini
# Requires: oha (cargo install oha)

set -e

HOST="${HOST:-localhost}"
PORT="${PORT:-4545}"
BASE_URL="http://${HOST}:${PORT}"
CONCURRENCY="${CONCURRENCY:-500}"
REQUESTS="${REQUESTS:-5000}"

GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}=== AI Endpoint Simulator — Multi-Dialect Benchmark ===${NC}"
echo -e "Target: ${BASE_URL} | Concurrency: ${CONCURRENCY} | Requests: ${REQUESTS}"
echo ""

# Check oha
if ! command -v oha &> /dev/null; then
    echo "oha not found. Install: cargo install oha"
    exit 1
fi

# Check simulator
if ! curl -s "${BASE_URL}/health" > /dev/null 2>&1; then
    echo -e "${YELLOW}Simulator not running. Start it first: ./run_simulator.sh${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Simulator is running${NC}"
echo ""

# Benchmark helper
bench() {
    local name="$1"
    local endpoint="$2"
    local body="$3"

    echo -e "${CYAN}=== ${name} (c${CONCURRENCY} n${REQUESTS}) ===${NC}"
    oha -c "$CONCURRENCY" -n "$REQUESTS" -m POST \
        -H 'Content-Type: application/json' \
        -d "$body" \
        "${BASE_URL}${endpoint}" 2>&1 | grep -E "Success|Requests/sec|50.00%|99.00%|Slowest"
    echo ""
}

# Health baseline
echo -e "${CYAN}=== Health Endpoint (baseline) ===${NC}"
oha -c 100 -n 5000 "${BASE_URL}/health" 2>&1 | grep -E "Success|Requests/sec|50.00%|99.00%"
echo ""

# All 5 dialects
bench "OpenAI" \
    "/v1/chat/completions" \
    '{"model":"gpt-4o","messages":[{"role":"user","content":"Hello"}],"stream":true}'

bench "Anthropic" \
    "/v1/messages" \
    '{"model":"claude-3-5-sonnet","messages":[{"role":"user","content":"Hello"}],"stream":true}'

bench "Ollama" \
    "/api/chat" \
    '{"model":"llama3","messages":[{"role":"user","content":"Hello"}],"stream":true}'

bench "Cohere" \
    "/v1/chat" \
    '{"messages":[{"role":"user","content":"Hello"}],"stream":true}'

bench "Gemini" \
    "/v1beta/models/gemini-1.5-pro:streamGenerateContent" \
    '{"contents":[{"parts":[{"text":"Hello"}]}]}'

echo -e "${GREEN}=== Benchmark Complete ===${NC}"
echo ""
echo "Override defaults: CONCURRENCY=1000 REQUESTS=10000 ./benchmark.sh"
