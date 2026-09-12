-- Alone in its own migration: `ALTER TYPE ... ADD VALUE` cannot be used by later statements
-- in the same transaction, and some Postgres versions refuse it in a transaction at all.
-- The columns that go with it are in 0014.
ALTER TYPE document_kind ADD VALUE IF NOT EXISTS 'certificate';
