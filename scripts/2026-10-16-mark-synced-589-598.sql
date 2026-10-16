-- 2026-10-16 資料修復：589（ブチ切れ令嬢 04）、598（てんびん 08）實際已同步，卻被記成 sync_failed
--
-- 背景（稽核 E6 的實際重現）：第 6 項部署後重跑合集同步，viewer 回應 202 超過 core 的 10 秒逾時，
-- core 當成「沒送出」而在之後的輪詢重複送出同一筆同步。第一個請求把檔案搬進媒體庫並回報 synced，
-- 後到的重複請求找不到來源檔而回報 failed，core 無條件接受，把 synced 蓋成 completed → sync_failed。
--
-- 已確認：viewer sync_tasks 262（589）與 266（598）為 completed；媒體庫兩個檔案存在，大小與原始檔一致；
-- downloads.file_path 已是媒體庫路徑（synced callback 寫入的），所以只需要改回狀態。
-- 冪等：只動仍為 sync_failed 的這兩筆。
--
-- 使用方式（生產）：
--   docker exec -i bangumi-postgres psql -U bangumi -d bangumi -v ON_ERROR_STOP=1 < 本檔
BEGIN;

UPDATE downloads
SET status = 'synced',
    sync_retry_count = 0,
    error_message = NULL,
    updated_at = NOW()
WHERE download_id IN (589, 598)
  AND status = 'sync_failed'
  AND file_path LIKE '/media/jellyfin/%';

-- 預期 2 列 synced
SELECT download_id, status, file_path FROM downloads WHERE download_id IN (589, 598) ORDER BY 1;

COMMIT;
