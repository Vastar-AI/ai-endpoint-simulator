#!/bin/bash

# ============================================
# AI Endpoint Simulator - Native Runner
# ============================================
# Optimized for load testing workflow platform
# Uses CPU cores 0-7, leaving 8-15 for workflow
# ============================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_FILE="${SCRIPT_DIR}/config.yml"
BINARY="${SCRIPT_DIR}/target/release/ai-endpoint-simulator"
REDIS_CONTAINER="redis-simulator"
LOG_LEVEL="${LOG_LEVEL:-info}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${GREEN}=== AI Endpoint Simulator ===${NC}"
echo ""

# Use config.local.yml if config.yml doesn't exist
if [ ! -f "$CONFIG_FILE" ]; then
    if [ -f "${SCRIPT_DIR}/config.local.yml" ]; then
        cp "${SCRIPT_DIR}/config.local.yml" "$CONFIG_FILE"
        echo -e "${YELLOW}Created config.yml from config.local.yml${NC}"
    else
        echo -e "${RED}✗ No config file found${NC}"
        exit 1
    fi
fi

# Get port from config
PORT=$(grep 'port:' "$CONFIG_FILE" | head -1 | awk '{print $2}')
PORT=${PORT:-4545}

# Kill any existing simulator process by binary name
echo -e "${YELLOW}Checking for existing simulator processes...${NC}"
EXISTING_SIM=$(pgrep -f "target/release/ai-endpoint-simulator" 2>/dev/null | grep -v $$ || true)
if [ -n "$EXISTING_SIM" ]; then
    echo -e "${YELLOW}Killing existing simulator process(es): PID ${EXISTING_SIM}${NC}"
    kill -9 $EXISTING_SIM 2>/dev/null || true
    sleep 1
    echo -e "${GREEN}✓ Old simulator process killed${NC}"
else
    echo -e "${GREEN}✓ No existing simulator process found${NC}"
fi

# Double check: kill any process listening on the port
echo -e "${YELLOW}Checking for processes on port ${PORT}...${NC}"
EXISTING_PID=$(lsof -ti:${PORT} 2>/dev/null || true)
if [ -n "$EXISTING_PID" ]; then
    echo -e "${YELLOW}Killing process(es) on port ${PORT}: PID ${EXISTING_PID}${NC}"
    kill -9 $EXISTING_PID 2>/dev/null || true
    sleep 1
    echo -e "${GREEN}✓ Process on port ${PORT} killed${NC}"
else
    echo -e "${GREEN}✓ Port ${PORT} is free${NC}"
fi

# Check if binary exists
if [ ! -f "$BINARY" ]; then
    echo -e "${YELLOW}Binary not found. Building...${NC}"
    cd "$SCRIPT_DIR"
    cargo build --release
fi

# Start Redis if not running
echo ""
echo -e "${YELLOW}Checking Redis...${NC}"
if ! docker ps --format '{{.Names}}' 2>/dev/null | grep -q "^${REDIS_CONTAINER}$"; then
    # Check if container exists but stopped
    if docker ps -a --format '{{.Names}}' 2>/dev/null | grep -q "^${REDIS_CONTAINER}$"; then
        echo -e "${YELLOW}Starting existing Redis container...${NC}"
        docker start "$REDIS_CONTAINER" >/dev/null 2>&1
    else
        echo -e "${YELLOW}Creating and starting Redis container...${NC}"
        docker run -d --name "$REDIS_CONTAINER" \
            -p 6379:6379 \
            --cpuset-cpus="0" \
            --memory="512m" \
            redis:7-alpine redis-server --appendonly yes --maxmemory 256mb --maxmemory-policy allkeys-lru \
            >/dev/null 2>&1
    fi
    sleep 2
fi

# Verify Redis is running
if docker ps --format '{{.Names}}' 2>/dev/null | grep -q "^${REDIS_CONTAINER}$"; then
    echo -e "${GREEN}✓ Redis is running${NC}"
else
    echo -e "${RED}✗ Failed to start Redis${NC}"
    exit 1
fi

echo ""
echo -e "${GREEN}Configuration:${NC}"
echo "  - Config: $CONFIG_FILE"
echo "  - Workers: $(grep 'workers:' $CONFIG_FILE | awk '{print $2}')"
echo "  - Port: ${PORT}"
echo "  - CPU Cores: 0-7 (pinned)"
echo ""

# Run with CPU affinity (cores 0-7)
echo -e "${GREEN}Starting simulator with CPU pinning (cores 0-7)...${NC}"
echo ""
echo -e "${CYAN}Endpoints:${NC}"
echo "  OpenAI     POST http://localhost:${PORT}/v1/chat/completions"
echo "  Anthropic  POST http://localhost:${PORT}/v1/messages"
echo "  Ollama     POST http://localhost:${PORT}/api/chat"
echo "  Cohere     POST http://localhost:${PORT}/v1/chat"
echo "  Gemini     POST http://localhost:${PORT}/v1beta/models/{model}:streamGenerateContent"
echo "  Health     GET  http://localhost:${PORT}/health"
echo "  Test       POST http://localhost:${PORT}/test_completion"
echo ""
echo -e "${CYAN}Quick test (curl):${NC}"
echo "  curl -N -X POST http://localhost:${PORT}/v1/chat/completions \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -d '{\"model\":\"gpt-4o\",\"messages\":[{\"role\":\"user\",\"content\":\"Hello\"}],\"stream\":true}'"
echo ""
echo -e "${CYAN}Baseline benchmark (oha):${NC}"
echo "  # OpenAI (c500, 5000 requests)"
echo "  oha -c 500 -n 5000 -m POST -H 'Content-Type: application/json' \\"
echo "    -d '{\"model\":\"gpt-4o\",\"messages\":[{\"role\":\"user\",\"content\":\"Hi\"}],\"stream\":true}' \\"
echo "    http://localhost:${PORT}/v1/chat/completions"
echo ""
echo "  # Anthropic"
echo "  oha -c 500 -n 5000 -m POST -H 'Content-Type: application/json' \\"
echo "    -d '{\"model\":\"claude-sim\",\"messages\":[{\"role\":\"user\",\"content\":\"Hi\"}],\"stream\":true}' \\"
echo "    http://localhost:${PORT}/v1/messages"
echo ""
echo "  # All dialects"
echo "  ./benchmark.sh"
echo ""

# Check if taskset is available
if command -v taskset &> /dev/null; then
    exec taskset -c 0-7 "$BINARY"
else
    echo -e "${YELLOW}Warning: taskset not available, running without CPU pinning${NC}"
    exec "$BINARY"
fi
