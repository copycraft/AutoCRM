-- Images and documents: metadata here, bytes in object storage.

CREATE TABLE images (
    id                BIGSERIAL PRIMARY KEY,
    order_id          BIGINT NOT NULL REFERENCES orders(id),
    category          image_category NOT NULL,
    -- The original, byte-for-byte as uploaded (EXIF included). Evidence lives here.
    storage_key       TEXT NOT NULL,
    -- Derived copies: re-encoded, EXIF/GPS stripped. What the UI and emails use.
    display_key       TEXT,
    thumb_key         TEXT,
    content_type      TEXT NOT NULL,
    original_filename TEXT,
    content_hash      BYTEA NOT NULL CHECK (length(content_hash) = 32),   -- sha256 of the original
    byte_size         BIGINT NOT NULL CHECK (byte_size > 0),
    width             INT,
    height            INT,
    captured_at       TIMESTAMPTZ,                                       -- from EXIF where available
    processed_at      TIMESTAMPTZ,
    processing_error  TEXT,
    source_ref        TEXT,                                              -- migration provenance (MiniCRM URL)
    uploaded_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    uploaded_by       BIGINT REFERENCES users(id),
    immutable         BOOLEAN NOT NULL,
    deleted_at        TIMESTAMPTZ,
    deleted_by        BIGINT REFERENCES users(id),
    -- Intake photos document pre-existing damage: always write-once.
    CHECK (category <> 'intake' OR immutable)
);
-- Re-uploading the same photo to the same order is the same image: this makes
-- mobile batch retries idempotent.
CREATE UNIQUE INDEX images_order_hash_key ON images (order_id, content_hash) WHERE deleted_at IS NULL;
CREATE INDEX images_order_idx ON images (order_id, category, captured_at) WHERE deleted_at IS NULL;

-- Enforced in the database, not just the API: an immutable image's identity and bytes
-- cannot be changed or removed by any code path, including a future bug or a manual UPDATE.
-- Derived-copy columns (display/thumb keys, dimensions, processing status) stay writable.
CREATE FUNCTION protect_immutable_images() RETURNS trigger AS $$
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
        OR NEW.storage_key IS DISTINCT FROM OLD.storage_key
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

CREATE TRIGGER images_immutable
    BEFORE UPDATE OR DELETE ON images
    FOR EACH ROW EXECUTE FUNCTION protect_immutable_images();

CREATE TYPE document_kind AS ENUM ('design', 'cad', 'other');

CREATE TABLE documents (
    id           BIGSERIAL PRIMARY KEY,
    order_id     BIGINT NOT NULL REFERENCES orders(id),
    kind         document_kind NOT NULL,
    filename     TEXT NOT NULL,
    content_type TEXT NOT NULL,
    storage_key  TEXT NOT NULL,
    content_hash BYTEA NOT NULL CHECK (length(content_hash) = 32),
    byte_size    BIGINT NOT NULL CHECK (byte_size > 0),
    source_ref   TEXT,
    uploaded_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    uploaded_by  BIGINT REFERENCES users(id),
    deleted_at   TIMESTAMPTZ,
    deleted_by   BIGINT REFERENCES users(id)
);
CREATE INDEX documents_order_idx ON documents (order_id, uploaded_at DESC) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX documents_order_hash_key ON documents (order_id, content_hash) WHERE deleted_at IS NULL;
