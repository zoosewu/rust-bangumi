# Bangumi 開發 Skills 指南

## 可用的 Agent Skills

### 1. Finishing Development Phase - 項目自動化收尾

**檔案位置：** `.claude/skills/finishing-development-phase.md`

**功能：** 當開發工作完成到一個段落時，自動執行以下工作：

- ✅ 代碼格式檢查和修復
- ✅ Clippy 靜態分析
- ✅ 編譯驗證
- ✅ API Spec 驗證
- ✅ 生成完成報告
- ✅ 更新進度日誌
- ✅ 自動 Git 提交

**使用方式：**

在與 Claude Code 互動時，只需告訴 Claude：

```
我完成了 [phase-name] 的開發，請進行收尾工作
```

或

```
完成了動畫管理功能的實現，請幫我進行項目收尾
```

Claude 會自動調用此 skill 進行完整的收尾流程。

**互動流程：**

```
你：我完成了 phase5-anime-management 的開發，請進行收尾

Claude：[調用 finishing-development-phase skill]
        🎯 開始項目收尾工作流程

        📋 收集開發階段信息...
        
        輸入 Phase 名稱: phase5-anime-management
        
        📝 實現的功能：
        功能: 實現動畫基本管理
        功能: 添加季度過濾
        功能: 實現字幕組管理
        功能: 
        
        🔌 新增/修改的 API 端點：
        端點: POST /anime
        端點: GET /anime/{id}
        端點: DELETE /anime/{id}
        端點:
        
        ... (數據庫變更)
        
        🔍 執行代碼質量檢查...
        ✓ 代碼格式已修復
        ✓ Clippy 檢查完成
        ✓ 編譯成功
        
        📝 驗證 API Spec...
        ✓ API Spec 存在
        
        📄 生成完成報告...
        ✓ 完成報告已生成
        
        📊 更新進度日誌...
        ✓ 進度日誌已更新
        
        📤 準備 Git 提交...
        ✓ 已提交
        
        ✨ 項目收尾工作完成！
```

## 工作流程概覽

### 標準開發流程

```
1. 啟動開發環境
   $ make dev-infra
   $ make dev-run

2. 開發功能...
   （編寫代碼、修改代碼、測試等）

3. 完成一個階段後
   「我完成了 [phase-name] 的開發，請進行收尾」
   
   → Agent 自動執行：
     ✅ 代碼檢查
     ✅ 文檔更新
     ✅ 報告生成
     ✅ Git 提交

4. 推送到遠程
   $ git push
```

## 生成的文件

完成收尾後會生成：

| 文件 | 位置 | 說明 |
|------|------|------|
| 完成報告 | `/workspace/docs/plans/YYYY-MM-DD-<phase>-completion.md` | 詳細的階段完成記錄 |
| 更新的進度日誌 | `/workspace/docs/PROGRESS.md` | 項目整體進度 |
| Git 提交 | Git log | 結構化的提交訊息 |

## Skill 的智能特性

### 1. 自動代碼修復
- 如果代碼格式不符，自動執行 `cargo fmt`
- 編譯成功後才進行下一步

### 2. 互動式信息收集
- 通過問答收集 phase 信息
- 支持多行輸入（功能、API 端點等）

### 3. 結構化報告
- 自動生成完整的完成報告
- 包含功能、API、數據庫、測試等內容
- 用戶可後續編輯補充

### 4. 智能 Git 提交
- 生成有結構的提交訊息
- 顯示預覽，要求確認
- 失敗時給出恢復建議

## 常見場景

### 場景 1：完成一個功能
```
完成了 RSS 訂閱功能的實現，請進行收尾
```

Agent 會自動：
- 檢查代碼質量
- 記錄新增的 API 端點
- 記錄資料庫變更
- 生成完整的完成報告

### 場景 2：修復一組 Bug
```
修復了 3 個數據庫連接問題，請完成收尾
```

Agent 會：
- 驗證修復是否成功編譯
- 記錄修復內容
- 自動提交變更

### 場景 3：性能優化
```
完成了查詢性能優化，請進行項目收尾
```

Agent 會：
- 確認性能改進代碼質量
- 記錄優化詳情
- 生成優化報告

## 後續手動操作

完成收尾後，你可能需要：

1. **編輯完成報告**
   ```bash
   vim /workspace/docs/plans/YYYY-MM-DD-<phase>-completion.md
   ```

2. **更新 API Spec**（如有新端點）
   ```bash
   vim /workspace/docs/api/openapi.yaml
   ```

3. **推送到遠程**
   ```bash
   git push
   ```

4. **提交代碼審查**
   ```bash
   gh pr create --title "Phase: <name>" --body "$(cat /workspace/docs/plans/YYYY-MM-DD-<phase>-completion.md)"
   ```

## 整合提示

此 skill 與以下工具/流程完美整合：

- ✅ **開發環境**：`make dev-infra` + `make dev-run`
- ✅ **代碼檢查**：自動執行 `cargo fmt`、`cargo clippy`
- ✅ **文檔**：自動更新進度日誌
- ✅ **API 管理**：驗證 OpenAPI spec
- ✅ **版本控制**：自動 Git 提交

## 需要幫助？

如果 skill 執行中出現問題：

1. **查看 Skill 文檔**
   ```bash
   cat .claude/skills/finishing-development-phase.md
   ```

2. **查看收尾工作指南**
   ```bash
   cat docs/PHASE_FINALIZATION_GUIDE.md
   ```

3. **手動運行 Agent**
   ```bash
   bash .claude/skills/finishing-development-phase-agent.sh
   ```

4. **檢查完成報告**
   ```bash
   cat docs/plans/YYYY-MM-DD-*-completion.md
   ```

## 下一步

開始使用此 skill：

1. 完成一個開發階段
2. 告訴 Claude：「完成了 [phase-name] 的開發，請進行收尾」
3. 按照 Agent 的提示提供信息
4. 自動完成所有收尾工作！

祝開發愉快！🚀
