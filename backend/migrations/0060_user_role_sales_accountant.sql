-- Alone in its own migration: `ALTER TYPE ... ADD VALUE` cannot be used by later statements
-- in the same transaction (see 0013_document_kind_certificate.sql for the precedent).
--
-- Two more roles. Értékesítő (sales): leads, quotes and customers, with prices but without
-- the invoicing. Könyvelő (accountant): invoices, payments and supplier invoices, without
-- touching production.
ALTER TYPE user_role ADD VALUE IF NOT EXISTS 'sales';
ALTER TYPE user_role ADD VALUE IF NOT EXISTS 'accountant';
