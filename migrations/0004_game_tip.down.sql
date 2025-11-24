-- Revert game_tip table

DROP INDEX IF EXISTS "idx_game_tip_created_at";
DROP TABLE IF EXISTS "game_tip";
