-- anime_links.raw_item_id: 刪除 raw_anime_items 時保留 link，僅清空參照
ALTER TABLE anime_links
  DROP CONSTRAINT IF EXISTS anime_links_raw_item_id_fkey,
  ADD CONSTRAINT anime_links_raw_item_id_fkey
    FOREIGN KEY (raw_item_id) REFERENCES raw_anime_items(item_id) ON DELETE SET NULL;

-- raw_anime_items: 刪除訂閱時一併刪除相關原始項目
ALTER TABLE raw_anime_items
  DROP CONSTRAINT raw_anime_items_subscription_id_fkey,
  ADD CONSTRAINT raw_anime_items_subscription_id_fkey
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(subscription_id) ON DELETE CASCADE;

-- subscription_conflicts: 刪除訂閱時一併刪除相關衝突（確保 CASCADE）
ALTER TABLE subscription_conflicts
  DROP CONSTRAINT subscription_conflicts_subscription_id_fkey,
  ADD CONSTRAINT subscription_conflicts_subscription_id_fkey
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(subscription_id) ON DELETE CASCADE;
