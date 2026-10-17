-- AI 產生、尚未確認的 filter rule 原本以 target_type='global' 寫進 filter_rules，只靠 pending_result_id 標記，
-- 但套用規則時沒有排除它們 → 確認前就對所有連結生效。確認流程本來就從 pending_ai_results.generated_data
-- 重建規則，這些資料列沒有用處；刪除並移除欄位，讓「待確認的規則」無法再存在於 filter_rules。
DELETE FROM filter_rules WHERE pending_result_id IS NOT NULL;
ALTER TABLE filter_rules DROP COLUMN pending_result_id;
