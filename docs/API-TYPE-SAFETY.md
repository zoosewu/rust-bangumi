# API Type Safety — 開發流程

## 概覽

本專案使用三層型別安全保護，確保 Rust 後端與 TypeScript 前端的 API contract 一致：

1. **Rust DTO → OpenAPI spec**：utoipa 的 `ToSchema` + `#[utoipa::path]` 從 Rust struct 自動生成 `/openapi.json`
2. **OpenAPI spec → TypeScript interfaces**：`openapi-typescript` 生成 `frontend/src/generated/api.ts`
3. **TypeScript interfaces → Effect Schema**：`AssertExtends<>` type assertion 在 `tsc` 時驗證一致性

若任一層漂移，`bun run typecheck` 會失敗，CI 阻擋合併。

## 新增 API endpoint 的標準流程

### 後端（Rust）

1. 在 `core-service/src/dto.rs` 新增 DTO struct，加上 `ToSchema` derive
2. 在對應 handler 加上 `#[utoipa::path(...)]` annotation
3. 在 `core-service/src/openapi.rs` 的 `#[openapi(...)]` 中加入新 path 和 schema
4. 執行 `cargo check -p core-service` 確認無錯誤
5. 執行 `cargo test -p core-service` 確認測試通過

### 前端（TypeScript）

6. 重新生成型別（使用 committed 的 `openapi-generated.json` 或 running server）：
   ```bash
   cd frontend && bun run generate:api
   ```
7. 在對應的 `frontend/src/schemas/*.ts` 加入 Effect Schema 定義
8. 加入 type assertion：
   ```typescript
   // eslint-disable-next-line @typescript-eslint/no-unused-vars
   type _CheckXxx = AssertExtends<components["schemas"]["XxxResponse"], XxxSchema>
   ```
9. 執行 `bun run typecheck` 確認無錯誤
10. 在 `frontend/src/layers/ApiLayer.ts` 加入新的 API 方法實作
11. 在 `frontend/src/services/CoreApi.ts` 加入 interface 方法簽名

### 提交

12. 後端改動 + 更新的 `docs/api/openapi-generated.json` + 更新的 `frontend/src/generated/api.ts` + 新的 Effect Schema 一起 commit

## CI 驗證步驟

```bash
# 後端
cargo check -p core-service
cargo test -p core-service

# 前端（使用 committed 的 api.ts，不需要 running server）
cd frontend
bun install
bun run typecheck  # 包含 AssertExtends 型別檢查
bun run test
```

## 型別漂移診斷

若 `bun run typecheck` 報錯 `Type 'X' does not satisfy the constraint 'Y'`：

1. 找到報錯的 `_Check*` assertion 行
2. 比對 `frontend/src/generated/api.ts` 中對應的 interface 欄位
3. 比對 `frontend/src/schemas/*.ts` 中的 Effect Schema 欄位
4. 常見原因：
   - 後端 `Option<T>` → 生成 `T | null` → Schema 需用 `Schema.NullOr(Schema.T)`
   - 後端改了欄位名稱 → Schema 欄位名稱過期
   - 後端新增必填欄位 → Schema 未更新

## 重要注意事項

- `frontend/src/generated/api.ts` **進入版控**。後端 DTO 改動後必須重新執行 `bun run generate:api` 並 commit 結果。
- `generate:api` script 使用本地 `docs/api/openapi-generated.json`（由 `cargo test -p core-service -- write_openapi_spec_to_file` 生成），不需要 running server。
- `AssertExtends<A, B>` 驗證 B extends A，即「B 可以 assign 給 A」。若需雙向驗證，加兩行 assertion。
- Effect Schema 提供 **runtime 驗證**（response body 解碼時），type assertion 提供 **compile-time 驗證**，兩者互補，不可相互替代。
- Effect 4.0 預計改寫 Schema API。屆時需更新 AssertExtends pattern，但 openapi-typescript 的部分不受影響。
