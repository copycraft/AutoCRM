-- Invoice and proforma PDFs live in `documents` like every other file, but they are not
-- "other": the order screen lists them apart, and a proforma must never be mistaken for a
-- tax document. Separate migration because a new enum value cannot be used in the
-- transaction that adds it.
ALTER TYPE document_kind ADD VALUE IF NOT EXISTS 'invoice';
ALTER TYPE document_kind ADD VALUE IF NOT EXISTS 'proforma';
