-- HR module: the staff directory, visible only to admins and to users an admin has given
-- HR access. Employees are their own records: not everyone on the shop floor has a CRM
-- login, and not every CRM user is an employee.

ALTER TABLE users ADD COLUMN hr_access BOOLEAN NOT NULL DEFAULT false;

CREATE TABLE employees (
    id             BIGSERIAL PRIMARY KEY,
    full_name      TEXT NOT NULL CHECK (btrim(full_name) <> ''),
    email          TEXT,
    company_phone  TEXT,
    personal_phone TEXT,
    -- Object-store key of the one profile picture (a re-encoded square JPEG), if any.
    photo_key      TEXT,
    archived_at    TIMESTAMPTZ,
    created_by     BIGINT REFERENCES users(id),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX employees_name_idx ON employees (lower(full_name));

CREATE TRIGGER employees_touch BEFORE UPDATE ON employees
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
