#!/bin/bash
# 開發環境一鍵啟動腳本

set -e

echo "🚀 啟動開發環境..."

# 檢查 Docker 是否安裝
if ! command -v docker &> /dev/null; then
    echo "❌ Docker 未安裝"
    exit 1
fi

# 檢查 Docker Compose 是否安裝
if ! docker compose version &> /dev/null; then
    echo "❌ Docker Compose 未安裝"
    exit 1
fi

# 啟動開發基礎設施
echo "📦 啟動 PostgreSQL 和 Adminer..."
docker compose -f docker-compose.dev.yaml up -d

# 等待 PostgreSQL 就緒
echo "⏳ 等待 PostgreSQL 就緒..."
sleep 2

# 檢查 PostgreSQL 狀態
if docker compose -f docker-compose.dev.yaml ps postgres | grep -q "healthy"; then
    echo "✅ PostgreSQL 就緒"
else
    echo "⚠️  PostgreSQL 尚未完全就緒，請稍候..."
fi

echo ""
echo "✨ 開發環境已啟動！"
echo ""
echo "📍 服務位置："
echo "   • PostgreSQL:  localhost:5432"
echo "   • Adminer:     http://localhost:8081"
echo ""
echo "📝 後續步驟："
echo "   1. 複製環境配置: cp .env.example .env (如果需要)"
echo "   2. 在新終端啟動核心服務: cargo run --package core-service"
echo "   3. 在新終端啟動 Fetcher:  cargo run --package fetcher-mikanani"
echo ""
echo "🛑 停止開發環境: docker compose -f docker-compose.dev.yaml down"
echo ""
