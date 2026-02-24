-- ============================================================================
-- Drop anime_cover_images table
-- ============================================================================
DROP TABLE IF EXISTS anime_cover_images;

-- NOTE: PostgreSQL does not support removing enum values once added.
-- The 'metadata' value added to module_type cannot be reverted via migration.
-- To fully revert, you would need to recreate the enum type manually:
--   1. Create a new enum without 'metadata'
--   2. Update all columns using the old enum to use the new one
--   3. Drop the old enum and rename the new one
