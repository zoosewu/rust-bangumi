.PHONY: help dev-infra dev-infra-down dev-setup dev-run db-migrate db-migrate-redo db-migrate-list build test lint check fmt cli

# 顏色輸出
YELLOW := \033[0;33m
GREEN := \033[0;32m
BLUE := \033[0;34m
NC := \033[0m # No Color

help:
	@echo "$(YELLOW)Bangumi 開發指令$(NC)"
	@echo ""
	@echo "$(BLUE)開發環境:$(NC)"
	@echo "  $(GREEN)make dev-infra$(NC)      - 啟動開發基礎設施 (PostgreSQL + Adminer)"
	@echo "  $(GREEN)make dev-infra-down$(NC) - 停止開發基礎設施"
	@echo "  $(GREEN)make dev-setup$(NC)      - 檢測環境並自動配置（host/container）"
	@echo "  $(GREEN)make dev-run$(NC)        - 完整開發啟動（環境設定 + 遷移 + 服務）"
	@echo ""
	@echo "$(BLUE)資料庫遷移 (Diesel):$(NC)"
	@echo "  $(GREEN)make db-migrate$(NC)     - 執行所有待執行的遷移"
	@echo "  $(GREEN)make db-migrate-redo$(NC) - 回滾並重新執行最後一次遷移"
	@echo "  $(GREEN)make db-migrate-list$(NC) - 查看遷移狀態"
	@echo ""
	@echo "$(BLUE)Rust 構建與測試:$(NC)"
	@echo "  $(GREEN)make build$(NC)          - 構建所有項目"
	@echo "  $(GREEN)make test$(NC)           - 運行所有測試"
	@echo "  $(GREEN)make lint$(NC)           - Clippy 靜態檢查"
	@echo "  $(GREEN)make fmt$(NC)            - 格式化代碼"
	@echo "  $(GREEN)make check$(NC)          - 運行所有檢查 (fmt + lint + build)"
	@echo ""
	@echo "$(BLUE)CLI:$(NC)"
	@echo "  $(GREEN)make cli$(NC)            - 運行 CLI 工具 (ARGS=\"--help\")"
	@echo ""

# ============================================================================
# 開發環境
# ============================================================================

dev-infra:
	@echo "$(YELLOW)啟動開發基礎設施...$(NC)"
	docker compose -f docker-compose.dev.yaml up -d
	@echo "$(GREEN)✓ PostgreSQL 運行在 localhost:5432$(NC)"
	@echo "$(GREEN)✓ Adminer 運行在 http://localhost:8081$(NC)"

dev-infra-down:
	@echo "$(YELLOW)停止開發基礎設施...$(NC)"
	docker compose -f docker-compose.dev.yaml down

dev-setup:
	@bash scripts/setup-dev-env.sh

dev-run:
	@bash scripts/dev-run.sh

# ============================================================================
# 資料庫遷移 (Diesel)
# ============================================================================

db-migrate:
	@echo "$(YELLOW)執行資料庫遷移...$(NC)"
	cd core-service && diesel migration run

db-migrate-redo:
	@echo "$(YELLOW)回滾並重新執行最後一次遷移...$(NC)"
	cd core-service && diesel migration redo

db-migrate-list:
	@echo "$(YELLOW)查看遷移狀態...$(NC)"
	cd core-service && diesel migration list

# ============================================================================
# Rust 構建與測試
# ============================================================================

build:
	@echo "$(YELLOW)構建所有項目...$(NC)"
	cargo build

test:
	@echo "$(YELLOW)運行測試...$(NC)"
	cargo test

lint:
	@echo "$(YELLOW)Clippy 靜態檢查...$(NC)"
	cargo clippy --all-targets --all-features

fmt:
	@echo "$(YELLOW)格式化代碼...$(NC)"
	cargo fmt --all

check: fmt lint build
	@echo "$(GREEN)✓ 所有檢查通過$(NC)"

# ============================================================================
# CLI
# ============================================================================

cli:
	@cargo run --package bangumi-cli -- $(ARGS)
