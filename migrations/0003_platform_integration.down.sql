-- Postgres does not support removing ENUM values directly.
-- To roll back, recreate the type without 'platform'.
ALTER TYPE "integration_name" RENAME TO "integration_name_old";

CREATE TYPE "integration_name" AS ENUM ('auth0', 'session');

ALTER TABLE "system_log"
    ALTER COLUMN integration_name TYPE "integration_name"
    USING integration_name::text::"integration_name";

DROP TYPE "integration_name_old";
