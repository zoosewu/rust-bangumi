# TAM - tmux AI Manager 設計文件

**日期：** 2026-02-27
**命令：** `tam`

## 概述

一個互動式 tmux session 管理工具，整合 Claude AI 自動生成每個 session 的摘要，方便在多個 Claude Code session 間快速切換。

## UI 佈局

```
┌─────────────────────────────────────────────┐
│  TAM - tmux AI Manager          [3 sessions] │
├─────────────────────────────────────────────┤
│ ▶ [1] dev-work      ★ 正在實作 auth API     │
│   [2] debug-redis     ⟳ 載入摘要中...       │
│   [3] infra-docker  ★ 設定 compose 檔       │
├─────────────────────────────────────────────┤
│ Preview: dev-work                            │
│ $ git add .                                  │
│ $ cargo build                                │
│ error[E0502]: cannot borrow...               │
└─────────────────────────────────────────────┘
  ↑↓/jk 移動  Enter 進入  r 改名  d 刪除  q 離開
```

## 技術架構

### 安裝位置
- 主腳本：`~/tam`
- Symlink：`~/.local/bin/tam`
- Config：`~/.config/tam/config`

### AI 整合
- 使用 `claude -p --model <model>` 呼叫 Claude CLI
- 背景非同步執行，不阻塞主選單
- 摘要格式：`摘要內容 | 建議名稱`

### 元件

| 函式 | 說明 |
|------|------|
| `load_config()` | 載入 config 檔，套用預設值 |
| `list_sessions()` | 從 `tmux ls` 取得 session 列表 |
| `capture_pane()` | `tmux capture-pane` 擷取最後 N 行 |
| `ai_worker()` | 背景呼叫 `claude -p`，存入 temp 檔 |
| `render_ui()` | 清空終端機並重繪整個選單 |
| `input_loop()` | `read -sn1` 捕捉按鍵，dispatch 動作 |
| `action_attach()` | attach 到選中的 session |
| `action_rename()` | 改名（預填 AI 建議名稱） |
| `action_kill()` | 刪除 session（確認後執行） |
| `cleanup()` | EXIT trap，清理 temp 目錄 |

## Config 檔 (`~/.config/tam/config`)

```bash
# AI 設定
TAM_MODEL="claude-haiku-4-5-20251001"
TAM_CAPTURE_LINES=150
TAM_PROMPT="用一句繁體中文描述這個 terminal session 在做什麼，格式：摘要內容 | 建議名稱"

# UI 設定
TAM_PREVIEW_HEIGHT=8      # preview 區塊顯示幾行
TAM_REFRESH_INTERVAL=0.5  # 輪詢 AI 摘要結果的間隔（秒）

# 路徑
TAM_CACHE_DIR="/tmp/tam-$$"
```

## 資料流

```
啟動
  │
  ├── load_config()
  ├── list_sessions()
  ├── 為每個 session 啟動 ai_worker() (background)
  │     └── capture_pane() → claude -p → /tmp/tam-$$/summary-{session}
  │
  └── input_loop()
        ├── render_ui() (每 REFRESH_INTERVAL 秒重繪)
        │     ├── 讀取 session 列表
        │     ├── 讀取 AI 摘要（如已完成）
        │     └── 顯示 preview（當前選中 session）
        └── 按鍵處理
              ├── ↑↓/jk → 移動游標
              ├── Enter  → action_attach()
              ├── r      → action_rename()
              ├── d      → action_kill()
              └── q      → 離開
```

## 技術細節

- 語言：純 bash（相容 zsh）
- 依賴：`tmux`、`claude`（Claude Code CLI）
- ANSI：手刻 escape codes，支援終端機 resize
- 按鍵：`read -sn1` 捕捉單字元，`\033[` 序列處理方向鍵
- Temp 目錄：`/tmp/tam-$$`，`trap cleanup EXIT` 自動清理
