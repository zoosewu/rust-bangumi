#!/bin/bash
# End-to-End Integration Test Script
# Usage: ./scripts/test-e2e.sh [--cleanup]
#
# Prerequisites:
#   - PostgreSQL running (docker-compose.dev.yaml)
#   - Core service running on port 8000
#   - Fetcher service running on port 8001
#   - Downloader service running on port 8002 (optional)
#   - qBittorrent running on port 8080 (optional)

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
CORE_URL="${CORE_URL:-http://localhost:8000}"
FETCHER_URL="${FETCHER_URL:-http://localhost:8001}"
DOWNLOADER_URL="${DOWNLOADER_URL:-http://localhost:8002}"
QBITTORRENT_URL="${QBITTORRENT_URL:-http://localhost:8080}"
TEST_RSS_URL="https://mikanani.me/RSS/Bangumi?bangumiId=3416&subgroupid=583"
TEST_SUBSCRIPTION_NAME="E2E-Test-$(date +%s)"
CLEANUP="${1:-}"

# Counters
PASSED=0
FAILED=0
SKIPPED=0

# Helper functions
log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[FAIL]${NC} $1"; }
log_pass() { echo -e "${GREEN}[PASS]${NC} $1"; ((PASSED++)); }
log_fail() { echo -e "${RED}[FAIL]${NC} $1"; ((FAILED++)); }
log_skip() { echo -e "${YELLOW}[SKIP]${NC} $1"; ((SKIPPED++)); }

check_service() {
    local name=$1
    local url=$2
    local endpoint=$3

    if curl -s --connect-timeout 3 "$url$endpoint" > /dev/null 2>&1; then
        log_pass "$name is running at $url"
        return 0
    else
        log_fail "$name is not responding at $url"
        return 1
    fi
}

# ============================================
# Test Suite
# ============================================

echo "================================================"
echo "  Bangumi E2E Integration Test"
echo "  $(date)"
echo "================================================"
echo ""

# Test 1: Service Health Checks
echo "=== Test 1: Service Health Checks ==="

CORE_OK=false
FETCHER_OK=false
DOWNLOADER_OK=false

if check_service "Core Service" "$CORE_URL" "/services"; then
    CORE_OK=true
fi

if check_service "Fetcher Service" "$FETCHER_URL" "/health"; then
    FETCHER_OK=true
fi

if check_service "Downloader Service" "$DOWNLOADER_URL" "/health"; then
    DOWNLOADER_OK=true
else
    log_skip "Downloader tests will be skipped"
fi

echo ""

# Abort if core services are down
if [ "$CORE_OK" = false ] || [ "$FETCHER_OK" = false ]; then
    log_error "Core or Fetcher service is not running. Aborting."
    exit 1
fi

# Test 2: Service Registration
echo "=== Test 2: Service Registration ==="

SERVICES=$(curl -s "$CORE_URL/services" 2>/dev/null)
FETCHER_COUNT=$(echo "$SERVICES" | jq '.services | map(select(.service_type == "fetcher")) | length' 2>/dev/null || echo "0")

if [ "$FETCHER_COUNT" -gt 0 ]; then
    log_pass "Found $FETCHER_COUNT registered fetcher(s)"
    echo "$SERVICES" | jq -r '.services[] | "  - \(.service_name) (\(.service_type)) at \(.host):\(.port)"' 2>/dev/null
else
    log_warn "No fetchers registered in Core's registry"
fi

echo ""

# Test 3: Fetcher Can-Handle Check
echo "=== Test 3: Fetcher Can-Handle Check ==="

CAN_HANDLE=$(curl -s -X POST "$FETCHER_URL/can-handle-subscription" \
    -H "Content-Type: application/json" \
    -d "{\"source_url\":\"$TEST_RSS_URL\",\"source_type\":\"rss\"}" 2>/dev/null)

if echo "$CAN_HANDLE" | jq -e '.can_handle == true' > /dev/null 2>&1; then
    log_pass "Fetcher can handle mikanani.me RSS"
else
    log_fail "Fetcher cannot handle the test RSS URL"
    echo "  Response: $CAN_HANDLE"
fi

echo ""

# Test 4: Create Subscription
echo "=== Test 4: Create Subscription ==="

CREATE_RESULT=$(curl -s -X POST "$CORE_URL/subscriptions" \
    -H "Content-Type: application/json" \
    -d "{\"source_url\":\"$TEST_RSS_URL\",\"name\":\"$TEST_SUBSCRIPTION_NAME\",\"fetch_interval_minutes\":60,\"source_type\":\"rss\"}" 2>/dev/null)

SUBSCRIPTION_ID=$(echo "$CREATE_RESULT" | jq -r '.id // .subscription_id // empty' 2>/dev/null)

if [ -n "$SUBSCRIPTION_ID" ] && [ "$SUBSCRIPTION_ID" != "null" ]; then
    log_pass "Created subscription with ID: $SUBSCRIPTION_ID"
    echo "  Name: $TEST_SUBSCRIPTION_NAME"
else
    ERROR_MSG=$(echo "$CREATE_RESULT" | jq -r '.message // .error // "Unknown error"' 2>/dev/null)
    log_fail "Failed to create subscription: $ERROR_MSG"
    echo "  Full response: $CREATE_RESULT"
    SUBSCRIPTION_ID=""
fi

echo ""

# Test 5: Trigger Fetch (if subscription was created)
if [ -n "$SUBSCRIPTION_ID" ]; then
    echo "=== Test 5: Trigger Fetch ==="

    FETCH_RESULT=$(curl -s -X POST "$FETCHER_URL/fetch" \
        -H "Content-Type: application/json" \
        -d "{\"subscription_id\":$SUBSCRIPTION_ID,\"rss_url\":\"$TEST_RSS_URL\",\"callback_url\":\"$CORE_URL/raw-fetcher-results\"}" 2>/dev/null)

    if echo "$FETCH_RESULT" | jq -e '.accepted == true' > /dev/null 2>&1; then
        log_pass "Fetch task accepted"

        # Wait for async fetch to complete
        log_info "Waiting for fetch to complete (5 seconds)..."
        sleep 5

        # Check subscription status
        SUB_STATUS=$(curl -s "$CORE_URL/subscriptions" 2>/dev/null | \
            jq ".subscriptions[] | select(.id == $SUBSCRIPTION_ID)" 2>/dev/null)

        LAST_FETCHED=$(echo "$SUB_STATUS" | jq -r '.last_fetched_at // "never"' 2>/dev/null)
        FETCH_COUNT=$(echo "$SUB_STATUS" | jq -r '.fetch_count // 0' 2>/dev/null)

        if [ "$LAST_FETCHED" != "never" ] && [ "$LAST_FETCHED" != "null" ]; then
            log_pass "Fetch completed successfully"
            echo "  Last fetched: $LAST_FETCHED"
            echo "  Fetch count: $FETCH_COUNT"
        else
            log_warn "Fetch may not have completed yet (async)"
            echo "  Status: $SUB_STATUS"
        fi
    else
        log_fail "Fetch task was not accepted"
        echo "  Response: $FETCH_RESULT"
    fi

    echo ""
fi

# Test 6: Downloader Test (optional)
if [ "$DOWNLOADER_OK" = true ]; then
    echo "=== Test 6: Downloader Test ==="

    # Test with a fake magnet link (will fail at qBittorrent but tests the endpoint)
    DOWNLOAD_RESULT=$(curl -s -X POST "$DOWNLOADER_URL/download" \
        -H "Content-Type: application/json" \
        -d '{"link_id":999,"url":"magnet:?xt=urn:btih:0000000000000000000000000000000000000000"}' 2>/dev/null)

    if echo "$DOWNLOAD_RESULT" | jq -e '.status' > /dev/null 2>&1; then
        log_pass "Downloader endpoint responded"
        echo "  Status: $(echo "$DOWNLOAD_RESULT" | jq -r '.status')"
    else
        log_fail "Downloader endpoint failed"
        echo "  Response: $DOWNLOAD_RESULT"
    fi

    echo ""
else
    echo "=== Test 6: Downloader Test ==="
    log_skip "Downloader service not available"
    echo ""
fi

# Test 7: List Subscriptions
echo "=== Test 7: List Subscriptions ==="

SUBS=$(curl -s "$CORE_URL/subscriptions" 2>/dev/null)
SUB_COUNT=$(echo "$SUBS" | jq '.subscriptions | length' 2>/dev/null || echo "0")

if [ "$SUB_COUNT" -gt 0 ]; then
    log_pass "Listed $SUB_COUNT subscription(s)"
    echo "$SUBS" | jq -r '.subscriptions[] | "  - [\(.id)] \(.name // .source_url)"' 2>/dev/null | head -5
else
    log_warn "No subscriptions found"
fi

echo ""

# Cleanup (optional)
if [ "$CLEANUP" = "--cleanup" ] && [ -n "$SUBSCRIPTION_ID" ]; then
    echo "=== Cleanup ==="
    # Note: Add delete endpoint call here if available
    log_info "Cleanup requested but delete endpoint not implemented"
    echo ""
fi

# Summary
echo "================================================"
echo "  Test Summary"
echo "================================================"
echo -e "  ${GREEN}Passed:${NC}  $PASSED"
echo -e "  ${RED}Failed:${NC}  $FAILED"
echo -e "  ${YELLOW}Skipped:${NC} $SKIPPED"
echo ""

if [ $FAILED -gt 0 ]; then
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
else
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
fi
