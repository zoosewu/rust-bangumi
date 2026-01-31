# 整合 Service Modules 表 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 將三個分散的 service modules 表（fetcher_modules、downloader_modules、viewer_modules）合併為單一的 service_modules 表，並使用 enum 欄位來區分服務類別。

**Architecture:**
透過 migration 逐步合併表結構：
1. 建立新的 `service_modules` 表，包含 `module_type` enum 欄位
2. 將現有資料從三個舊表遷移到新表
3. 更新 Rust 模型和 schema 定義
4. 更新服務註冊邏輯以使用新表
5. 清理舊表

**Tech Stack:**
- PostgreSQL (ENUM 類型)
- Diesel ORM + Migrations
- Rust (Axum + Tokio)

---

## Task 1: 建立 ENUM 類型和新的 service_modules 表

**Files:**
- Create: `/workspace/core-service/migrations/2026-01-27-000004-consolidate-service-modules/up.sql`
- Create: `/workspace/core-service/migrations/2026-01-27-000004-consolidate-service-modules/down.sql`

**Step 1: 建立 migration 檔案目錄**

```bash
mkdir -p /workspace/core-service/migrations/2026-01-27-000004-consolidate-service-modules
```

**Step 2: 編寫 up.sql - 建立 ENUM 和新表**

在 `/workspace/core-service/migrations/2026-01-27-000004-consolidate-service-modules/up.sql` 中寫入：

```sql
-- Create ENUM type for module type
CREATE TYPE module_type AS ENUM ('fetcher', 'downloader', 'viewer');

-- Create the consolidated service_modules table
CREATE TABLE service_modules (
  module_id SERIAL PRIMARY KEY,
  module_type module_type NOT NULL,
  name VARCHAR(255) NOT NULL UNIQUE,
  version VARCHAR(50) NOT NULL,
  description TEXT,
  is_enabled BOOLEAN NOT NULL DEFAULT true,
  config_schema TEXT,
  priority INT NOT NULL DEFAULT 50,
  base_url VARCHAR(255) NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes
CREATE INDEX idx_service_modules_module_type ON service_modules(module_type);
CREATE INDEX idx_service_modules_base_url ON service_modules(base_url);
CREATE INDEX idx_service_modules_name_type ON service_modules(name, module_type);

-- Migrate data from fetcher_modules
INSERT INTO service_modules (module_type, name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at)
SELECT 'fetcher'::module_type, name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at
FROM fetcher_modules;

-- Migrate data from downloader_modules
INSERT INTO service_modules (module_type, name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at)
SELECT 'downloader'::module_type, name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at
FROM downloader_modules;

-- Migrate data from viewer_modules
INSERT INTO service_modules (module_type, name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at)
SELECT 'viewer'::module_type, name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at
FROM viewer_modules;
```

**Step 3: 編寫 down.sql - 回滾邏輯**

在 `/workspace/core-service/migrations/2026-01-27-000004-consolidate-service-modules/down.sql` 中寫入：

```sql
-- Restore data to old tables
INSERT INTO fetcher_modules (name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at)
SELECT name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at
FROM service_modules
WHERE module_type = 'fetcher'::module_type;

INSERT INTO downloader_modules (name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at)
SELECT name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at
FROM service_modules
WHERE module_type = 'downloader'::module_type;

INSERT INTO viewer_modules (name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at)
SELECT name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at
FROM service_modules
WHERE module_type = 'viewer'::module_type;

-- Drop the new table and enum type
DROP TABLE service_modules;
DROP TYPE module_type;
```

**Step 4: 驗證 migration 檔案已建立**

```bash
ls -la /workspace/core-service/migrations/2026-01-27-000004-consolidate-service-modules/
```

預期輸出：
```
total X
-rw-r--r-- 1 user group date time up.sql
-rw-r--r-- 1 user group date time down.sql
```

**Step 5: 執行 migration**

```bash
cd /workspace/core-service && diesel migration run
```

預期輸出：
```
Running migration 2026-01-27-000004-consolidate-service-modules
```

**Step 6: Commit**

```bash
git add /workspace/core-service/migrations/2026-01-27-000004-consolidate-service-modules/
git commit -m "$(cat <<'EOF'
feat: create service_modules table with module_type enum

- Create service_modules table consolidating fetcher, downloader, and viewer modules
- Add module_type ENUM (fetcher, downloader, viewer)
- Add indexes on module_type and base_url for performance
- Migrate existing data from three separate tables
- Add down.sql for migration rollback

Co-Authored-By: Claude Haiku 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: 更新 Diesel Schema 定義

**Files:**
- Modify: `/workspace/core-service/src/schema.rs`

**Step 1: 重新生成 schema.rs (Diesel 自動化)**

```bash
cd /workspace/core-service && diesel print-schema > src/schema.rs
```

驗證新增的 service_modules 表定義已出現在 schema.rs 中。

**Step 2: 手動驗證 schema.rs**

檢查 `/workspace/core-service/src/schema.rs` 包含：

```rust
diesel::table! {
    service_modules (module_id) {
        module_id -> Int4,
        #[max_length = 50]
        module_type -> module_type,  // 自定義 ENUM 類型
        #[max_length = 255]
        name -> Varchar,
        #[max_length = 50]
        version -> Varchar,
        description -> Nullable<Text>,
        is_enabled -> Bool,
        config_schema -> Nullable<Text>,
        priority -> Int4,
        #[max_length = 255]
        base_url -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}
```

如果 Diesel 未自動生成 ENUM 類型定義，需手動新增：

```rust
// 在 schema.rs 最上方加入
#[derive(diesel::sql_types::SqlType)]
#[postgres(type_name = "module_type")]
pub struct ModuleType;
```

**Step 3: Commit**

```bash
git add /workspace/core-service/src/schema.rs
git commit -m "$(cat <<'EOF'
refactor: regenerate schema.rs with service_modules table

- Add service_modules table definition
- Add module_type ENUM support for Diesel
- Remove old table definitions (will be cleaned up in separate task)

Co-Authored-By: Claude Haiku 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: 建立 Rust 模型和 ENUM 定義

**Files:**
- Modify: `/workspace/core-service/src/models/db.rs`

**Step 1: 在 db.rs 頂部定義 ModuleType ENUM**

在 `/workspace/core-service/src/models/db.rs` 的最開始（在所有其他 use 聲明之後）加入：

```rust
// ============ ServiceModule ENUM ============
#[derive(Debug, Clone, Copy, PartialEq, Eq, diesel::FromSql, diesel::AsExpression)]
#[diesel(sql_type = crate::schema::sql_types::ModuleType)]
pub enum ModuleTypeEnum {
    #[serde(rename = "fetcher")]
    Fetcher,
    #[serde(rename = "downloader")]
    Downloader,
    #[serde(rename = "viewer")]
    Viewer,
}

impl std::fmt::Display for ModuleTypeEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModuleTypeEnum::Fetcher => write!(f, "fetcher"),
            ModuleTypeEnum::Downloader => write!(f, "downloader"),
            ModuleTypeEnum::Viewer => write!(f, "viewer"),
        }
    }
}

impl From<&shared::ServiceType> for ModuleTypeEnum {
    fn from(service_type: &shared::ServiceType) -> Self {
        match service_type {
            shared::ServiceType::Fetcher => ModuleTypeEnum::Fetcher,
            shared::ServiceType::Downloader => ModuleTypeEnum::Downloader,
            shared::ServiceType::Viewer => ModuleTypeEnum::Viewer,
        }
    }
}
```

**Step 2: 在 db.rs 中新增 ServiceModule 模型**

在檔案中合適位置（靠近其他 modules 定義）加入：

```rust
// ============ ServiceModules ============
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = super::super::schema::service_modules)]
pub struct ServiceModule {
    pub module_id: i32,
    pub module_type: ModuleTypeEnum,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub is_enabled: bool,
    pub config_schema: Option<String>,
    pub priority: i32,
    pub base_url: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = super::super::schema::service_modules)]
pub struct NewServiceModule {
    pub module_type: ModuleTypeEnum,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub is_enabled: bool,
    pub config_schema: Option<String>,
    pub priority: i32,
    pub base_url: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
```

**Step 3: 測試編譯**

```bash
cd /workspace/core-service && cargo check
```

預期：無編譯錯誤

**Step 4: Commit**

```bash
git add /workspace/core-service/src/models/db.rs
git commit -m "$(cat <<'EOF'
feat: add ServiceModule model and ModuleTypeEnum

- Create ModuleTypeEnum with Fetcher, Downloader, Viewer variants
- Add ServiceModule and NewServiceModule Queryable structs
- Implement conversion from shared::ServiceType to ModuleTypeEnum
- Add Display trait for ModuleTypeEnum

Co-Authored-By: Claude Haiku 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: 更新服務註冊邏輯

**Files:**
- Modify: `/workspace/core-service/src/handlers/services.rs`

**Step 1: 更新註冊函數以使用 service_modules 表**

在 `/workspace/core-service/src/handlers/services.rs` 中，將 `register` 函數的 UPSERT 邏輯替換為單一查詢。

原始邏輯（行 47-78）：
```rust
let upsert_query = match &payload.service_type {
    ServiceType::Fetcher => { /* fetcher_modules */ },
    ServiceType::Downloader => { /* downloader_modules */ },
    ServiceType::Viewer => { /* viewer_modules */ },
};
```

新邏輯：
```rust
let module_type_enum = ModuleTypeEnum::from(&payload.service_type);
let module_type_str = module_type_enum.to_string();

let upsert_query = diesel::sql_query(
    "INSERT INTO service_modules (module_type, name, version, description, is_enabled, config_schema, created_at, updated_at, priority, base_url) \
     VALUES ($1::module_type, $2, $3, $4, $5, NULL, $6, $7, $8, $9) \
     ON CONFLICT (name) DO UPDATE SET \
     is_enabled = EXCLUDED.is_enabled, \
     base_url = EXCLUDED.base_url, \
     module_type = EXCLUDED.module_type, \
     updated_at = EXCLUDED.updated_at"
);

let upsert_query = upsert_query
    .bind::<diesel::sql_types::Text, _>(&module_type_str)
    .bind::<diesel::sql_types::Varchar, _>(&payload.service_name)
    .bind::<diesel::sql_types::Varchar, _>("1.0.0")
    .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(Some(service_description))
    .bind::<diesel::sql_types::Bool, _>(true)
    .bind::<diesel::sql_types::Timestamp, _>(naive_now)
    .bind::<diesel::sql_types::Timestamp, _>(naive_now)
    .bind::<diesel::sql_types::Int4, _>(50i32)
    .bind::<diesel::sql_types::Text, _>(&service_base_url);
```

**Step 2: 新增必要的 import**

在 `/workspace/core-service/src/handlers/services.rs` 頂部加入：

```rust
use crate::models::db::ModuleTypeEnum;
```

**Step 3: 測試編譯**

```bash
cd /workspace/core-service && cargo check
```

預期：無編譯錯誤

**Step 4: 運行現有測試**

```bash
cd /workspace/core-service && cargo test --test '*services*'
```

預期：所有測試通過

**Step 5: Commit**

```bash
git add /workspace/core-service/src/handlers/services.rs
git commit -m "$(cat <<'EOF'
refactor: update service registration to use service_modules table

- Replace three separate table UPSERT queries with single service_modules query
- Use module_type ENUM for service type classification
- Maintain backward compatibility with existing registration logic
- Add ModuleTypeEnum conversion from ServiceType

Co-Authored-By: Claude Haiku 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: 建立清理舊表的 Migration

**Files:**
- Create: `/workspace/core-service/migrations/2026-01-27-000005-drop-legacy-modules-tables/up.sql`
- Create: `/workspace/core-service/migrations/2026-01-27-000005-drop-legacy-modules-tables/down.sql`

**Step 1: 建立新 migration 目錄**

```bash
mkdir -p /workspace/core-service/migrations/2026-01-27-000005-drop-legacy-modules-tables
```

**Step 2: 編寫 up.sql - 刪除舊表**

在 `/workspace/core-service/migrations/2026-01-27-000005-drop-legacy-modules-tables/up.sql` 中寫入：

```sql
-- Drop indexes from old tables
DROP INDEX IF EXISTS idx_fetcher_modules_base_url;
DROP INDEX IF EXISTS idx_downloader_modules_base_url;
DROP INDEX IF EXISTS idx_viewer_modules_base_url;

-- Drop old tables
DROP TABLE IF EXISTS fetcher_modules;
DROP TABLE IF EXISTS downloader_modules;
DROP TABLE IF EXISTS viewer_modules;
```

**Step 3: 編寫 down.sql - 重建舊表（用於回滾測試）**

在 `/workspace/core-service/migrations/2026-01-27-000005-drop-legacy-modules-tables/down.sql` 中寫入：

```sql
-- Recreate fetcher_modules table
CREATE TABLE fetcher_modules (
  fetcher_id SERIAL PRIMARY KEY,
  name VARCHAR(255) NOT NULL UNIQUE,
  version VARCHAR(50) NOT NULL,
  description TEXT,
  is_enabled BOOLEAN NOT NULL DEFAULT true,
  config_schema TEXT,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  priority INT NOT NULL DEFAULT 50,
  base_url VARCHAR(255) NOT NULL
);

CREATE INDEX idx_fetcher_modules_base_url ON fetcher_modules(base_url);

-- Recreate downloader_modules table
CREATE TABLE downloader_modules (
  downloader_id SERIAL PRIMARY KEY,
  name VARCHAR(255) NOT NULL UNIQUE,
  version VARCHAR(50) NOT NULL,
  description TEXT,
  is_enabled BOOLEAN NOT NULL DEFAULT true,
  config_schema TEXT,
  priority INT NOT NULL DEFAULT 50,
  base_url VARCHAR(255) NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_downloader_modules_base_url ON downloader_modules(base_url);

-- Recreate viewer_modules table
CREATE TABLE viewer_modules (
  viewer_id SERIAL PRIMARY KEY,
  name VARCHAR(255) NOT NULL UNIQUE,
  version VARCHAR(50) NOT NULL,
  description TEXT,
  is_enabled BOOLEAN NOT NULL DEFAULT true,
  config_schema TEXT,
  priority INT NOT NULL DEFAULT 50,
  base_url VARCHAR(255) NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_viewer_modules_base_url ON viewer_modules(base_url);

-- Restore data from service_modules
INSERT INTO fetcher_modules (name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at)
SELECT name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at
FROM service_modules
WHERE module_type = 'fetcher'::module_type;

INSERT INTO downloader_modules (name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at)
SELECT name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at
FROM service_modules
WHERE module_type = 'downloader'::module_type;

INSERT INTO viewer_modules (name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at)
SELECT name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at
FROM service_modules
WHERE module_type = 'viewer'::module_type;
```

**Step 4: 執行新 migration**

```bash
cd /workspace/core-service && diesel migration run
```

預期輸出：
```
Running migration 2026-01-27-000005-drop-legacy-modules-tables
```

**Step 5: 驗證舊表已刪除**

```bash
cd /workspace/core-service && psql -c "\dt" | grep -E "fetcher_modules|downloader_modules|viewer_modules"
```

預期：無輸出（表已不存在）

**Step 6: Commit**

```bash
git add /workspace/core-service/migrations/2026-01-27-000005-drop-legacy-modules-tables/
git commit -m "$(cat <<'EOF'
feat: drop legacy service modules tables

- Drop fetcher_modules, downloader_modules, viewer_modules tables
- Remove associated indexes
- Add down.sql for rollback capability

Co-Authored-By: Claude Haiku 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: 清理 Rust 模型中的舊定義

**Files:**
- Modify: `/workspace/core-service/src/models/db.rs`

**Step 1: 刪除舊的 FetcherModule、DownloaderModule、ViewerModule 定義**

在 `/workspace/core-service/src/models/db.rs` 中刪除以下結構（約在第 185-272 行）：

```rust
// 刪除這些：
#[derive(Queryable, Selectable, Debug, Clone)]
pub struct FetcherModule { ... }

#[derive(Insertable)]
pub struct NewFetcherModule { ... }

#[derive(Queryable, Selectable, Debug, Clone)]
pub struct DownloaderModule { ... }

#[derive(Insertable)]
pub struct NewDownloaderModule { ... }

#[derive(Queryable, Selectable, Debug, Clone)]
pub struct ViewerModule { ... }

#[derive(Insertable)]
pub struct NewViewerModule { ... }
```

**Step 2: 驗證編譯**

```bash
cd /workspace/core-service && cargo check
```

如果有編譯錯誤，檢查是否有其他地方仍在使用舊模型。

**Step 3: 更新使用舊模型的地方**

搜尋文件中使用舊模型的位置：

```bash
cd /workspace/core-service && grep -r "FetcherModule\|DownloaderModule\|ViewerModule" src/ --include="*.rs" | grep -v "ServiceModule"
```

對每個找到的使用位置進行更新，使用新的 `ServiceModule` 類型。

**Step 4: 再次測試編譯**

```bash
cd /workspace/core-service && cargo test
```

預期：所有測試通過

**Step 5: Commit**

```bash
git add /workspace/core-service/src/models/db.rs
git commit -m "$(cat <<'EOF'
refactor: remove legacy service module models

- Delete FetcherModule, DownloaderModule, ViewerModule structs
- Remove corresponding Insertable traits
- All service module operations now use ServiceModule model

Co-Authored-By: Claude Haiku 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: 驗收和文檔更新

**Files:**
- Modify: `/workspace/ARCHITECTURE.md` (如果存在)
- Create/Modify: database schema documentation

**Step 1: 更新架構文檔**

如果存在架構文檔，更新其中關於 modules 表的描述，說明新的單一 service_modules 表設計。

**Step 2: 運行完整測試套件**

```bash
cd /workspace/core-service && cargo test
cd /workspace && npm test  # 如果有其他測試
```

預期：所有測試通過

**Step 3: 驗證 migration 可回滾**

```bash
cd /workspace/core-service && diesel migration redo
```

預期：migration 成功回滾和重新運行

**Step 4: Commit 最終更新**

```bash
git add /workspace/ARCHITECTURE.md  # 如果修改了
git commit -m "$(cat <<'EOF'
docs: update documentation for consolidated service modules

- Update architecture documentation
- Document service_modules table and module_type enum
- Reference new consolidated design

Co-Authored-By: Claude Haiku 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## 關鍵考慮事項

1. **資料遷移安全性**：在執行 Task 1 時，migration 會自動將資料從舊表遷移到新表。應在測試環境驗證資料完整性。

2. **ENUM 支援**：確保 PostgreSQL 版本支援自定義 ENUM 類型（PostgreSQL 8.3+）。

3. **Diesel 同步**：Task 2 需要執行 `diesel print-schema` 以自動生成 schema 定義。

4. **向後相容性**：在 Task 4 之前，舊的三個表仍然存在，所以現有查詢仍可正常工作。Task 5 清理舊表應在所有代碼都遷移到新表之後執行。

5. **回滾計畫**：所有 migration 都有 `down.sql` 以支持回滾。可使用 `diesel migration redo` 驗證。

---

## 測試驗證清單

- [ ] Migration 執行成功
- [ ] 資料完全遷移到 service_modules 表
- [ ] Rust 代碼編譯通過
- [ ] 服務註冊端點正常工作
- [ ] 所有測試通過
- [ ] Migration 可以成功回滾
- [ ] 資料庫索引按預期創建
