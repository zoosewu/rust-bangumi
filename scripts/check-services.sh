#!/bin/bash
# Quick service status check
# Usage: ./scripts/check-services.sh

echo "=== Bangumi Service Status ==="
echo ""

# Infrastructure
echo "Infrastructure:"
echo -n "  PostgreSQL (5432):    "
if docker ps --format '{{.Names}}' | grep -q "bangumi-postgres"; then
    echo -e "\033[32mRunning\033[0m"
else
    echo -e "\033[31mStopped\033[0m"
fi

echo -n "  qBittorrent (8080):   "
if curl -s --connect-timeout 2 http://localhost:8080 > /dev/null 2>&1; then
    echo -e "\033[32mRunning\033[0m"
else
    echo -e "\033[31mStopped\033[0m"
fi

echo ""
echo "Application Services:"

echo -n "  Core (8000):          "
if curl -s --connect-timeout 2 http://localhost:8000/services > /dev/null 2>&1; then
    echo -e "\033[32mRunning\033[0m"
else
    echo -e "\033[31mStopped\033[0m"
fi

echo -n "  Fetcher (8001):       "
if curl -s --connect-timeout 2 http://localhost:8001/health > /dev/null 2>&1; then
    echo -e "\033[32mRunning\033[0m"
else
    echo -e "\033[31mStopped\033[0m"
fi

echo -n "  Downloader (8002):    "
if curl -s --connect-timeout 2 http://localhost:8002/health > /dev/null 2>&1; then
    echo -e "\033[32mRunning\033[0m"
else
    echo -e "\033[31mStopped\033[0m"
fi

echo ""
echo "=== Registered Services in Core ==="
curl -s http://localhost:8000/services 2>/dev/null | jq -r '.services[] | "  \(.service_name) (\(.service_type)) - \(.host):\(.port)"' 2>/dev/null || echo "  (Core not available)"

echo ""
