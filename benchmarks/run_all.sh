#!/bin/bash
# Run all benchmarks for ZoldyQ
#
# Usage:
#   ./benchmarks/run_all.sh
#
# Requirements:
#   - Python 3.7+
#   - Docker (for Redis and RabbitMQ)
#   - Rust 1.75+

set -e

echo "================================"
echo "ZoldyQ Benchmark Suite"
echo "================================"
echo ""

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check Python dependencies
echo -e "${BLUE}Checking Python dependencies...${NC}"
MISSING_DEPS=()

if ! python3 -c "import pika" 2>/dev/null; then
    MISSING_DEPS+=("pika")
fi

if ! python3 -c "import redis" 2>/dev/null; then
    MISSING_DEPS+=("redis")
fi

if ! python3 -c "import pika" 2>/dev/null; then
    MISSING_DEPS+=("pika")
fi

if ! python3 -c "import numpy" 2>/dev/null; then
    MISSING_DEPS+=("numpy")
fi

if ! python3 -c "import tabulate" 2>/dev/null; then
    MISSING_DEPS+=("tabulate")
fi

if [ ${#MISSING_DEPS[@]} -gt 0 ]; then
    echo -e "${YELLOW}⚠ Missing Python dependencies: ${MISSING_DEPS[*]}${NC}"
    echo -e "${BLUE}Installing dependencies...${NC}"
    pip3 install ${MISSING_DEPS[*]}
    echo -e "${GREEN}✓ Dependencies installed${NC}"
else
    echo -e "${GREEN}✓ All Python dependencies installed${NC}"
fi
echo ""

# Check if ZoldyQ is running
echo -e "${BLUE}Checking if ZoldyQ RESP server is running...${NC}"
# Test RESP connection using redis-cli or Python
if redis-cli -p 6379 PING > /dev/null 2>&1 || python3 -c "import redis; redis.Redis(host='localhost', port=6379).ping()" 2>/dev/null; then
    echo -e "${GREEN}✓ ZoldyQ RESP server is running on port 6379${NC}"
else
    echo -e "${YELLOW}⚠ ZoldyQ is not running. Starting it...${NC}"
    ZOLDYQ_QUEUE_CAPACITY=2000000 cargo build --release
    ./target/release/zoldyq > /tmp/zoldyq.log 2>&1 &
    ZOLDYQ_PID=$!
    echo -e "${BLUE}   Waiting for server to start...${NC}"
    
    # Wait for server to be ready (max 10 seconds)
    for i in {1..10}; do
        sleep 1
        if redis-cli -p 6379 PING > /dev/null 2>&1 || python3 -c "import redis; redis.Redis(host='localhost', port=6379).ping()" 2>/dev/null; then
            echo -e "${GREEN}✓ ZoldyQ started (PID: $ZOLDYQ_PID)${NC}"
            break
        fi
        if [ $i -eq 10 ]; then
            echo -e "${RED}✗ Failed to start ZoldyQ after 10 seconds${NC}"
            echo -e "${YELLOW}   Check logs: tail /tmp/zoldyq.log${NC}"
            exit 1
        fi
    done
fi
echo ""

# # 1. Run Rust internal benchmarks
# echo "================================"
# echo "1. Internal Rust Benchmarks"
# echo "================================"
# echo ""
# echo -e "${BLUE}Running criterion benchmarks...${NC}"
# cargo bench --bench queue_benchmarks
# echo ""

# # 2. Run HTTP API benchmarks
# echo "================================"
# echo "2. HTTP API Benchmarks"
# echo "================================"
# echo ""
# echo -e "${BLUE}Running HTTP benchmarks...${NC}"
# cargo bench --bench http_benchmarks
# echo ""

# 3. Compare with Redis
echo "================================"
echo "3. ZoldyQ vs Redis"
echo "================================"
echo ""
echo -e "${BLUE}Checking if Redis is running...${NC}"
if docker ps | grep -q redis; then
    echo -e "${GREEN}✓ Redis is running${NC}"
else
    echo -e "${YELLOW}⚠ Redis is not running. Starting it...${NC}"
    docker run -d --name redis -p 6380:6379 redis:latest
sleep 2
echo -e "${GREEN}✓ Redis started on port 6380${NC}"
fi
echo ""

echo -e "${BLUE}Running comparison benchmark...${NC}"
python3 benchmarks/compare_redis.py
echo ""

# 4. Compare with RabbitMQ
echo "================================"
echo "4. ZoldyQ vs RabbitMQ"
echo "================================"
echo ""
echo -e "${BLUE}Checking if RabbitMQ is running...${NC}"
if docker ps | grep -q rabbitmq; then
    echo -e "${GREEN}✓ RabbitMQ is running${NC}"
else
    echo -e "${YELLOW}⚠ RabbitMQ is not running. Starting it...${NC}"
    docker run -d --name rabbitmq -p 5672:5672 -p 15672:15672 rabbitmq:3-management
sleep 5
echo -e "${GREEN}✓ RabbitMQ started on port 5672${NC}"
fi
echo ""

echo -e "${BLUE}Running comparison benchmark...${NC}"
python3 benchmarks/compare_rabbitmq.py
echo ""

# Summary
echo "================================"
echo "Benchmark Suite Complete!"
echo "================================"
echo ""
echo "Results saved to:"
echo "  - Criterion reports: target/criterion/"
echo "  - HTML reports: target/criterion/report/index.html"
echo ""
echo "To view HTML reports:"
echo "  open target/criterion/report/index.html"
echo ""

# Cleanup if we started ZoldyQ
if [ ! -z "$ZOLDYQ_PID" ]; then
    echo -e "${YELLOW}Stopping ZoldyQ (PID: $ZOLDYQ_PID)...${NC}"
    kill $ZOLDYQ_PID
    echo -e "${GREEN}✓ ZoldyQ stopped${NC}"
fi

