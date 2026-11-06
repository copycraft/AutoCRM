-- The build questionnaire (műszaki kérdőív): what the office asks about a job, by project
-- type and by the answers so far.
--
-- The client's work is four kinds: boxes, van insulation, hearses and custom projects. What
-- matters differs per kind, and per answer: a body that heats gets the heater questions, one
-- that also cools gets the cooling ones as well, and a freezer adds the freezer ones. Until
-- now a project type carried one fixed form, heating or cooling and never both (0015).
--
-- The questions are configuration now, edited in settings like the walkaround lists. A
-- section of questions belongs to some project types (or to all of them) and is asked
-- either always, or only once a yes/no question is answered yes: "heats" opens the heating
-- section, "cools" the cooling one, and "freezes" inside cooling the freezer one.
--
-- Answers are one JSON object per lead and per order, keyed by question key. The lead's
-- are filled while selling and copied onto the order when the lead is converted; a
-- question marked `required` has to be answered before the lead may become an order.
-- Reports can still ask "how many freezer bodies" in SQL: `spec_answers->>'freezes'`.

-- 1. The client's project types. Only labels still at their placeholder are changed:
--    anything the office renamed in settings is theirs and stays.
UPDATE project_types SET label_hu = 'Doboz', position = 10
 WHERE key = 'refrigerated_body' AND label_hu = 'Hűtőfelépítmény';
UPDATE project_types SET label_hu = 'Furgon szigetelés', position = 20
 WHERE key = 'van_conversion' AND label_hu = 'Furgon hűtősítés';
INSERT INTO project_types (key, label_hu, position) VALUES ('hearse', 'Halottszállító', 30)
ON CONFLICT (key) DO NOTHING;
UPDATE project_types SET label_hu = 'Egyedi projekt', position = 40
 WHERE key = 'other' AND label_hu = 'Egyéb';
-- Heating and unit installs are answers now, not kinds of job; repairs are not work the
-- client listed. Kept for the orders that carry them; the office can switch them back on.
UPDATE project_types SET is_active = false
 WHERE (key, label_hu) IN (('unit_install', 'Hűtőgép beépítés'),
                           ('heated_body', 'Fűtött felépítmény'),
                           ('repair', 'Javítás / átalakítás'));

-- 2. The questionnaire.
CREATE TABLE spec_sections (
    id         BIGSERIAL PRIMARY KEY,
    key        TEXT NOT NULL UNIQUE CHECK (key ~ '^[a-z][a-z0-9_]*$'),
    title      TEXT NOT NULL CHECK (char_length(btrim(title)) BETWEEN 1 AND 100),
    position   INT NOT NULL DEFAULT 0,
    is_active  BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Which project types a section is asked for. A section with no rows here is asked for
-- every type, and also before a type is chosen.
CREATE TABLE spec_section_project_types (
    section_id      BIGINT NOT NULL REFERENCES spec_sections(id) ON DELETE CASCADE,
    project_type_id BIGINT NOT NULL REFERENCES project_types(id) ON DELETE CASCADE,
    PRIMARY KEY (section_id, project_type_id)
);

CREATE TABLE spec_questions (
    id         BIGSERIAL PRIMARY KEY,
    section_id BIGINT NOT NULL REFERENCES spec_sections(id) ON DELETE CASCADE,
    -- The answer's name in the JSON. Never changes, like a stage key, and is unique across
    -- sections: the answers are one flat object.
    key        TEXT NOT NULL UNIQUE CHECK (key ~ '^[a-z][a-z0-9_]*$'),
    label      TEXT NOT NULL CHECK (char_length(btrim(label)) BETWEEN 1 AND 200),
    -- One line under the field: an example, or what to measure.
    help       TEXT,
    kind       TEXT NOT NULL CHECK (kind IN ('yes_no', 'number', 'text', 'long_text', 'choice', 'multi_choice')),
    -- Shown next to a number: °C, mm, kW.
    unit       TEXT,
    options    TEXT[] NOT NULL DEFAULT '{}',
    min_value  NUMERIC,
    max_value  NUMERIC,
    -- Has to be answered before the lead may become an order.
    required   BOOLEAN NOT NULL DEFAULT false,
    position   INT NOT NULL DEFAULT 0,
    is_active  BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT spec_questions_options_for_choices
        CHECK (kind IN ('choice', 'multi_choice') OR cardinality(options) = 0),
    CONSTRAINT spec_questions_choices_have_options
        CHECK (kind NOT IN ('choice', 'multi_choice') OR cardinality(options) > 0),
    CONSTRAINT spec_questions_range_for_numbers
        CHECK (kind = 'number' OR (min_value IS NULL AND max_value IS NULL)),
    CONSTRAINT spec_questions_range_order
        CHECK (min_value IS NULL OR max_value IS NULL OR min_value <= max_value)
);
CREATE INDEX spec_questions_section_idx ON spec_questions (section_id, position, id);

-- Asked only once this yes/no question is answered yes; NULL: always asked. That the
-- question is a yes/no one, and that no section ends up waiting on itself, is checked
-- where it is set (api::questionnaire).
ALTER TABLE spec_sections
    ADD COLUMN shown_when_question_id BIGINT REFERENCES spec_questions(id);

-- 3. Where the answers live. The lead also learns its project type, so the right sections
--    can be asked before the conversion.
ALTER TABLE leads
    ADD COLUMN project_type_id BIGINT REFERENCES project_types(id),
    ADD COLUMN spec_answers JSONB NOT NULL DEFAULT '{}'
        CONSTRAINT leads_spec_answers_object CHECK (jsonb_typeof(spec_answers) = 'object');
ALTER TABLE orders
    ADD COLUMN spec_answers JSONB NOT NULL DEFAULT '{}'
        CONSTRAINT orders_spec_answers_object CHECK (jsonb_typeof(spec_answers) = 'object');

-- 4. The starting questionnaire. Placeholder wording until the client has gone through it;
--    every part of it can be changed in settings.
INSERT INTO spec_sections (key, title, position) VALUES
    ('general', 'Alapok',            10),
    ('box',     'Doboz',             20),
    ('van',     'Furgon szigetelés', 20),
    ('hearse',  'Halottszállító',    20),
    ('custom',  'Egyedi projekt',    20),
    ('cooling', 'Hűtés',             30),
    ('freezer', 'Fagyasztás',        40),
    ('heating', 'Fűtés',             50);

INSERT INTO spec_section_project_types (section_id, project_type_id)
SELECT s.id, pt.id
  FROM spec_sections s
  JOIN project_types pt ON pt.key = CASE s.key
       WHEN 'box' THEN 'refrigerated_body'
       WHEN 'van' THEN 'van_conversion'
       WHEN 'hearse' THEN 'hearse'
       WHEN 'custom' THEN 'other'
  END;

INSERT INTO spec_questions
    (section_id, key, label, help, kind, unit, options, min_value, max_value, required, position)
SELECT s.id, q.key, q.label, q.help, q.kind, q.unit, q.options, q.min_value, q.max_value, q.required, q.position
  FROM (VALUES
    ('general', 'heats',          'Fűteni kell?',                 NULL, 'yes_no',    NULL, '{}'::text[], NULL::numeric, NULL::numeric, true,  10),
    ('general', 'cools',          'Hűteni kell?',                 NULL, 'yes_no',    NULL, '{}', NULL, NULL, true,  20),
    ('general', 'insulation_mm',  'Szigetelés vastagsága',        NULL, 'number',    'mm', '{}', 0, 500, false, 30),
    ('general', 'spec_notes',     'Megjegyzés a kivitelhez',      NULL, 'long_text', NULL, '{}', NULL, NULL, false, 90),

    ('box', 'box_length_mm',  'Belső hossz',          NULL, 'number', 'mm', '{}', 1, 20000, true,  10),
    ('box', 'box_width_mm',   'Belső szélesség',      NULL, 'number', 'mm', '{}', 1, 3000,  true,  20),
    ('box', 'box_height_mm',  'Belső magasság',       NULL, 'number', 'mm', '{}', 1, 4000,  true,  30),
    ('box', 'box_rear_door',  'Hátsó ajtó',           NULL, 'choice', NULL, '{Kétszárnyú,Redőny,Nincs}', NULL, NULL, false, 40),
    ('box', 'box_side_doors', 'Oldalajtók száma',     NULL, 'number', 'db', '{}', 0, 6, false, 50),
    ('box', 'box_tail_lift',  'Hátfalemelő',          NULL, 'yes_no', NULL, '{}', NULL, NULL, false, 60),
    ('box', 'box_meat_rails', 'Húsakasztó sín',       NULL, 'yes_no', NULL, '{}', NULL, NULL, false, 70),

    ('van', 'van_size',            'Méret (hossz / magasság)',  'pl. L2H2', 'text',   NULL, '{}', NULL, NULL, true,  10),
    ('van', 'van_partition',       'Válaszfal a fülke mögött',  NULL,       'yes_no', NULL, '{}', NULL, NULL, false, 20),
    ('van', 'van_floor_insulated', 'Padló szigetelése',         NULL,       'yes_no', NULL, '{}', NULL, NULL, false, 30),
    ('van', 'van_lighting',        'Belső LED-világítás',       NULL,       'yes_no', NULL, '{}', NULL, NULL, false, 40),

    ('hearse', 'hearse_coffins',        'Koporsóhelyek száma',          NULL, 'number', 'db', '{}', 1, 6, true, 10),
    ('hearse', 'hearse_loading',        'Rakodórendszer',               'pl. görgős padló, kihúzható tálca', 'text', NULL, '{}', NULL, NULL, false, 20),
    ('hearse', 'hearse_partition',      'Válaszfal a fülke mögött',     NULL, 'yes_no', NULL, '{}', NULL, NULL, false, 30),
    ('hearse', 'hearse_washable_floor', 'Mosható, fertőtleníthető padló', NULL, 'yes_no', NULL, '{}', NULL, NULL, false, 40),

    ('custom', 'custom_scope', 'Mit kell elkészíteni?', NULL, 'long_text', NULL, '{}', NULL, NULL, true, 10),

    ('cooling', 'cool_temp_c',         'Hűtött hőmérséklet',       NULL,      'number', '°C', '{}', -40, 30, true,  10),
    ('cooling', 'cooling_unit_make',   'Hűtőgép gyártója',         NULL,      'text',   NULL, '{}', NULL, NULL, false, 20),
    ('cooling', 'cooling_unit_model',  'Hűtőgép típusa',           NULL,      'text',   NULL, '{}', NULL, NULL, false, 30),
    ('cooling', 'atp_class',           'ATP osztály',              'pl. FRC', 'text',   NULL, '{}', NULL, NULL, false, 40),
    ('cooling', 'compartments',        'Rekeszek száma',           NULL,      'number', 'db', '{}', 1, 5, false, 50),
    ('cooling', 'defrost',             'Leolvasztás',              NULL,      'choice', NULL, '{Automatikus,Kézi,Forrógázas}', NULL, NULL, false, 60),
    ('cooling', 'electric_standby',    'Hálózati (elektromos) üzem', NULL,    'yes_no', NULL, '{}', NULL, NULL, false, 70),
    ('cooling', 'freezes',             'Fagyasztani is kell?',     NULL,      'yes_no', NULL, '{}', NULL, NULL, true,  80),
    ('cooling', 'cooling_unit_serial', 'Hűtőgép gyári száma',      'A telefonon is felvehető, a gép adattáblájáról.', 'text', NULL, '{}', NULL, NULL, false, 90),

    ('freezer', 'freeze_temp_c',   'Fagyasztott hőmérséklet',          NULL, 'number', '°C', '{}', -40, 0, true,  10),
    ('freezer', 'freeze_separate', 'Külön fagyasztott rekesz (osztott raktér)', NULL, 'yes_no', NULL, '{}', NULL, NULL, false, 20),

    ('heating', 'heat_temp_c',    'Fűtött hőmérséklet', NULL, 'number', '°C', '{}', 0, 120, true,  10),
    ('heating', 'heater_make',    'Fűtőkészülék gyártója', NULL, 'text', NULL, '{}', NULL, NULL, false, 20),
    ('heating', 'heater_model',   'Fűtőkészülék típusa',   NULL, 'text', NULL, '{}', NULL, NULL, false, 30),
    ('heating', 'heat_output_kw', 'Teljesítmény',  NULL, 'number', 'kW', '{}', 0.1, 100, false, 40),
    ('heating', 'fuel',           'Energiaforrás', NULL, 'choice', NULL, '{Dízel,Elektromos,LPG,Motorhűtőfolyadék}', NULL, NULL, false, 50),
    ('heating', 'thermostat',     'Termosztát',    NULL, 'yes_no', NULL, '{}', NULL, NULL, false, 60)
  ) AS q(section, key, label, help, kind, unit, options, min_value, max_value, required, position)
  JOIN spec_sections s ON s.key = q.section;

UPDATE spec_sections s SET shown_when_question_id = q.id
  FROM spec_questions q
 WHERE (s.key, q.key) IN (('cooling', 'cools'), ('freezer', 'freezes'), ('heating', 'heats'));

-- 5. The specs already recorded, as answers. The temperature goes to the side it was
--    recorded for; the defrost and fuel keys become the labels the choices offer.
UPDATE orders o
   SET spec_answers = jsonb_strip_nulls(jsonb_build_object(
        'heats', s.form = 'heating',
        'cools', s.form = 'cooling',
        'insulation_mm', s.insulation_mm,
        'spec_notes', s.notes,
        'cool_temp_c', CASE WHEN s.form = 'cooling' THEN s.target_temp_c END,
        'cooling_unit_make', s.cooling_unit_make,
        'cooling_unit_model', s.cooling_unit_model,
        'atp_class', s.atp_class,
        'compartments', s.compartments,
        'defrost', CASE s.defrost WHEN 'automatic' THEN 'Automatikus' WHEN 'manual' THEN 'Kézi'
                                  WHEN 'hot_gas' THEN 'Forrógázas' END,
        'electric_standby', s.electric_standby,
        'cooling_unit_serial', s.cooling_unit_serial,
        'heat_temp_c', CASE WHEN s.form = 'heating' THEN s.target_temp_c END,
        'heater_make', s.heater_make,
        'heater_model', s.heater_model,
        'heat_output_kw', s.heat_output_kw,
        'fuel', CASE s.fuel WHEN 'diesel' THEN 'Dízel' WHEN 'electric' THEN 'Elektromos'
                            WHEN 'lpg' THEN 'LPG' WHEN 'engine_coolant' THEN 'Motorhűtőfolyadék' END,
        'thermostat', s.thermostat
   ))
  FROM order_specs s
 WHERE s.order_id = o.id;

DROP TABLE order_specs;
ALTER TABLE project_types DROP COLUMN spec_form;
