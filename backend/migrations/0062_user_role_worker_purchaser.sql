-- Alone in its own migration: `ALTER TYPE ... ADD VALUE` cannot be used by later statements
-- in the same transaction (see 0013_document_kind_certificate.sql for the precedent).
--
-- Munkás (worker): builds the job — photos, comments, their department's part.
-- Anyagbeszerző (purchaser): buys the materials, and takes the vehicles in and hands them
-- back.
ALTER TYPE user_role ADD VALUE IF NOT EXISTS 'worker';
ALTER TYPE user_role ADD VALUE IF NOT EXISTS 'purchaser';
