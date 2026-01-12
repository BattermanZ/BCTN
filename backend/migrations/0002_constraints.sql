PRAGMA foreign_keys = ON;

-- Enforce the "exactly 2 accounts total" rule at the DB layer.
CREATE TRIGGER IF NOT EXISTS trg_app_user_limit_before_insert
BEFORE INSERT ON app_user
BEGIN
  SELECT
    CASE
      WHEN (SELECT COUNT(*) FROM app_user) >= 2 THEN RAISE(ABORT, 'max 2 users')
    END;
END;

-- Enforce allowed values for swipe fields.
CREATE TRIGGER IF NOT EXISTS trg_swipe_validate_before_insert
BEFORE INSERT ON swipe
BEGIN
  SELECT
    CASE
      WHEN NEW.card_kind IN ('group', 'concept') THEN NULL
      ELSE RAISE(ABORT, 'invalid card_kind')
    END;

  SELECT
    CASE
      WHEN NEW.action IN ('like', 'nope', 'superlike') THEN NULL
      ELSE RAISE(ABORT, 'invalid action')
    END;
END;

CREATE TRIGGER IF NOT EXISTS trg_swipe_validate_before_update
BEFORE UPDATE ON swipe
BEGIN
  SELECT
    CASE
      WHEN NEW.card_kind IN ('group', 'concept') THEN NULL
      ELSE RAISE(ABORT, 'invalid card_kind')
    END;

  SELECT
    CASE
      WHEN NEW.action IN ('like', 'nope', 'superlike') THEN NULL
      ELSE RAISE(ABORT, 'invalid action')
    END;
END;

