-- Alone in its own migration: `ALTER TYPE ... ADD VALUE` cannot be used by later statements
-- in the same transaction (see 0013_document_kind_certificate.sql for the precedent).
--
-- Short walkaround clips recorded during a handover inspection are documents of their own
-- kind, so they are never mistaken for a design file or offered as an email attachment.
ALTER TYPE document_kind ADD VALUE IF NOT EXISTS 'video';
