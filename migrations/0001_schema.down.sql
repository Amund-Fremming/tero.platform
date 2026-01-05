-- Add migration script here

-- Drop foreign key constraints
ALTER TABLE IF EXISTS "saved_game" DROP CONSTRAINT IF EXISTS "fk_saved_game_user";
ALTER TABLE IF EXISTS "saved_game" DROP CONSTRAINT IF EXISTS "fk_saved_game_base";
ALTER TABLE IF EXISTS "quiz_game" DROP CONSTRAINT IF EXISTS "fk_quiz_game_base";
ALTER TABLE IF EXISTS "spin_game" DROP CONSTRAINT IF EXISTS "fk_spin_game_base";

-- Drop indexes
DROP INDEX IF EXISTS "idx_saved_game_id";

DROP INDEX IF EXISTS "idx_system_log_ceverity";

DROP INDEX IF EXISTS "idx_saved_game_delete_keys";

DROP INDEX IF EXISTS "idx_integration_subject";

DROP INDEX IF EXISTS "idx_game_base_id";
DROP INDEX IF EXISTS "idx_game_base_game_type";
DROP INDEX IF EXISTS "idx_game_base_type_and_category";

DROP INDEX IF EXISTS "idx_pseudo_user_id";
DROP INDEX IF EXISTS "idx_pseudo_user_last_active";

DROP INDEX IF EXISTS "idx_base_user_id";
DROP INDEX IF EXISTS "idx_base_user_auth0_id";

-- Drop tables
DROP TABLE IF EXISTS "saved_game";
DROP TABLE IF EXISTS "quiz_game";
DROP TABLE IF EXISTS "spin_game";
DROP TABLE IF EXISTS "spin_game_round";
DROP TABLE IF EXISTS "system_log";
DROP TABLE IF EXISTS "integration";
DROP TABLE IF EXISTS "join_key";
DROP TABLE IF EXISTS "prefix_word";
DROP TABLE IF EXISTS "suffix_word";
DROP TABLE IF EXISTS "base_user";
DROP TABLE IF EXISTS "pseudo_user";
DROP TABLE IF EXISTS "game_base";

-- Drop types
DROP TYPE IF EXISTS "integration_name";
DROP TYPE IF EXISTS "log_ceverity";
DROP TYPE IF EXISTS "log_action";
DROP TYPE IF EXISTS "subject_type";
DROP TYPE IF EXISTS "game_category";
DROP TYPE IF EXISTS "gender";
DROP TYPE IF EXISTS "game_type";