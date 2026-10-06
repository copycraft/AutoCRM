-- Where each employee stands, in the lists HR kept in MiniCRM: absent (parental benefit,
-- long-term...), waiting to be filed, active (and which papers are still missing), left.
-- One status per employee. The lists are configuration: HR renames, recolours, adds.
--
-- Two flags tie a status to the employee record: `ends_employment` (Megszűnt jogviszony)
-- is the status an archived employee has, and choosing it archives them; `is_default`
-- (Aktív munkavállaló) is where a new or returning employee starts.
CREATE TABLE employee_statuses (
    id              BIGSERIAL PRIMARY KEY,
    section         TEXT NOT NULL CHECK (btrim(section) <> ''),
    label           TEXT NOT NULL CHECK (btrim(label) <> ''),
    color           TEXT NOT NULL DEFAULT '#dde1e6' CHECK (color ~ '^#[0-9a-f]{6}$'),
    position        INT NOT NULL DEFAULT 0,
    ends_employment BOOLEAN NOT NULL DEFAULT false,
    is_default      BOOLEAN NOT NULL DEFAULT false,
    archived_at     TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (NOT (ends_employment AND is_default))
);
CREATE UNIQUE INDEX employee_statuses_label_idx
    ON employee_statuses (lower(section), lower(label)) WHERE archived_at IS NULL;

INSERT INTO employee_statuses (section, label, color, position, ends_employment, is_default) VALUES
    ('Távollévő munkavállaló', 'Szülői ellátásban részesülő',          '#5aa05a', 10,  false, false),
    ('Távollévő munkavállaló', 'Munkabaleseti ellátásban részesülő',   '#4f8a52', 20,  false, false),
    ('Távollévő munkavállaló', 'Tartósan távollévő (egyéb ok)',        '#2e4a30', 30,  false, false),
    ('Távollévő munkavállaló', 'Fizetés nélküli, egy hónapnál hosszabb', '#1e3320', 40, false, false),
    ('Iktatásra váró',         'Iktatásra vár',                        '#c9402a', 1010, false, false),
    ('Iktatásra váró',         'Állásajánlatra jelentkező',            '#f7dcb4', 1020, false, false),
    ('Aktív munkavállaló',     'Személyi adatokra vár',                '#93bcff', 2010, false, false),
    ('Aktív munkavállaló',     'Pénzügyi adatokra vár',                '#3d6fd6', 2020, false, false),
    ('Aktív munkavállaló',     'Családi, szociális adatokra vár',      '#2e55a8', 2030, false, false),
    ('Aktív munkavállaló',     'Aktív munkavállaló',                   '#1f3a75', 2040, false, true),
    ('Aktív munkavállaló',     'Társas vállalkozás közreműködő tagja', '#dde1e6', 2050, false, false),
    ('Megszűnt',               'Megszűnt jogviszony',                  '#2b313b', 3010, true,  false);

ALTER TABLE employees ADD COLUMN status_id BIGINT REFERENCES employee_statuses(id);
UPDATE employees SET status_id = (SELECT id FROM employee_statuses
                                   WHERE CASE WHEN employees.archived_at IS NULL
                                              THEN is_default ELSE ends_employment END)
 WHERE status_id IS NULL;
CREATE INDEX employees_status_idx ON employees (status_id);
