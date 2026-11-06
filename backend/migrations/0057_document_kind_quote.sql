-- Alone in its own migration: `ALTER TYPE ... ADD VALUE` cannot be used by later statements
-- in the same transaction (see 0013_document_kind_certificate.sql for the precedent).
--
-- The quotation PDF. The sales manager writes it outside the CRM and sends it from their
-- own mailbox; the CRM files it on the lead and counts the follow-ups from the day it went
-- out. A kind of its own, so the lead screen can show which file is the quotation.
ALTER TYPE document_kind ADD VALUE IF NOT EXISTS 'quote';
