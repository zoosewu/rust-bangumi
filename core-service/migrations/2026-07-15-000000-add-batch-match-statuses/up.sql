-- download_scheduler 的批次檔案配對流程會寫入 'batch_unmatched'（暫時無法把集數
-- 對應到檔案，待重試）與 'batch_failed'（重試耗盡的終態），但初始 schema 的
-- downloads_status_check 從未包含這兩個值。結果是每次寫入都被約束擋下，整個 UPDATE
-- 被 rollback（連 progress / file_path 都沒寫入），記錄永久卡在 'downloading'。
ALTER TABLE downloads
    DROP CONSTRAINT IF EXISTS downloads_status_check;

ALTER TABLE downloads
    ADD CONSTRAINT downloads_status_check CHECK (status IN (
        'pending', 'downloading', 'completed', 'failed', 'cancelled',
        'downloader_error', 'no_downloader', 'syncing', 'synced', 'sync_failed',
        'batch_unmatched', 'batch_failed'
    ));
