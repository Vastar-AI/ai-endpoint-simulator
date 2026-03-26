#!/bin/bash

# Stop simulator and Redis

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

echo "Stopping AI Endpoint Simulator..."

# Stop simulator
pkill -f "ai-endpoint-simulator" 2>/dev/null && \
    echo -e "${GREEN}✓ Simulator stopped${NC}" || \
    echo -e "${RED}Simulator was not running${NC}"

# Optionally stop Redis (uncomment if needed)
# docker stop redis-simulator 2>/dev/null && \
#     echo -e "${GREEN}✓ Redis stopped${NC}" || \
#     echo -e "${RED}Redis was not running${NC}"

echo "Done."
