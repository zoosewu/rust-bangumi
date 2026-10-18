-- SyncService::notify_viewer_resync 會寫入 'resyncing'（重新同步進行中），但約束從未包含這個值：
-- 寫入一直被擋下，viewer 仍照做而 core 不知道 resync 在進行中（也就無法依序等待、無法辨識失敗回報）。
ALTER TABLE downloads
    DROP CONSTRAINT IF EXISTS downloads_status_check;

ALTER TABLE downloads
    ADD CONSTRAINT downloads_status_check CHECK (status IN (
        'pending', 'downloading', 'completed', 'failed', 'cancelled',
        'downloader_error', 'no_downloader', 'syncing', 'resyncing', 'synced', 'sync_failed',
        'batch_unmatched', 'batch_failed'
    ));
