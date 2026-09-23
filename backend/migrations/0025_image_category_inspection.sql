-- Alone in its own migration: `ALTER TYPE ... ADD VALUE` cannot run inside the
-- transaction block a larger migration would put it in (see
-- 0013_document_kind_certificate.sql for the precedent).
--
-- Handover inspections (átadás-átvétel) photograph the car zone by zone. Those photos
-- travel through the same ticket → PUT → complete flow as every other image, so they
-- need a category of their own rather than hiding in production/completion.
ALTER TYPE image_category ADD VALUE IF NOT EXISTS 'inspection';
