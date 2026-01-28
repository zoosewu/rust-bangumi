-- Create ENUM type for module type
CREATE TYPE module_type AS ENUM ('fetcher', 'downloader', 'viewer');

-- Create the consolidated service_modules table
CREATE TABLE service_modules (
  module_id SERIAL PRIMARY KEY,
  module_type module_type NOT NULL,
  name VARCHAR(255) NOT NULL UNIQUE,
  version VARCHAR(50) NOT NULL,
  description TEXT,
  is_enabled BOOLEAN NOT NULL DEFAULT true,
  config_schema TEXT,
  priority INT NOT NULL DEFAULT 50,
  base_url VARCHAR(255) NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes
CREATE INDEX idx_service_modules_module_type ON service_modules(module_type);
CREATE INDEX idx_service_modules_base_url ON service_modules(base_url);
CREATE INDEX idx_service_modules_name_type ON service_modules(name, module_type);

-- Migrate data from fetcher_modules
INSERT INTO service_modules (module_type, name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at)
SELECT 'fetcher'::module_type, name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at
FROM fetcher_modules;

-- Migrate data from downloader_modules
INSERT INTO service_modules (module_type, name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at)
SELECT 'downloader'::module_type, name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at
FROM downloader_modules;

-- Migrate data from viewer_modules
INSERT INTO service_modules (module_type, name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at)
SELECT 'viewer'::module_type, name, version, description, is_enabled, config_schema, priority, base_url, created_at, updated_at
FROM viewer_modules;
