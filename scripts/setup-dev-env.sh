#!/bin/bash
# 開發環境自動設定腳本
# 功能：檢測環境（host/container），如果是 container 則自動連接到 bangumi-dev-network

set -e

CONTAINER_DEV_NETWORK="workspace_bangumi-dev-network"
POSTGRES_HOST="bangumi-postgres-dev"
POSTGRES_PORT="5432"
POSTGRES_DB="bangumi"
POSTGRES_USER="bangumi"
POSTGRES_PASSWORD="bangumi_dev_password"

# ============================================================================
# 環境檢測
# ============================================================================

is_in_docker() {
    # 方法 1：檢查 /.dockerenv 檔案
    if [ -f "/.dockerenv" ]; then
        return 0
    fi
    
    # 方法 2：檢查 /proc/self/cgroup
    if grep -q docker /proc/self/cgroup 2>/dev/null; then
        return 0
    fi
    
    return 1
}

# ============================================================================
# 主程式
# ============================================================================

echo "🔍 檢查運行環境..."

if is_in_docker; then
    echo "✓ 檢測到：Docker 容器環境"
    echo ""
    echo "📡 正在連接到開發網絡..."
    
    # 獲取當前容器名稱
    CURRENT_CONTAINER=$(hostname)
    echo "   當前容器：$CURRENT_CONTAINER"
    
    # 檢查是否已連接到網絡
    if docker network inspect "$CONTAINER_DEV_NETWORK" --format='{{range .Containers}}{{.Name}}{{end}}' 2>/dev/null | grep -q "$CURRENT_CONTAINER"; then
        echo "   ✓ 已連接到 $CONTAINER_DEV_NETWORK"
    else
        echo "   ⏳ 嘗試連接到 $CONTAINER_DEV_NETWORK..."
        if docker network connect "$CONTAINER_DEV_NETWORK" "$CURRENT_CONTAINER" 2>/dev/null; then
            echo "   ✓ 成功連接到 $CONTAINER_DEV_NETWORK"
        else
            echo "   ⚠️ 無法自動連接網絡（可能已連接或無 Docker socket 權限）"
        fi
    fi
    
    # 設置環境變數（使用容器內 DNS 名稱）
    export DATABASE_URL="postgresql://$POSTGRES_USER:$POSTGRES_PASSWORD@$POSTGRES_HOST:$POSTGRES_PORT/$POSTGRES_DB"
    echo ""
    echo "📝 資料庫連接字串："
    echo "   $DATABASE_URL"
    
else
    echo "✓ 檢測到：Host 環境（WSL2/本地機器）"
    echo ""
    echo "📝 資料庫連接字串："
    echo "   postgresql://$POSTGRES_USER:$POSTGRES_PASSWORD@127.0.0.1:$POSTGRES_PORT/$POSTGRES_DB"
    
    export DATABASE_URL="postgresql://$POSTGRES_USER:$POSTGRES_PASSWORD@127.0.0.1:$POSTGRES_PORT/$POSTGRES_DB"
fi

echo ""
echo "✨ 開發環境設定完成"
echo ""
