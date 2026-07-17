# Lessons

- For action-only table columns, do not fill empty cells with placeholder text unless the user asks for it; leave them blank when no action is available.
- Prefer icon-only controls for compact repeat actions like retry when the surrounding column already names the action.
- When a user says a status should explain why an item did not proceed, make that status take priority over generic parser/download state in both API data and UI rendering.
- For tags in dark UI, avoid high-saturation foreground colors on tinted backgrounds; keep text neutral and use only subtle borders/background tints for semantic distinction.
- 對生產環境做的任何設定調整，都要在同一輪工作中回寫 repo 對應的檔案（例如改 qBittorrent 偏好設定 → 更新 `configs/qbittorrent/*.conf` 並記錄理由）。生產調整若只存在於生產，容器重建即遺失、無版本控制、dev/prod 行為分歧。**做調整前先找出 repo 裡對應的檔案；若找不到（設定屬 runtime state、未被掛載），仍要把關鍵設定與理由文件化。** 資料庫中的應用資料（parser、filter rule）由 UI 管理，屬例外，但變更理由值得記錄。
