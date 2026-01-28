-- Restore data to old tables
INSERT INTO fetcher_modules (name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at)
SELECT name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at
FROM service_modules
WHERE module_type = 'fetcher'::module_type;

INSERT INTO downloader_modules (name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at)
SELECT name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at
FROM service_modules
WHERE module_type = 'downloader'::module_type;

INSERT INTO viewer_modules (name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at)
SELECT name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at
FROM service_modules
WHERE module_type = 'viewer'::module_type;

-- Drop the new table and enum type
DROP TABLE service_modules;
DROP TYPE module_type;
