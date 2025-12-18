-- Add up migration script here
-- Make sure pgcrypto extension is enabled for gen_random_uuid()
-- CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE EXTENSION IF NOT EXISTS pgcrypto;

insert into "integration" ("subject", "name") values ('NmMLjTtX0XrnWjbe1JbRAgwEmn5lP3Sg@clients', 'session');
