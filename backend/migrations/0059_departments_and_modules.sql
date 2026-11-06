-- Departments and the technical content (műszaki tartalom) split by them.
--
-- An order's technical content used to be one block. The workshop works in departments —
-- body building, metalwork, refrigeration — and each wants only its own part, on its own
-- sheet, and to hear about it when it is ready for them. So the content is one module per
-- department on the order; each department has a colour, used wherever its module shows.

CREATE TABLE departments (
    id         BIGSERIAL PRIMARY KEY,
    key        TEXT NOT NULL UNIQUE CHECK (key ~ '^[a-z0-9_]+$'),
    name       TEXT NOT NULL CHECK (btrim(name) <> ''),
    color      TEXT NOT NULL CHECK (color ~ '^#[0-9a-fA-F]{6}$'),
    position   INT NOT NULL DEFAULT 0,
    is_active  BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- The client's three, in their colours: metalwork red, refrigeration blue, body grey.
INSERT INTO departments (key, name, color, position) VALUES
    ('felepitmeny', 'Felépítmény', '#64748b', 10),
    ('lakatos',     'Lakatos',     '#dc2626', 20),
    ('hutes',       'Hűtés',       '#2563eb', 30);

-- Who works in which department, and who heads it. A head is told when work is released to
-- the department; a member may edit the department's module on any order.
CREATE TABLE user_departments (
    user_id       BIGINT NOT NULL REFERENCES users(id),
    department_id BIGINT NOT NULL REFERENCES departments(id),
    is_head       BOOLEAN NOT NULL DEFAULT false,
    PRIMARY KEY (user_id, department_id)
);
CREATE INDEX user_departments_department_idx ON user_departments (department_id);

CREATE TABLE order_modules (
    id                 BIGSERIAL PRIMARY KEY,
    order_id           BIGINT NOT NULL REFERENCES orders(id),
    department_id      BIGINT NOT NULL REFERENCES departments(id),
    content            TEXT NOT NULL DEFAULT '',
    -- draft: being written; released: handed to the department (heads told); done: built.
    status             TEXT NOT NULL DEFAULT 'draft'
                           CHECK (status IN ('draft', 'released', 'done')),
    -- When the text last changed, apart from updated_at (which a status change also moves):
    -- content changed after released_at means the department has an outdated version.
    content_updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    released_at        TIMESTAMPTZ,
    released_by        BIGINT REFERENCES users(id),
    updated_by         BIGINT REFERENCES users(id),
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (order_id, department_id)
);
CREATE TRIGGER order_modules_touch BEFORE UPDATE ON order_modules
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
CREATE INDEX order_modules_department_idx ON order_modules (department_id, status);

-- The drawings and photos a module's sheet carries: the order's own files, linked, so one
-- drawing can serve two departments and deleting the link never deletes the file.
CREATE TABLE order_module_files (
    id          BIGSERIAL PRIMARY KEY,
    module_id   BIGINT NOT NULL REFERENCES order_modules(id) ON DELETE CASCADE,
    image_id    BIGINT REFERENCES images(id),
    document_id BIGINT REFERENCES documents(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (num_nonnulls(image_id, document_id) = 1),
    UNIQUE (module_id, image_id),
    UNIQUE (module_id, document_id)
);

-- The letter a department head gets when a module is released to them, or changed after.
INSERT INTO email_templates (key, name, subject, body, is_automatic, category, folder) VALUES
('module_released', 'Műszaki tartalom a részlegnek (belső értesítő)',
 '{{module.department}} – {{module.event}}: {{order.number}} {{order.title}}',
 'Műszaki tartalom a(z) {{module.department}} részlegnek ({{module.event}}).

Projekt: {{order.number}} – {{order.title}}

{{module.content}}

Megnyitás a CRM-ben: {{module.url}}', true, 'projects', 'workflow');
