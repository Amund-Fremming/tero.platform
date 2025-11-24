-- Add game_tip table for storing user-submitted tips about games

CREATE TABLE "game_tip" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    "header" VARCHAR(100) NOT NULL,
    "mobile_phone" VARCHAR(20) NOT NULL,
    "description" VARCHAR(500) NOT NULL,
    "created_at" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX "idx_game_tip_created_at" ON "game_tip" ("created_at" DESC);
