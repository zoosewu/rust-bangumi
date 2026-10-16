-- 2026-10-16 資料修復：重新走一次「下載完成」流程，讓合集與資料夾型 torrent 取得實際檔案
--
-- 背景：core 與 downloader-qbittorrent 之前都沒有掛載 /downloads（稽核 E2），downloader 回報的
-- 檔案清單永遠是空的 → video_file 全為 NULL：
--   - 6 個合集（42 集）逐集配對失敗，重試 10 次後停在 batch_failed
--   - 421 魔法科劇場版（BDRip 資料夾）把資料夾當影片交給 viewer，同步失敗 3 次停在 sync_failed
-- 掛載修好並部署後，把這些記錄改回 downloading：下一輪輪詢看到 qB 已完成，會用實際檔案清單
-- 重新挑主影片（最大的影片檔）／逐集配對，再交給 viewer 同步。
--
-- 部署前已用生產這 7 個資料夾的實際檔名驗證：42 集全部配對成功；劇場版 17 個影片中選中正片。
-- 冪等：只動仍停在 batch_failed / sync_failed 的記錄，重跑無副作用。
--
-- 使用方式（生產，需先部署含 /downloads 掛載的 core 與 downloader-qbittorrent）：
--   docker exec -i bangumi-postgres psql -U bangumi -d bangumi -v ON_ERROR_STOP=1 < 本檔
BEGIN;

UPDATE downloads
SET status = 'downloading',
    sync_retry_count = 0,
    error_message = NULL,
    video_file = NULL,
    subtitle_files = NULL,
    updated_at = NOW()
WHERE module_id = 2
  AND (
    (status = 'batch_failed' AND torrent_hash IN (
      '3b32e61c3de48397c57dca7697baee4770953d5a',  -- Super no Ura de Yani Suu Futari 01-12
      '6fc9e67c716175a8599f5eb0dda13993d9c8e257',  -- Tenbin 01-12
      '8c0a13b2a0c5cdf8bf144741dd79c01e3c0a053a',  -- Buchigire Reijou 01-12
      '95a78f2dcf43e863638092675e87d859cd9721b6',  -- Tongari Boushi 04-05
      'b25e1974e9e003e16283e91ad168ef898c670bf0',  -- Tongari Boushi 01-02
      'b848f5c61b622efb8612de7f1a3c6fe5605a208b'   -- Tongari Boushi 12-13
    ))
    OR (status = 'sync_failed' AND download_id = 421
        AND torrent_hash = '5e7fd2b62fb74ef8d0da49a7bf1bd7916b39624a')  -- 魔法科劇場版 BDRip 資料夾
  );

-- 預期 43 列（42 + 1）
SELECT status, count(*) FROM downloads
WHERE torrent_hash IN (
  '3b32e61c3de48397c57dca7697baee4770953d5a', '6fc9e67c716175a8599f5eb0dda13993d9c8e257',
  '8c0a13b2a0c5cdf8bf144741dd79c01e3c0a053a', '95a78f2dcf43e863638092675e87d859cd9721b6',
  'b25e1974e9e003e16283e91ad168ef898c670bf0', 'b848f5c61b622efb8612de7f1a3c6fe5605a208b',
  '5e7fd2b62fb74ef8d0da49a7bf1bd7916b39624a')
GROUP BY status;

COMMIT;
