# Finishing Development Phase - 自動化項目收尾

## 目的

當開發工作完成到一個段落時，自動執行以下收尾工作：

1. **代碼質量檢查** - 格式、lint、編譯驗證
2. **文檔更新** - 生成完成報告和更新進度
3. **API Spec 驗證** - 確保 OpenAPI 規格有效
4. **環境驗證** - 檢查開發環境配置
5. **Git 提交** - 自動提交變更

## 工作流程

### 第一步：收集信息

在開始收尾工作前，向用戶收集以下信息：

1. **Phase 名稱** - 這個開發階段的名稱 (例：`phase5-anime-management`)
2. **實現功能** - 列出在這個 phase 中實現的功能
3. **API 端點更新** - 記錄新增或修改的 API 端點
4. **資料庫變更** - 記錄數據庫表或字段的變更
5. **測試狀態** - 確認測試是否通過

### 第二步：代碼質量檢查

- 執行 `cargo fmt --all` 進行代碼格式化
- 執行 `cargo clippy --all-targets --all-features` 進行靜態分析
- 執行 `cargo check` 進行編譯檢查
- 報告檢查結果

### 第三步：驗證 API Spec

- 確認 `/workspace/docs/api/openapi.yaml` 存在
- 驗證 YAML 格式正確
- 提示是否需要更新 spec

### 第四步：生成完成報告

建立檔案：`/workspace/docs/plans/YYYY-MM-DD-<phase-name>-completion.md`

報告包含：
- Phase 名稱和完成日期
- 實現的功能清單
- API 端點更新
- 資料庫變更
- 測試狀態
- 待辦項

### 第五步：更新進度日誌

在 `/workspace/docs/PROGRESS.md` 中添加：
- Phase 名稱
- 完成日期
- 完成報告位置

### 第六步：Git 提交

- 自動生成提交訊息
- 添加所有修改檔案
- 提交代碼
- 顯示推送建議

## 執行方式

使用 Skill tool 或 Claude Code 中的 `/finishing-development-phase` 命令：

```
/finishing-development-phase
```

或直接使用 Skill tool：

```
Skill: finishing-development-phase
```

## 詳細步驟

### 步驟 1：詢問 Phase 信息

```
❓ 請提供此開發階段的信息：

1. Phase 名稱 (例：phase5-anime-management)
2. 實現的主要功能（用列表形式）
3. 新增的 API 端點（用列表形式）
4. 資料庫變更（新表、修改字段等）
5. 測試狀態（單元測試、集成測試、手動測試）
```

### 步驟 2：執行代碼檢查

- 運行三個檢查命令
- 如果格式不符，自動執行 `cargo fmt`
- 報告任何 Clippy 警告
- 確認編譯成功

### 步驟 3：驗證 API Spec

- 檢查 `/workspace/docs/api/openapi.yaml` 是否存在
- 如果存在，提示是否需要更新
- 顯示當前 spec 的端點摘要

### 步驟 4：生成完成報告

建立結構化的完成報告，包含：

```markdown
# [Phase 名稱] 完成報告

**完成日期：** YYYY-MM-DD

## 實現功能
- [x] 功能 1
- [x] 功能 2

## API 端點更新
...

## 資料庫變更
...

## 測試情況
...
```

用戶可以在生成後編輯報告添加詳細信息。

### 步驟 5：提交到 Git

- 顯示將要提交的檔案列表
- 生成提交訊息
- 請求確認
- 執行提交
- 提示推送命令

## 檢查清單

完成工作後，此 skill 會驗證：

- ✅ 代碼格式正確
- ✅ 無編譯錯誤
- ✅ Clippy 檢查通過
- ✅ API Spec 有效
- ✅ 完成報告已生成
- ✅ 進度日誌已更新
- ✅ 代碼已提交

## 輸出

完成後會生成：

1. **完成報告** - `/workspace/docs/plans/YYYY-MM-DD-<phase>-completion.md`
2. **更新的進度日誌** - `/workspace/docs/PROGRESS.md`
3. **Git 提交** - 帶有結構化訊息
4. **總結報告** - 顯示所有完成的工作

## 故障處理

如果遇到問題：

- **代碼格式錯誤** - 自動執行 `cargo fmt`
- **編譯失敗** - 顯示錯誤並要求修復
- **缺少完成信息** - 提示補充
- **Git 衝突** - 提示用戶手動解決

## 使用示例

### 例 1：完成動畫管理功能

```
user: 我完成了動畫管理功能的開發，請進行收尾

assistant: [調用 finishing-development-phase skill]
```

Skill 會：
1. 詢問 phase 名稱、功能、API 端點等
2. 運行代碼檢查
3. 驗證 API spec
4. 生成完成報告
5. 提交代碼

### 例 2：完成 Bug 修復

```
user: 修復了 3 個 bug，請幫我收尾

assistant: [調用 finishing-development-phase skill]
```

相同的流程會執行。

## 注意事項

- 此 skill 不會自動推送到遠程倉庫（需手動 `git push`）
- 生成的完成報告需要用戶填寫詳細信息
- 所有檢查都是非破壞性的（不會刪除代碼）
- 如果 Git 操作失敗，可以稍後手動提交

## 下一步

完成收尾工作後：

1. 查看生成的完成報告
2. 填寫詳細的功能說明和測試信息
3. 如需更新 API spec，編輯 `/workspace/docs/api/openapi.yaml`
4. 推送到遠程倉庫：`git push`
5. 開始下一個開發 phase

## 相關檔案

- API Spec: `/workspace/docs/api/openapi.yaml`
- 進度日誌: `/workspace/docs/PROGRESS.md`
- 完成報告: `/workspace/docs/plans/YYYY-MM-DD-*-completion.md`
- 開發指南: `/workspace/docs/PHASE_FINALIZATION_GUIDE.md`

---

**使用方式：**

在 Claude Code 中執行：

```
/finishing-development-phase
```

或告訴 Claude：

```
我完成了 [phase-name] 的開發，請進行收尾工作
```

Claude 會自動調用此 skill 執行完整的收尾流程。
