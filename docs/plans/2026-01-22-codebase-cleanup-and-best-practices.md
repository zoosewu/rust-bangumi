# 代碼庫清理和最佳實踐實現計劃

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 清理代碼庫中的重複文件、重組模塊結構、統一最佳實踐，並建立完整的測試框架。

**Architecture:** 通過系統性地整合重複的模型定義、刪除臨時文件、重組模塊結構、統一導出慣例，最終形成清晰的分層架構（shared 層 → 服務層 → 應用層）。

**Tech Stack:** Rust + Cargo Workspace，遵循官方最佳實踐。

---

## 任務清單

### 優先度 1：刪除臨時文件和整合重複模型

**Task 1.1: 刪除臨時 test_rss.rs 文件**
- Delete: `/nodejs/rust-bangumi/test_rss.rs`
- Reason: 重複於 `/fetchers/mikanani/examples/test_rss.rs`
- Verify: 確認 examples 版本保留且完整

**Task 1.2: 整合 shared 模型定義**
- Create consolidated `shared/src/models.rs` with all types:
  - Core shared types (AnimeMetadata, AnimeLink, FilterRule, etc.)
  - CLI specific types (ListResponse, SuccessResponse)
  - Service registration types
- Ensure backward compatibility with existing code

**Task 1.3: 清理 core-service db/models.rs**
- 保留僅 Diesel ORM 相關的類型
- 移除與 shared 重複的型別
- 更新 imports

**Task 1.4: 刪除冗餘 cli/src/models.rs**
- 將依賴轉換為 `use shared::models::*`
- 保留任何 CLI 特定的本地類型

### 優先度 2：重組模塊結構

**Task 2.1: 重構 core-service 模塊佈局**
- Reorganize `core-service/src/`
- Move CRUD operations from models/ to db/operations.rs
- Update all imports

**Task 2.2: 統一 lib.rs 導出慣例**
- Update all workspace crates' lib.rs
- Implement consistent `pub use` patterns
- Ensure convenient access to public APIs

**Task 2.3: 優化 mod.rs 結構**
- Review all mod.rs files for clarity
- Remove redundant re-exports
- Add module documentation

### 優先度 3：完善測試框架

**Task 3.1: 建立統一的測試配置**
- Create `tests/common/mod.rs` in core-service
- Implement test fixtures and helpers
- Document testing conventions

**Task 3.2: 添加 shared 模塊測試**
- Create `shared/tests/`
- Test model serialization/deserialization
- Test error types

**Task 3.3: 統一測試編寫標準**
- Document testing best practices
- Add test naming conventions
- Create test templates

**Task 3.4: 擴展測試覆蓋**
- Add bench tests for performance-critical code
- Add property-based tests where applicable
- Improve edge case coverage

### 優先度 4：文檔和驗證

**Task 4.1: 更新 README 和結構文檔**
- Document new module organization
- Add architecture diagrams
- Include module dependency graph

**Task 4.2: 編譯和測試驗證**
- Run full test suite
- Verify all crates compile
- Check for unused imports/code

**Task 4.3: 建立 skill 實現最佳實踐**
- Create `codebase-maintenance-skill`
- Automate code quality checks
- Document maintenance procedures

---

## 詳細實現步驟

### Phase 1: 刪除和整合（3-4 個子任務）

#### Step 1.1.1: 刪除 test_rss.rs
```bash
rm /nodejs/rust-bangumi/test_rss.rs
git add -A
```

#### Step 1.2.1: 整合 shared/src/models.rs
- Review current `shared/src/models.rs` (221 行)
- Review `cli/src/models.rs` (140 行)
- Review `core-service/src/db/models.rs` (503 行)
- Merge all non-conflicting types into shared
- Keep Diesel-specific types in core-service

#### Step 1.3.1: 清理 core-service/src/db/models.rs
- Remove all types that exist in shared
- Keep Diesel Queryable/Insertable pairs
- Update module documentation

#### Step 1.4.1: 更新 cli/src/models.rs
- Replace with `pub use shared::models::*;`
- Keep only CLI-specific local types if any
- Update all internal imports

### Phase 2: 模塊重組（2-3 個子任務）

#### Step 2.1.1: 重構 core-service 模塊
```
Before:
core-service/src/
├── models/
│   ├── mod.rs (導出)
│   └── db.rs (CRUD - 504 行)
└── db/
    ├── mod.rs (連接池)
    └── models.rs (ORM 類型)

After:
core-service/src/
├── db/
│   ├── mod.rs (連接池)
│   ├── models.rs (ORM 類型)
│   ├── schema.rs (表定義)
│   └── operations.rs (CRUD - 改名)
└── models/
    └── mod.rs (導出 API 類型)
```

#### Step 2.2.1: 統一導出慣例
```rust
// 模式：core-service/src/lib.rs
pub mod db;
pub mod handlers;
pub mod services;
pub mod state;

// 便利導出主要類型
pub use db::{Database, DbPool};
pub use state::AppState;
```

#### Step 2.3.1: 優化 mod.rs
- 為每個模塊添加文檔注釋
- 刪除冗餘的中間層 mod.rs
- 確保清晰的導出層次

### Phase 3: 測試框架（2-3 個子任務）

#### Step 3.1.1: 創建共享測試工具
```
core-service/tests/
├── common/
│   ├── mod.rs
│   ├── fixtures.rs (測試數據)
│   └── helpers.rs (測試函數)
└── integration_*.rs
```

#### Step 3.2.1: 添加 shared 模塊測試
```
shared/tests/
├── models_test.rs (序列化測試)
├── errors_test.rs (錯誤類型測試)
└── integration_test.rs
```

#### Step 3.3.1: 文檔化測試標準
- 創建 `docs/TESTING.md`
- 記錄命名慣例
- 提供測試模板

#### Step 3.4.1: 擴展覆蓋
- 確定高優先級的性能關鍵代碼
- 添加基準測試
- 添加邊界情況測試

### Phase 4: 驗證和完善（2 個子任務）

#### Step 4.1.1: 更新文檔
- 更新 README.md
- 創建 `docs/ARCHITECTURE.md`
- 添加模塊依賴圖

#### Step 4.2.1: 完整驗證
- 運行 `cargo build` (所有工作區)
- 運行 `cargo test` (完整測試套件)
- 運行 `cargo clippy` (代碼質量檢查)
- 檢查 `cargo doc --no-deps` (文檔生成)

---

## Skill 實現計劃

### 新 Skill: `codebase-maintenance`

**功能**：
1. **代碼質量檢查**
   - `codebase-maintenance check` - 檢查重複代碼
   - `codebase-maintenance lint` - 運行 clippy
   - `codebase-maintenance format` - 格式化代碼

2. **測試管理**
   - `codebase-maintenance test` - 運行完整測試套件
   - `codebase-maintenance coverage` - 生成覆蓋報告
   - `codebase-maintenance bench` - 運行基準測試

3. **文檔生成**
   - `codebase-maintenance docs` - 生成 API 文檔
   - `codebase-maintenance graph` - 生成依賴圖
   - `codebase-maintenance report` - 生成代碼報告

4. **預提交鉤子**
   - 集成到 git hooks
   - 自動檢查提交質量
   - 防止低質量代碼進入版本庫

**實現方式**：
```toml
# Cargo.toml 額外依賴
[dev-dependencies]
cargo-tarpaulin = "0.29"  # 代碼覆蓋
criterion = "0.5"          # 基準測試
proptest = "1.4"          # 屬性測試
```

---

## 預期成果

### 代碼質量改進
- ✅ 代碼重複減少 30%+ (models.rs 合併)
- ✅ 模塊清晰度提高 (統一導出)
- ✅ 測試覆蓋提高 (新增測試)

### 可維護性提升
- ✅ 新開發者上手時間減少 (清晰結構)
- ✅ 代碼審查效率提高 (一致的風格)
- ✅ Bug 修復時間減少 (減少重複)

### 文檔完整性
- ✅ 架構文檔完成
- ✅ 測試標準文檔化
- ✅ API 文檔自動生成

### 編譯驗證
- ✅ 所有 crates 編譯成功
- ✅ 無警告或最小化警告
- ✅ 測試全數通過

---

## 執行策略

**預估時間**：每個任務 10-15 分鐘，總計 2-3 小時
**依賴關係**：按優先度順序執行（1 → 2 → 3 → 4）
**驗證方式**：每個任務完成後運行 `cargo check` 和 `cargo test`

