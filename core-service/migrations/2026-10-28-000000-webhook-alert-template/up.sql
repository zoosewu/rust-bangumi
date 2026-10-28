-- 異常通知模板：有值的 webhook 也會收到異常通知（下載器連不上、派送 / 同步 / AI 生成失敗）
ALTER TABLE webhooks ADD COLUMN alert_template TEXT;
