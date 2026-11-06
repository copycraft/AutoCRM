-- Files filed per project on a browsable share (STORAGE_BACKEND=fs, usually NFS): every
-- order, enquiry, employee and job posting gets one folder, named once from what was known
-- then ("projektek/2026/2026-0042_Kovacs-Kft_ABC-123"), so a later rename of the customer
-- never splits a project across two folders.

CREATE TABLE storage_folders (
    entity     TEXT NOT NULL CHECK (entity IN ('order', 'lead', 'employee', 'job_posting')),
    entity_id  BIGINT NOT NULL,
    folder     TEXT NOT NULL CHECK (folder ~ '^[A-Za-z0-9][A-Za-z0-9._/-]*$'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (entity, entity_id)
);
-- lower(): the share is also browsed from Windows, where "A1" and "a1" are one folder.
CREATE UNIQUE INDEX storage_folders_folder_key ON storage_folders (lower(folder));

-- A file moved to its project folder (re-sorting, a lead becoming an order) leaves its old
-- key here, so a download link issued before the move still finds it.
CREATE TABLE storage_moves (
    old_key  TEXT PRIMARY KEY,
    new_key  TEXT NOT NULL,
    moved_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX storage_moves_moved_idx ON storage_moves (moved_at);

-- Re-sorting moves an intake photo too: same bytes (checked against content_hash before
-- the row changes), new place. Only a transaction that says so with
-- `SET LOCAL autocrm.relocating = 'on'` may change the key; everything else stays frozen.
CREATE OR REPLACE FUNCTION protect_immutable_images() RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        IF OLD.immutable THEN
            RAISE EXCEPTION 'image % is immutable and cannot be deleted', OLD.id USING ERRCODE = 'AC001';
        END IF;
        RETURN OLD;
    END IF;

    IF OLD.immutable AND (
           NEW.immutable IS DISTINCT FROM OLD.immutable
        OR NEW.order_id IS DISTINCT FROM OLD.order_id
        OR NEW.category IS DISTINCT FROM OLD.category
        OR (NEW.storage_key IS DISTINCT FROM OLD.storage_key
            AND coalesce(current_setting('autocrm.relocating', true), '') <> 'on')
        OR NEW.content_hash IS DISTINCT FROM OLD.content_hash
        OR NEW.byte_size IS DISTINCT FROM OLD.byte_size
        OR NEW.uploaded_at IS DISTINCT FROM OLD.uploaded_at
        OR NEW.uploaded_by IS DISTINCT FROM OLD.uploaded_by
        OR NEW.deleted_at IS NOT NULL
    ) THEN
        RAISE EXCEPTION 'image % is immutable', OLD.id USING ERRCODE = 'AC001';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
