# Core Service

Bangumi 系統的核心服務，負責管理動畫資料、訂閱、解析器等功能。

## 環境需求

- Rust (推薦使用 rustup 安裝)
- PostgreSQL 14+
- Diesel CLI

## 安裝 Diesel CLI

```bash
cargo install diesel_cli --no-default-features --features postgres
```

## 資料庫設定

### 1. 環境變數

在 `core-service/` 目錄下建立 `.env` 檔案：

```env
DATABASE_URL=postgresql://username:password@host:port/database_name
```

範例：
```env
DATABASE_URL=postgresql://bangumi:bangumi_password@localhost:5432/bangumi
```

### 2. 建立資料庫

```bash
# 使用 psql 建立資料庫
createdb -U postgres bangumi

# 或進入 psql 後執行
psql -U postgres
CREATE DATABASE bangumi;
CREATE USER bangumi WITH PASSWORD 'your_password';
GRANT ALL PRIVILEGES ON DATABASE bangumi TO bangumi;
```

## 資料庫遷移 (Database Migration)

### 執行遷移

有兩種方式可以執行資料庫遷移：

#### 方法一：使用 Diesel CLI (推薦用於開發)

```bash
cd core-service

# 執行所有待處理的遷移
diesel migration run

# 查看遷移狀態
diesel migration list

# 回滾最後一次遷移
diesel migration revert

# 回滾所有遷移
diesel migration revert --all

# 重新執行遷移（回滾後再執行）
diesel migration redo
```

#### 方法二：程式啟動時自動遷移

應用程式啟動時會自動執行待處理的遷移。這是透過 `db::run_migrations()` 函數實現的。

### 建立新遷移

```bash
cd core-service

# 建立新的遷移檔案
diesel migration generate migration_name
```

這會在 `migrations/` 目錄下建立一個新的資料夾，包含：
- `up.sql` - 執行遷移的 SQL
- `down.sql` - 回滾遷移的 SQL

### 更新 Schema

執行遷移後，需要更新 Rust schema 檔案：

```bash
cd core-service

# 從資料庫產生 schema.rs
diesel print-schema > src/schema.rs
```

## 專案結構

```
core-service/
├── migrations/           # 資料庫遷移檔案
│   └── YYYY-MM-DD-NNNNNN-name/
│       ├── up.sql
│       └── down.sql
├── src/
│   ├── db/
│   │   ├── mod.rs        # 資料庫連線和遷移
│   │   ├── models.rs     # Diesel models (re-export)
│   │   └── repository/   # Repository pattern 實作
│   ├── handlers/         # HTTP handlers
│   ├── models/           # 資料模型
│   ├── schema.rs         # Diesel schema (自動產生)
│   ├── state.rs          # AppState 和 Repositories
│   └── main.rs
├── tests/                # 整合測試
├── .env                  # 環境變數 (不要提交到 git)
├── diesel.toml           # Diesel 設定
└── Cargo.toml
```

## 執行服務

```bash
cd core-service

# 開發模式
cargo run

# 釋出模式
cargo run --release
```

## 測試

```bash
cd core-service

# 執行所有測試（不含需要資料庫的整合測試）
cargo test

# 執行包含整合測試（需要測試資料庫）
DATABASE_TEST_URL=postgresql://user:pass@localhost:5432/bangumi_test cargo test -- --include-ignored

# 只執行單元測試
cargo test --lib
```

## Repository Pattern

本專案使用 Repository Pattern 來抽象資料庫操作，方便進行單元測試。

### 使用範例

```rust
// 在 handler 中使用 repository
pub async fn get_anime(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    match state.repos.anime.find_by_id(id).await {
        Ok(Some(anime)) => Json(anime).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
```

### 可用的 Repositories

| Repository | 說明 |
|------------|------|
| `anime` | 動畫資料 |
| `subscription` | RSS 訂閱 |
| `service_module` | 服務模組（Fetcher、Parser 等）|
| `season` | 季度 |
| `anime_series` | 動畫系列 |
| `subtitle_group` | 字幕組 |
| `filter_rule` | 過濾規則 |
| `anime_link` | 動畫連結 |
| `title_parser` | 標題解析器 |
| `raw_item` | 原始項目 |
| `conflict` | 訂閱衝突 |

## 常見問題

### Q: 遷移失敗怎麼辦？

1. 檢查 `DATABASE_URL` 是否正確
2. 確認資料庫服務正在執行
3. 確認使用者有足夠的權限
4. 查看詳細錯誤訊息：`diesel migration run --locked-schema`

### Q: schema.rs 和資料庫不同步？

```bash
# 重新產生 schema.rs
diesel print-schema > src/schema.rs
```

### Q: 如何重置資料庫？

```bash
# 回滾所有遷移
diesel migration revert --all

# 重新執行所有遷移
diesel migration run
```

或直接刪除並重建資料庫：

```bash
dropdb bangumi
createdb bangumi
diesel migration run
```
