-- 2026-10-17 資料修復：把訂閱層級的 parser 綁回建立它的訂閱（項目 4）
--
-- 背景：待確認頁的「確認」用訂閱層級時沒有送 target_id，parser 存成
-- created_from_type='subscription'、created_from_id=NULL。舊程式不看綁定、每個 parser
-- 都套用到所有訂閱，所以看不出問題；新程式只用全域 parser 加上本訂閱的 parser，
-- 沒有綁定訂閱的訂閱層級 parser 不會套用到任何訂閱。
-- → 必須先跑本檔、再部署新 core，否則這些 parser 會失效（下次全量 reparse 會讓它們解析過的項目變成 no_match）。
--
-- 對應依據（2026-10-17 生產唯讀查詢）：parser 名稱對應 pending_ai_results.generated_data->>'name'
-- 取得產生它的 subscription_id，並和 raw_anime_items 實際由它解析的訂閱交叉比對，全部一致：
--   22 → 17：原本綁 14（訂閱已不存在），實際解析的 42 筆全是訂閱 17「冰之城墙 - 桜都字幕组」
--   45、55 → 18 超市後門    47、51、52、59 → 27 豌豆 藥屋    48、56 → 20 暴怒千金
--   49 → 21 轉校生          58 → 1 金牌得主          53 → 8 欺詐遊戲
--   46「黒ネズミたち - 装备仔」：來自單一標題生成（pending 72 無訂閱），目前沒有對應訂閱、
--   也沒有任何 raw item 符合 → 停用（is_enabled=false），保留記錄
--   60：仍待確認（pending 113，訂閱 8），新程式確認時會自動綁訂閱 8，不用處理
--
-- 冪等：只動 created_from_id 仍是錯誤值的記錄，重跑無副作用。
--
-- 使用方式（生產，在部署項目 4 的 core 之前）：
--   docker exec -i bangumi-postgres psql -U bangumi -d bangumi -v ON_ERROR_STOP=1 < 本檔
BEGIN;

UPDATE title_parsers AS t
SET created_from_type = 'subscription',
    created_from_id = m.subscription_id,
    updated_at = NOW()
FROM (VALUES
    (22, 17),
    (45, 18), (55, 18),
    (47, 27), (51, 27), (52, 27), (59, 27),
    (48, 20), (56, 20),
    (49, 21),
    (58, 1),
    (53, 8)
) AS m(parser_id, subscription_id)
WHERE t.parser_id = m.parser_id
  AND t.created_from_id IS DISTINCT FROM m.subscription_id;

UPDATE title_parsers
SET is_enabled = false,
    updated_at = NOW()
WHERE parser_id = 46
  AND is_enabled;

-- 驗證 1：已確認、啟用的訂閱層級 parser 都綁到存在的訂閱（應為 0 列）
SELECT t.parser_id, t.name, t.created_from_id
FROM title_parsers t
LEFT JOIN subscriptions s ON s.subscription_id = t.created_from_id
WHERE t.created_from_type = 'subscription'
  AND t.is_enabled
  AND t.pending_result_id IS NULL
  AND s.subscription_id IS NULL;

-- 驗證 2：每筆已解析的 raw item，解析它的 parser 在新規則下仍適用於它的訂閱（應為 0 列）
SELECT r.item_id, r.subscription_id, r.parser_id
FROM raw_anime_items r
JOIN title_parsers t ON t.parser_id = r.parser_id
WHERE t.created_from_type = 'subscription'
  AND t.created_from_id IS DISTINCT FROM r.subscription_id;

COMMIT;
