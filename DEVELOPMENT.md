# 開發指南

## 開發環境設置

### 安裝依賴

```bash
# 安裝 Rust（如果還未安裝）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安裝必要工具
cargo install cargo-watch
cargo install diesel_cli --no-default-features --features postgres

# 安裝 Docker & Docker Compose
# 詳見 https://docs.docker.com/get-docker/
```

### 本地開發設置

```bash
# 複製環境配置
cp .env.example .env

# 啟動 Docker 開發基礎設施（PostgreSQL + Adminer）
make dev-infra

# 執行數據庫遷移
DATABASE_URL="postgresql://bangumi:bangumi_dev_password@localhost:5432/bangumi" \
diesel migration run
```

## 常用命令

### 構建

```bash
# 構建所有項目
cargo build

# 構建特定項目
cargo build --package core-service
cargo build --package bangumi-cli --release

# 檢查編譯
cargo check

# 格式化代碼
cargo fmt

# Lint 檢查
cargo clippy
```

### 運行

```bash
# 運行核心服務
cargo run --package core-service

# 運行 fetcher
CORE_SERVICE_URL=http://localhost:8000 \
cargo run --package fetcher-mikanani

# 運行 CLI
cargo run --package bangumi-cli -- --help
```

### 測試

```bash
# 運行所有測試
cargo test

# 運行特定測試
cargo test --package shared

# 帶輸出的測試
cargo test -- --nocapture

# 運行集成測試
cargo test --test '*'
```

### 監視文件變更

```bash
# 監視源代碼並自動重新編譯
cargo watch -x build

# 監視並運行測試
cargo watch -x test
```

## 項目架構說明

### 共享庫 (shared)

包含所有微服務共用的數據結構、錯誤類型、API 常數等。

- `models.rs` - 所有 Struct 定義（ServiceRegistration、FetchResponse 等）
- `errors.rs` - 統一的錯誤類型 AppError
- `api.rs` - API 路由、header、默認值等常數

### 核心服務 (core-service)

主協調服務，負責：
- PostgreSQL 連接與數據庫操作
- REST API 服務
- 服務註冊與健康檢查
- Cron 調度任務
- 過濾規則應用
- 錯誤處理與重試

模塊結構：
```
src/
├── main.rs          # 應用入口、路由定義
├── config.rs        # 配置管理
├── handlers/        # HTTP 請求處理器
├── services/        # 業務邏輯
├── models/          # 本地數據模型
└── db/              # 數據庫操作層
```

### 微服務（Fetcher、Downloader、Viewer）

各微服務的基本模式：

1. **啟動時向核心服務註冊自己**
2. **暴露相應的 REST 端點**
3. **接收來自核心服務的請求**
4. **執行具體業務邏輯**
5. **回報結果或進度**

## 編碼規範

### Rust 風格

- 遵循 [Rust API 指南](https://rust-lang.github.io/api-guidelines/)
- 使用 `cargo fmt` 格式化代碼
- 使用 `cargo clippy` 檢查代碼質量

### 命名慣例

- 模塊名：snake_case
- 類型名：PascalCase
- 函數名：snake_case
- 常數名：SCREAMING_SNAKE_CASE

### 錯誤處理

使用 `AppError` 統一錯誤類型：

```rust
use shared::{Result, AppError};

async fn my_handler() -> Result<Json<MyResponse>> {
    let data = fetch_data()
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    Ok(Json(data))
}
```

### 日誌

使用 `tracing` 庫記錄日誌：

```rust
tracing::debug!("調試信息");
tracing::info!("信息");
tracing::warn!("警告");
tracing::error!("錯誤");
```

## 數據庫遷移

本專案使用 **Diesel ORM** 進行資料庫遷移管理。

### 創建新遷移

```bash
# 創建新遷移（自動生成時間戳和目錄結構）
diesel migration generate my_migration_name

# 編輯生成的文件
vim migrations/TIMESTAMP_my_migration_name/up.sql    # 上升路徑
vim migrations/TIMESTAMP_my_migration_name/down.sql  # 下降路徑
```

### 執行遷移

```bash
# 執行所有待執行的遷移
DATABASE_URL="postgresql://bangumi:bangumi_dev_password@localhost:5432/bangumi" \
diesel migration run

# 或如果已設置環境變數
diesel migration run
```

### 回滾遷移

```bash
# 回滾最後一個遷移
diesel migration redo

# 回滾所有遷移
diesel migration revert --all
```

## 測試

### 單元測試

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        assert_eq!(2 + 2, 4);
    }
}
```

### 集成測試

在 `tests/` 目錄下創建測試文件。

### 測試覆蓋率

```bash
# 安裝 tarpaulin
cargo install cargo-tarpaulin

# 生成覆蓋率報告
cargo tarpaulin --out Html
```

## Docker 開發

### 構建單個服務鏡像

```bash
docker build -f Dockerfile.core -t bangumi-core .
docker build -f Dockerfile.fetcher-mikanani -t bangumi-fetcher-mikanani .
```

### 用 Docker Compose 開發

```bash
# 啟動特定服務
docker compose up core-service postgres

# 重建服務鏡像
docker compose build --no-cache

# 進入容器 shell
docker compose exec core-service bash

# 查看服務日誌
docker compose logs -f core-service
```

## 常見問題

### 編譯错误：某些依賴無法解析

確保所有依賴都在 `Cargo.toml` 中定義，特別是 `workspace.dependencies` 部分。

### 數據庫連接失敗

檢查 `DATABASE_URL` 環境變數是否正確設置，確保 PostgreSQL 服務正常運行。

### Docker 容器無法通信

確保所有容器都在同一個 Docker 網絡中（`bangumi-network`），使用服務名稱而不是 localhost 進行通信。

## 性能優化

### 編譯優化

使用 `--release` 標誌進行發佈構建：

```bash
cargo build --release
cargo run --release
```

### 數據庫查詢優化

- 添加適當的索引
- 使用 `EXPLAIN` 分析查詢計劃
- 避免 N+1 查詢問題

## 貢獻指南

1. 創建特性分支：`git checkout -b feature/my-feature`
2. 提交更改：`git commit -am 'Add my feature'`
3. 推送分支：`git push origin feature/my-feature`
4. 提交 PR 描述

確保所有測試通過且代碼符合規範。

