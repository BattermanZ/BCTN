PRAGMA foreign_keys = ON;

-- Previous admin update logic could write deleted_at=0, which makes the user appear deleted
-- (queries rely on `deleted_at IS NULL`). Repair those rows.
UPDATE app_user SET deleted_at = NULL WHERE deleted_at = 0;

