-- 還原前先把新狀態收斂為既有值，否則加回舊約束會失敗
UPDATE downloads SET status = 'downloading' WHERE status = 'batch_unmatched';
UPDATE downloads SET status = 'failed' WHERE status = 'batch_failed';

ALTER TABLE downloads
    DROP CONSTRAINT IF EXISTS downloads_status_check;

ALTER TABLE downloads
    ADD CONSTRAINT downloads_status_check CHECK (status IN (
        'pending', 'downloading', 'completed', 'failed', 'cancelled',
        'downloader_error', 'no_downloader', 'syncing', 'synced', 'sync_failed'
    ));
