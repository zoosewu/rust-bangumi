#!/bin/bash
# Quick smoke test - verifies basic connectivity
# Usage: ./scripts/smoke-test.sh
# Exit codes: 0 = all services up, 1 = some services down

set -e

FAILED=0

check() {
    local name=$1
    local url=$2
    if curl -sf --connect-timeout 3 "$url" > /dev/null 2>&1; then
        echo -e "[\033[32mOK\033[0m] $name"
    else
        echo -e "[\033[31mFAIL\033[0m] $name"
        FAILED=1
    fi
}

echo "Smoke Test: $(date)"
echo "---"
check "PostgreSQL" "localhost:5432"
check "Core Service" "http://localhost:8000/services"
check "Fetcher Service" "http://localhost:8001/health"
check "Downloader Service" "http://localhost:8002/health"
check "qBittorrent" "http://localhost:8080"

exit $FAILED
