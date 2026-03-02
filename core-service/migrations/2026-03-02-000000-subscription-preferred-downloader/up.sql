ALTER TABLE subscriptions
ADD COLUMN preferred_downloader_id INTEGER REFERENCES service_modules(module_id) ON DELETE SET NULL;
