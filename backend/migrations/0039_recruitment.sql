-- HR recruitment: job listings, each with a generic public application form, and the
-- applicant profiles the form creates. The form is reached by the listing's link; the
-- applicant's resume is attached to their profile as they submit.

CREATE TABLE job_postings (
    id           BIGSERIAL PRIMARY KEY,
    title        TEXT NOT NULL CHECK (btrim(title) <> ''),
    description  TEXT,
    location     TEXT,
    -- The unguessable path segment of the public link: a draft's link cannot be found by
    -- counting up from the id.
    slug         TEXT NOT NULL UNIQUE,
    -- draft: link exists, form refuses applications. published: form open. closed: form
    -- shows "no longer accepting".
    status       TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'published', 'closed')),
    published_at TIMESTAMPTZ,
    created_by   BIGINT REFERENCES users(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER job_postings_touch BEFORE UPDATE ON job_postings
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

CREATE TABLE job_applications (
    id                  BIGSERIAL PRIMARY KEY,
    posting_id          BIGINT NOT NULL REFERENCES job_postings(id),
    full_name           TEXT NOT NULL CHECK (btrim(full_name) <> ''),
    email               TEXT NOT NULL,
    phone               TEXT NOT NULL,
    age                 INT NOT NULL CHECK (age BETWEEN 14 AND 100),
    city                TEXT,
    message             TEXT,
    -- Object-store key of the uploaded resume; the original file, never re-encoded.
    resume_key          TEXT NOT NULL,
    resume_filename     TEXT NOT NULL,
    resume_content_type TEXT NOT NULL,
    -- HR's own notes, e.g. from the phone interview.
    notes               TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX job_applications_posting_idx ON job_applications (posting_id, created_at DESC);

CREATE TRIGGER job_applications_touch BEFORE UPDATE ON job_applications
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
