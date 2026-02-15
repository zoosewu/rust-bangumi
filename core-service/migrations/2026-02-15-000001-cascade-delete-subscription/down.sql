-- 恢復為不帶 CASCADE 的外鍵
ALTER TABLE subscription_conflicts
  DROP CONSTRAINT subscription_conflicts_subscription_id_fkey,
  ADD CONSTRAINT subscription_conflicts_subscription_id_fkey
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(subscription_id);

ALTER TABLE raw_anime_items
  DROP CONSTRAINT raw_anime_items_subscription_id_fkey,
  ADD CONSTRAINT raw_anime_items_subscription_id_fkey
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(subscription_id);

ALTER TABLE anime_links
  DROP CONSTRAINT IF EXISTS anime_links_raw_item_id_fkey,
  ADD CONSTRAINT anime_links_raw_item_id_fkey
    FOREIGN KEY (raw_item_id) REFERENCES raw_anime_items(item_id);
