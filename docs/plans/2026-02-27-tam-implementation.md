# TAM - tmux AI Manager Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 建立一個互動式 tmux session 管理器，整合 Claude Haiku 非同步生成每個 session 的摘要。

**Architecture:** 單一 bash 腳本 `~/tam`，啟動時背景呼叫 `claude -p` 為每個 tmux session 生成摘要，主選單用 ANSI escape codes 手刻互動 UI，`read -sn1` 捕捉按鍵。摘要結果存入 `/tmp/tam-PID/` temp 目錄，主循環輪詢更新。

**Tech Stack:** bash 4+, tmux, Claude Code CLI (`claude`)

---

### Task 1: 建立目錄結構與 config 檔

**Files:**
- Create: `~/tam` (主腳本)
- Create: `~/.config/tam/config` (使用者 config)

**Step 1: 建立 config 目錄與預設 config**

```bash
mkdir -p ~/.config/tam
cat > ~/.config/tam/config << 'EOF'
# TAM - tmux AI Manager Config

# AI 設定
TAM_MODEL="claude-haiku-4-5-20251001"
TAM_CAPTURE_LINES=150
TAM_PROMPT="用一句繁體中文描述這個 terminal session 在做什麼，格式：摘要內容 | 建議名稱"

# UI 設定
TAM_PREVIEW_HEIGHT=8
TAM_REFRESH_INTERVAL=0.3

# 路徑（不建議修改）
TAM_CACHE_DIR="/tmp/tam-$$"
EOF
```

**Step 2: 建立主腳本骨架，確認可執行**

建立 `~/tam`，內容如下（完整腳本見後續 Task）：

```bash
#!/usr/bin/env bash
# TAM - tmux AI Manager
set -euo pipefail

# 載入 config
TAM_MODEL="claude-haiku-4-5-20251001"
TAM_CAPTURE_LINES=150
TAM_PROMPT="用一句繁體中文描述這個 terminal session 在做什麼，格式：摘要內容 | 建議名稱"
TAM_PREVIEW_HEIGHT=8
TAM_REFRESH_INTERVAL=0.3
TAM_CACHE_DIR="/tmp/tam-$$"

[[ -f "${HOME}/.config/tam/config" ]] && source "${HOME}/.config/tam/config"
# 重新展開 $$ (config 裡的 $$ 是 subshell PID，需覆蓋)
TAM_CACHE_DIR="/tmp/tam-$$"

echo "TAM loaded"
```

```bash
chmod +x ~/tam
~/tam
```

預期輸出：`TAM loaded`

**Step 3: 建立 symlink**

```bash
mkdir -p ~/.local/bin
ln -sf ~/tam ~/.local/bin/tam
# 確認 PATH 包含 ~/.local/bin
echo $PATH | grep -q "$HOME/.local/bin" && echo "PATH OK" || echo "請將 ~/.local/bin 加入 PATH"
```

**Step 4: Commit**

```bash
git add docs/plans/2026-02-27-tam-implementation.md
git commit -m "docs: add tam tmux AI manager implementation plan"
```

---

### Task 2: tmux 工具函式

**Files:**
- Modify: `~/tam`（加入 tmux 相關函式）

**Step 1: 加入 `list_sessions` 函式**

在 `~/tam` 主體中加入：

```bash
# 取得所有 tmux session 名稱（陣列）
list_sessions() {
    tmux list-sessions -F "#{session_name}" 2>/dev/null || true
}

# 取得 session 的視窗數和最後活動時間
session_info() {
    local name="$1"
    tmux list-sessions -F "#{session_name} #{session_windows} #{session_activity}" 2>/dev/null \
        | awk -v n="$name" '$1==n {print $2, $3}'
}
```

**Step 2: 加入 `capture_pane` 函式**

```bash
# 擷取 session 最後 N 行輸出
capture_pane() {
    local session="$1"
    local lines="${2:-$TAM_CAPTURE_LINES}"
    # -p: print, -t: target, -S: start line (負數從底部算)
    tmux capture-pane -p -t "${session}" -S "-${lines}" 2>/dev/null || echo "(無法取得內容)"
}
```

**Step 3: 手動驗證**

```bash
# 確保至少有一個 tmux session
tmux new-session -d -s test-tam 2>/dev/null || true
source ~/tam  # 只 source，不執行 main
list_sessions
capture_pane test-tam 5
```

預期：看到 `test-tam` 和幾行 pane 內容。

---

### Task 3: ANSI UI 工具函式

**Files:**
- Modify: `~/tam`（加入 ANSI/繪圖函式）

**Step 1: 加入顏色與游標控制常數**

```bash
# ANSI 常數
RESET='\033[0m'
BOLD='\033[1m'
DIM='\033[2m'
REVERSE='\033[7m'
RED='\033[31m'
GREEN='\033[32m'
YELLOW='\033[33m'
BLUE='\033[34m'
CYAN='\033[36m'
WHITE='\033[37m'

cursor_hide()    { printf '\033[?25l'; }
cursor_show()    { printf '\033[?25h'; }
cursor_move()    { printf '\033[%d;%dH' "$1" "$2"; }  # row col
clear_screen()   { printf '\033[2J\033[H'; }
clear_line()     { printf '\033[2K'; }
term_rows()      { tput lines; }
term_cols()      { tput cols; }
```

**Step 2: 加入框線繪製輔助函式**

```bash
# 繪製水平分隔線（填滿終端機寬度）
draw_hline() {
    local cols
    cols=$(term_cols)
    local char="${1:-─}"
    printf '%*s' "$cols" '' | tr ' ' "$char"
}

# 截斷字串到指定寬度（處理中文）
truncate_str() {
    local str="$1"
    local max="$2"
    # 使用 awk 處理（避免中文字元寬度問題）
    echo "$str" | awk -v max="$max" '{ if (length($0) > max) print substr($0,1,max)"…"; else print $0 }'
}
```

**Step 3: 手動驗證**

```bash
source ~/tam
clear_screen
cursor_move 1 1
printf "${BOLD}${CYAN}TAM Test${RESET}\n"
draw_hline
cursor_show
```

預期：終端機清空，顯示藍色粗體標題和水平線。

---

### Task 4: AI Worker（背景非同步摘要）

**Files:**
- Modify: `~/tam`（加入 AI worker 函式）

**Step 1: 加入 `ai_worker` 函式**

```bash
# 背景執行：為單一 session 生成 AI 摘要
# 結果存到 $TAM_CACHE_DIR/summary-{session}
# 格式：摘要內容 | 建議名稱
ai_worker() {
    local session="$1"
    local summary_file="${TAM_CACHE_DIR}/summary-${session}"
    local content_file="${TAM_CACHE_DIR}/content-${session}"

    # 取得 pane 內容
    capture_pane "$session" "$TAM_CAPTURE_LINES" > "$content_file"

    # 呼叫 claude（帶 timeout 避免卡住）
    local result
    result=$(timeout 30 claude -p --model "$TAM_MODEL" "$TAM_PROMPT" < "$content_file" 2>/dev/null) || result="(摘要失敗)"

    # 寫入結果（atomic write）
    echo "$result" > "${summary_file}.tmp"
    mv "${summary_file}.tmp" "$summary_file"
}

# 讀取 session 的摘要（若未完成回傳空字串）
read_summary() {
    local session="$1"
    local summary_file="${TAM_CACHE_DIR}/summary-${session}"
    [[ -f "$summary_file" ]] && cat "$summary_file" || echo ""
}

# 從摘要中解析摘要文字部分
parse_summary_text() {
    echo "$1" | cut -d'|' -f1 | xargs
}

# 從摘要中解析建議名稱部分
parse_suggested_name() {
    echo "$1" | cut -d'|' -f2 | xargs
}

# 啟動所有 session 的 AI worker（背景執行）
start_ai_workers() {
    local sessions=("$@")
    mkdir -p "$TAM_CACHE_DIR"
    for session in "${sessions[@]}"; do
        ai_worker "$session" &
    done
}
```

**Step 2: 加入 cleanup**

```bash
cleanup() {
    cursor_show
    clear_screen
    # 結束所有背景 job
    jobs -p | xargs -r kill 2>/dev/null || true
    rm -rf "$TAM_CACHE_DIR"
}
trap cleanup EXIT INT TERM
```

**Step 3: 手動驗證**

```bash
source ~/tam
TAM_CACHE_DIR="/tmp/tam-test-$$"
mkdir -p "$TAM_CACHE_DIR"
ai_worker "test-tam" &
wait
cat "$TAM_CACHE_DIR/summary-test-tam"
```

預期：幾秒後顯示一行中文摘要，包含 `|` 分隔。

---

### Task 5: UI 渲染（render_ui）

**Files:**
- Modify: `~/tam`（加入渲染函式）

**Step 1: 加入主渲染函式**

```bash
# 全域狀態
SESSIONS=()       # session 名稱陣列
CURSOR=0          # 當前游標位置（index）

render_ui() {
    local cols rows
    cols=$(term_cols)
    rows=$(term_rows)
    local count=${#SESSIONS[@]}

    clear_screen
    cursor_move 1 1

    # ── 標題列 ──
    local title=" TAM - tmux AI Manager"
    local session_count="[${count} sessions] "
    printf "${BOLD}${CYAN}%-$((cols - ${#session_count}))s${RESET}" "$title"
    printf "${DIM}%s${RESET}\n" "$session_count"
    draw_hline; printf '\n'

    # ── Session 列表 ──
    local max_list_rows=$(( rows - TAM_PREVIEW_HEIGHT - 5 ))
    [[ $max_list_rows -lt 3 ]] && max_list_rows=3

    for i in "${!SESSIONS[@]}"; do
        local session="${SESSIONS[$i]}"
        local raw_summary
        raw_summary=$(read_summary "$session")
        local summary_text
        local prefix indicator

        if [[ -z "$raw_summary" ]]; then
            summary_text="⟳ 載入摘要中..."
            indicator="${DIM}"
        else
            summary_text="★ $(parse_summary_text "$raw_summary")"
            indicator="${GREEN}"
        fi

        local label
        label=$(truncate_str "${summary_text}" $(( cols - ${#session} - 8 )))

        if [[ $i -eq $CURSOR ]]; then
            printf " ${REVERSE}${BOLD} ▶ %-20s ${indicator}%s${RESET}\n" \
                "$(truncate_str "$session" 20)" "$label"
        else
            printf "   %-20s ${indicator}%s${RESET}\n" \
                "$(truncate_str "$session" 20)" "$label"
        fi

        # 列表最多顯示 max_list_rows 行
        [[ $i -ge $(( max_list_rows - 1 )) ]] && break
    done

    draw_hline; printf '\n'

    # ── Preview 區塊 ──
    local selected_session="${SESSIONS[$CURSOR]:-}"
    printf "${BOLD} Preview: ${CYAN}%s${RESET}\n" "$selected_session"

    if [[ -n "$selected_session" ]]; then
        capture_pane "$selected_session" "$TAM_PREVIEW_HEIGHT" \
            | tail -n "$TAM_PREVIEW_HEIGHT" \
            | while IFS= read -r line; do
                printf "  ${DIM}%s${RESET}\n" "$(truncate_str "$line" $(( cols - 3 )))"
            done
    fi

    draw_hline; printf '\n'

    # ── 操作提示 ──
    printf "${DIM} ↑↓/jk 移動  Enter 進入  r 改名  d 刪除  q 離開${RESET}\n"
}
```

**Step 2: 手動驗證（快速渲染測試）**

```bash
source ~/tam
SESSIONS=( $(list_sessions) )
CURSOR=0
TAM_CACHE_DIR="/tmp/tam-test-$$"
mkdir -p "$TAM_CACHE_DIR"
cursor_hide
render_ui
cursor_show
```

預期：清晰的選單 UI，第一個 session 反白顯示，preview 區塊有內容。

---

### Task 6: 按鍵輸入迴圈與動作處理

**Files:**
- Modify: `~/tam`（加入 input_loop 與 actions）

**Step 1: 加入 session 動作函式**

```bash
action_attach() {
    local session="${SESSIONS[$CURSOR]}"
    cursor_show
    clear_screen
    # 離開後回到 tam
    tmux attach-session -t "$session"
}

action_rename() {
    local session="${SESSIONS[$CURSOR]}"
    local raw_summary
    raw_summary=$(read_summary "$session")
    local suggested
    suggested=$(parse_suggested_name "$raw_summary")

    cursor_show
    cursor_move $(( $(term_rows) - 2 )) 1
    clear_line
    printf " 新名稱 [建議: ${CYAN}%s${RESET}]: " "$suggested"

    local new_name
    read -r new_name
    [[ -z "$new_name" ]] && new_name="$suggested"

    if [[ -n "$new_name" && "$new_name" != "$session" ]]; then
        tmux rename-session -t "$session" "$new_name"
        # 更新 SESSIONS 陣列
        SESSIONS[$CURSOR]="$new_name"
        # 移動 summary cache
        local old_cache="${TAM_CACHE_DIR}/summary-${session}"
        [[ -f "$old_cache" ]] && mv "$old_cache" "${TAM_CACHE_DIR}/summary-${new_name}"
    fi
    cursor_hide
}

action_kill() {
    local session="${SESSIONS[$CURSOR]}"

    cursor_show
    cursor_move $(( $(term_rows) - 2 )) 1
    clear_line
    printf " 確定刪除 ${RED}%s${RESET}？[y/N] " "$session"

    local confirm
    read -r confirm
    cursor_hide

    if [[ "$confirm" =~ ^[Yy]$ ]]; then
        tmux kill-session -t "$session"
        # 從陣列移除
        SESSIONS=( "${SESSIONS[@]:0:$CURSOR}" "${SESSIONS[@]:$(( CURSOR + 1 ))}" )
        local count=${#SESSIONS[@]}
        [[ $CURSOR -ge $count && $CURSOR -gt 0 ]] && (( CURSOR-- ))
    fi
}
```

**Step 2: 加入主迴圈**

```bash
input_loop() {
    local key esc_seq

    while true; do
        render_ui

        # 等待按鍵（帶 timeout 讓 UI 可以刷新摘要）
        key=""
        IFS= read -r -s -n1 -t "$TAM_REFRESH_INTERVAL" key || true

        case "$key" in
            # 方向鍵（ESC 序列）
            $'\033')
                IFS= read -r -s -n2 -t 0.1 esc_seq || true
                case "$esc_seq" in
                    '[A') [[ $CURSOR -gt 0 ]] && (( CURSOR-- )) ;;  # 上
                    '[B') [[ $CURSOR -lt $(( ${#SESSIONS[@]} - 1 )) ]] && (( CURSOR++ )) ;;  # 下
                esac
                ;;
            'k') [[ $CURSOR -gt 0 ]] && (( CURSOR-- )) ;;
            'j') [[ $CURSOR -lt $(( ${#SESSIONS[@]} - 1 )) ]] && (( CURSOR++ )) ;;
            '') ;; # timeout，繼續重繪（讓摘要更新）
            $'\n'|$'\r') action_attach ;;
            'r') action_rename ;;
            'd') action_kill ;;
            'q'|$'\x03') break ;;  # q 或 Ctrl+C
        esac

        # 若無 session 則離開
        [[ ${#SESSIONS[@]} -eq 0 ]] && break
    done
}
```

**Step 3: 加入 main 函式**

```bash
main() {
    # 確認 tmux 正在執行
    if ! command -v tmux &>/dev/null; then
        echo "錯誤：找不到 tmux" >&2
        exit 1
    fi

    # 確認 claude CLI 可用
    if ! command -v claude &>/dev/null; then
        echo "錯誤：找不到 claude CLI，請先安裝 Claude Code" >&2
        exit 1
    fi

    # 取得 session 列表
    mapfile -t SESSIONS < <(list_sessions)

    if [[ ${#SESSIONS[@]} -eq 0 ]]; then
        echo "目前沒有 tmux sessions。"
        echo "使用 'tmux new-session -s <名稱>' 建立一個。"
        exit 0
    fi

    # 初始化
    mkdir -p "$TAM_CACHE_DIR"
    cursor_hide

    # 啟動 AI workers（背景）
    start_ai_workers "${SESSIONS[@]}"

    # 進入互動迴圈
    input_loop
}

main "$@"
```

---

### Task 7: 整合所有片段，完成 ~/tam

**Files:**
- Rewrite: `~/tam`（整合所有函式為完整腳本）

**Step 1: 整合為完整腳本**

將上述所有函式按順序整合到 `~/tam`：

```
#!/usr/bin/env bash
[set -euo pipefail]
[預設 config 值]
[source ~/.config/tam/config]
[重設 TAM_CACHE_DIR]
[ANSI 常數]
[cursor/draw 函式]
[list_sessions, session_info, capture_pane]
[ai_worker, read_summary, parse_* 函式]
[start_ai_workers]
[cleanup + trap]
[SESSIONS=(), CURSOR=0]
[render_ui]
[action_attach, action_rename, action_kill]
[input_loop]
[main]
```

**Step 2: 驗證語法**

```bash
bash -n ~/tam && echo "語法 OK"
```

**Step 3: 端對端測試**

```bash
# 確保有測試 sessions
tmux new-session -d -s test-alpha 'bash' 2>/dev/null || true
tmux new-session -d -s test-beta  'bash' 2>/dev/null || true
tmux send-keys -t test-alpha 'echo "working on feature branch"' Enter
tmux send-keys -t test-beta  'echo "debugging network issue"' Enter

# 執行 tam
tam
```

預期：
- 出現選單，列出 test-alpha 和 test-beta
- ⟳ 載入中... 在幾秒後變成 ★ 中文摘要
- ↑↓ 移動游標，preview 跟著換
- Enter 進入 session，detach 後回到 tam

**Step 4: 清理測試 sessions**

```bash
tmux kill-session -t test-alpha 2>/dev/null || true
tmux kill-session -t test-beta  2>/dev/null || true
```

**Step 5: Commit**

```bash
git add ~/tam ~/.config/tam/config docs/plans/2026-02-27-tam-implementation.md
git commit -m "feat: add tam tmux AI manager with Claude Haiku summaries"
```

---

## 注意事項

1. **`set -euo pipefail` 與 `read` timeout**：`read -t` 在 timeout 時會回傳非零，需在 `set -e` 環境中用 `|| true` 處理。
2. **中文字寬**：`truncate_str` 用 `awk length()` 截斷，中文字元算 1，實際顯示佔 2 格。若版面跑掉可降低 `TAM_PREVIEW_HEIGHT` 或在 config 調整。
3. **`mapfile`**：需要 bash 4+（macOS 預設 bash 3，需用 `brew install bash` 或改用 `while IFS= read` 替代）。
4. **tmux attach 後回到 tam**：`action_attach` 在 `tmux attach-session` 回傳後（用戶 detach 後）會自動繼續執行 input_loop。

---

## 執行選項

**Plan 已儲存到 `docs/plans/2026-02-27-tam-implementation.md`。**

**兩種執行方式：**

**1. Subagent-Driven（本 session）** — 每個 Task 派一個 subagent，任務間有 review，迭代快

**2. Parallel Session（另開 session）** — 在新 session 中用 executing-plans skill，批次執行並設 checkpoint

**選哪種方式？**
