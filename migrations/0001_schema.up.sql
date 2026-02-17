-- Add migration script here
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TYPE "game_type" AS ENUM (
    'roulette',
    'duel',
    'quiz',
    'imposter'
);

CREATE TYPE "integration_name" AS ENUM (
    'auth0',
    'session'
);

CREATE TYPE "log_ceverity" AS ENUM (
    'critical',
    'warning',
    'info'
);

CREATE TYPE "log_action" AS ENUM (
    'create',
    'read',
    'update',
    'delete',
    'sync',
    'other'
);

CREATE TYPE "subject_type" AS ENUM (
    'registered_user',
    'guest_user',
    'integration',
    'system'
);

CREATE TYPE game_category AS ENUM (
    'girls',
    'boys',
    'mixed',
    'innercircle'
);

CREATE TYPE gender AS ENUM (
    'm',
    'f',
    'u'   
);

CREATE TABLE "saved_game" (
    "id" UUID PRIMARY KEY,
    "user_id" UUID NOT NULL,
    "base_id" UUID NOT NULL,
    UNIQUE ("base_id", "user_id")
);

CREATE TABLE "game_tip" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    "header" VARCHAR(100) NOT NULL,
    "mobile_phone" VARCHAR(20) NOT NULL,
    "description" VARCHAR(500) NOT NULL,
    "created_at" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE "system_log" (
    "id" BIGSERIAL PRIMARY KEY,
    "subject_id" VARCHAR(100) NOT NULL,
    "subject_type" subject_type NOT NULL,
    "action" log_action NOT NULL,
    "ceverity" log_ceverity NOT NULL,
    "function" VARCHAR(50) NOT NULL,
    "description" VARCHAR(512) NOT NULL,
    "metadata" JSONB,
    "created_at" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE "prefix_word" (
    "word" VARCHAR(5) PRIMARY KEY
);

CREATE TABLE "suffix_word" (
    "word" VARCHAR(5) PRIMARY KEY
);

CREATE TABLE "pseudo_user" (
    "id" UUID UNIQUE NOT NULL DEFAULT uuid_generate_v4(),
    "last_active" TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE "base_user" (
    "id" UUID NOT NULL UNIQUE DEFAULT uuid_generate_v4(),
    "username" VARCHAR (100) NOT NULL,
    "auth0_id" VARCHAR,
    "birth_date" DATE,
    "gender" gender NOT NULL DEFAULT 'u',
    "email" VARCHAR(150),
    "email_verified" BOOLEAN,
    "family_name" VARCHAR(100),
    "updated_at" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    "given_name" VARCHAR(100),
    "created_at" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE "game_base" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    "name" VARCHAR(100) NOT NULL,
    "game_type" game_type NOT NULL,
    "category" game_category NOT NULL DEFAULT 'mixed',
    "iterations" INTEGER NOT NULL DEFAULT 0,
    "times_played" INTEGER NOT NULL DEFAULT 0,
    "last_played" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    "synced" BOOLEAN NOT NULL DEFAULT false
);

CREATE TABLE "quiz_game" (
    "id" UUID PRIMARY KEY,
    "rounds" TEXT[] NOT NULL
);

CREATE TABLE "spin_game" (
    "id" UUID PRIMARY KEY,
    "rounds" TEXT[] NOT NULL
);

CREATE TABLE "imposter_game" (
    "id" UUID PRIMARY KEY,
    "rounds" TEXT[] NOT NULL
);

CREATE TABLE "random_game" (
    "id" BIGSERIAL PRIMARY KEY,
    "game_id" UUID NOT NULL,
    "rounds" TEXT[] NOT NULL,
    "game_type" game_type NOT NULL
);

CREATE INDEX "idx_random_game_id_game_type" ON "random_game" ("id", "game_type");

CREATE INDEX "idx_saved_game_id" ON "saved_game" ("id");
CREATE INDEX "idx_saved_game_delete_keys" ON "saved_game" ("id", "user_id");

CREATE INDEX "idx_system_log_ceverity" ON "system_log" ("ceverity", "created_at" DESC);

CREATE INDEX "idx_game_tip_created_at" ON "game_tip" ("created_at" DESC);

CREATE INDEX "idx_game_base_id" ON "game_base" ("id");
CREATE INDEX "idx_game_base_game_type" ON "game_base" ("game_type", "times_played" DESC);
CREATE INDEX "idx_game_base_type_and_category" ON "game_base" ("game_type", "category", "times_played" DESC);

CREATE INDEX "idx_pseudo_user_id" ON "pseudo_user" ("id");
CREATE INDEX "idx_pseudo_user_last_active" ON "pseudo_user" ("last_active");

CREATE INDEX "idx_base_user_id" ON "base_user" ("id");
CREATE INDEX "idx_base_user_auth0_id" ON "base_user" ("auth0_id");

ALTER TABLE "saved_game" 
ADD CONSTRAINT "fk_saved_game_user" 
FOREIGN KEY ("user_id") REFERENCES "base_user"("id") ON DELETE CASCADE;

ALTER TABLE "saved_game"
ADD CONSTRAINT "fk_saved_game_base" 
FOREIGN KEY ("base_id") REFERENCES "game_base"("id") ON DELETE CASCADE;

ALTER TABLE "quiz_game"
ADD CONSTRAINT "fk_quiz_game_base" 
FOREIGN KEY ("id") REFERENCES "game_base"("id") ON DELETE CASCADE;

ALTER TABLE "spin_game"
ADD CONSTRAINT "fk_spin_game_base" 
FOREIGN KEY ("id") REFERENCES "game_base"("id") ON DELETE CASCADE;

ALTER TABLE "imposter_game"
ADD CONSTRAINT "fk_imposter_game_base" 
FOREIGN KEY ("id") REFERENCES "game_base"("id") ON DELETE CASCADE;