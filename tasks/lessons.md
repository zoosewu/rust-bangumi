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
