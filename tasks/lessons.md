# Lessons

- For action-only table columns, do not fill empty cells with placeholder text unless the user asks for it; leave them blank when no action is available.
- Prefer icon-only controls for compact repeat actions like retry when the surrounding column already names the action.
- When a user says a status should explain why an item did not proceed, make that status take priority over generic parser/download state in both API data and UI rendering.
- For tags in dark UI, avoid high-saturation foreground colors on tinted backgrounds; keep text neutral and use only subtle borders/background tints for semantic distinction.
- 對生產環境做的任何設定調整，都要在同一輪工作中回寫 repo 對應的檔案（例如改 qBittorrent 偏好設定 → 更新 `configs/qbittorrent/*.conf` 並記錄理由）。生產調整若只存在於生產，容器重建即遺失、無版本控制、dev/prod 行為分歧。**做調整前先找出 repo 裡對應的檔案；若找不到（設定屬 runtime state、未被掛載），仍要把關鍵設定與理由文件化。** 資料庫中的應用資料（parser、filter rule）由 UI 管理，屬例外，但變更理由值得記錄。
- 把「檔案找不到／掃描結果為空」歸因到程式邏輯之前，先用 `docker inspect` 確認該容器真的掛載了那個路徑。2026-10-16 曾把合集 batch_failed 歸因於解析器拆分作品，實際原因是 core 與 downloader 根本沒有掛載 /downloads。
- 生產 DB 的狀態「不可能由程式產生」時（例如沒有任何路徑會寫入的狀態轉移），先做索引一致性檢查（seq scan 與 index scan 筆數比對、主鍵重複、序列是否落後 max(id)），再去推論程式 bug。
- 交給使用者的重啟指令，若涉及有輪詢依賴的服務，要依「被依賴者先起」排序，而且要分成兩次 `up`（先 `up -d downloader…`，等 healthy 後再 `up -d core-service`）。同一次 `docker compose up -d --no-deps A B` 不保證啟動順序。2026-10-16 部署第 6 項時 core 先起、downloader 晚了 5 秒，core 一輪詢就把 4 筆標成 downloader_error（E3）；這次靠 downloader 註冊後的恢復流程自動救回，修好 E3 前不能指望每次都這麼幸運。
- 比較「改動前後」的 clippy 警告數時不要用 `git stash` / `stash pop`：2026-10-17 一次 pop 撞上 `.git/index.lock`，變更雖已套回但 stash 沒被移除，差點以為改動遺失。改用 `git worktree add` 到 scratchpad 跑基準，或在動手前先記下基準數字。
- 回覆使用者一律用繁體中文，包括長篇總結與部署說明；寫完英文為主的 commit 訊息或程式碼後，切回對話時特別容易漏掉。2026-10-17 項目 4 的完成報告整篇用了英文，被使用者糾正。
  2026-10-18 階段 A 完成報告又整篇用英文，第二次被糾正。規則：**送出任何給使用者的回覆前，先確認整段是繁體中文**；長時間連續寫英文 commit / 程式碼後的總結最容易出錯，寫總結時從第一句就用中文起草。
- `sd -F` 做多行替換時，比對不到不會報錯、檔案也不變。2026-10-20 下載目錄清理的 core 路由就這樣漏掉，部署後打 API 才發現是 404；編譯也過，因為 handler 沒被用到的警告淹沒在既有的 95 個警告裡。規則：**多行替換用 Edit 工具或會 assert 的 python**；用 sd 替換後立刻用 `rg` 確認新內容真的在檔案裡；新增 API 端點後，在 main.rs 用 `rg` 確認路由有註冊。
- zsh 不會把未加引號的變數拆成多個參數（bash 會）：`C="docker --context x"; $C ps`、`X="xh -b"; $X ...`、`$IDS` 都會被當成一整個指令名稱或一個參數。2026-10-23 給使用者的遠端停止指令、2026-10-24 本機的 API 驗證都因此失敗。規則：給遠端（zsh）或本機（zsh）的指令不要把指令或多個參數存進變數；需要時直接寫出完整指令、用 for 迴圈列出字面值，或改寫成 Python 腳本。
- 這個 repo 會反覆出現殘留的空 `.git/index.lock`（2026-10-20、10-24、10-28，疑似 VS Code / Copilot 的 git 整合中斷留下）。遇到時：看建立時間、`ps` 確認沒有 git 程序（gitstatusd 除外），超過幾分鐘就移除後重試；不要用 `git stash` 繞過（會留下半套狀態）。
- 判斷「某個資料夾沒人在用」之前，除了看容器的掛載來源，還要查 named volume 的 `driver_opts`：bind 型 named volume（`type: none, o: bind, device: ...`）在 `docker inspect` 的 Mounts 只顯示 `/var/lib/docker/volumes/<name>/_data`，看不出實際目錄。2026-10-28 我把 `data/jellyfin-config`、`data/jellyfin-cache` 誤判成只有 Docker Desktop 在用並建議刪除，實際上是 OrbStack Jellyfin 正在用的設定（`docker volume inspect` 才看得到 device）。規則：建議刪除前跑 `docker volume inspect` 檢查所有 volume 的 device，並 grep compose 檔裡的路徑變數。
- `bool::then_some(x)` 一定會先建立 `x`（不管條件真假），`then(|| x)` 才是延遲建立。`x` 有 Drop 副作用時（放開佔位、拿鎖）會出事：2026-11-06 `claim()` 在佔位失敗時也建立了 GenerationClaim，丟掉時去拿同一把還沒放開的鎖 → 死結（測試卡住才發現）。規則：會被 drop 的守衛型別一律用 `then(||…)`，並且先放開鎖再建立。
- `pkill -f <字串>` 會比對到正在執行這條指令的 shell 本身（字串就在它的命令列裡），把自己殺掉（exit 144），到 2026-11-06 已經第三次。規則：要結束程序時先 `pgrep -fa` 找出 PID，再在**另一個**指令裡 `kill <PID>`；不要在同一條指令裡用 `pkill -f`。
- 寫入新的狀態值之前，先查資料表的 CHECK 限制（`select pg_get_constraintdef(oid) from pg_constraint where conrelid='<table>'::regclass`）。2026-11-06 我把批次結果標成 `rejected`，但 `pending_ai_results.status` 只允許 generating / pending / confirmed / failed（「拒絕」其實是刪除），生產回 500；DB 測試直接呼叫內部函式、跳過了寫入狀態的那段，所以沒抓到。規則：DB 測試要涵蓋實際寫入的路徑；多步驟寫入包在 transaction 裡，避免前半段成功、後半段失敗留下半套狀態（這次解析器先被刪、狀態更新才失敗）。
