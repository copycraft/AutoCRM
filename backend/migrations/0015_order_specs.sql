-- The build specification: what was actually fitted, and to what temperature.
--
-- Which section the order form shows is configuration, not code: `project_types.spec_form`
-- says whether a project type is a heating build, a cooling build, or neither. The office
-- can retype a project type in settings without a deploy, exactly as it can already rename
-- one, and a repair job gets no spec section at all.
--
-- One table with typed columns rather than JSONB: reports have to be able to ask "how many
-- FRC bodies did we build last year" in SQL, and `docs/DECISIONS.md` already rejects
-- key/value storage for settings on the same grounds.

ALTER TABLE project_types
    ADD COLUMN spec_form TEXT CHECK (spec_form IS NULL OR spec_form IN ('heating', 'cooling'));
COMMENT ON COLUMN project_types.spec_form IS
    'Which build-spec section the order form shows. NULL = none (repairs, other work).';

-- Placeholder, like the project types themselves. Change it in settings once the real
-- categories are known; nothing in code depends on which type carries which form.
UPDATE project_types SET spec_form = 'cooling'
 WHERE key IN ('refrigerated_body', 'van_conversion', 'unit_install');
INSERT INTO project_types (key, label_hu, position, spec_form)
VALUES ('heated_body', 'Fűtött felépítmény', 35, 'heating')
ON CONFLICT (key) DO NOTHING;

CREATE TABLE order_specs (
    order_id           BIGINT PRIMARY KEY REFERENCES orders(id),
    -- Copied from the project type when the spec is written, not looked up later: changing
    -- a project type's form must not silently reinterpret specs already recorded.
    form               TEXT NOT NULL CHECK (form IN ('heating', 'cooling')),

    -- Shared: a heated body and a cooled body both hold a temperature, and both are
    -- insulated. Duplicating these per variant would mean two columns to report on.
    target_temp_c      NUMERIC(4,1) CHECK (target_temp_c IS NULL OR target_temp_c BETWEEN -40 AND 120),
    insulation_mm      INT CHECK (insulation_mm IS NULL OR insulation_mm BETWEEN 0 AND 500),

    -- Cooling only.
    cooling_unit_make  TEXT,
    cooling_unit_model TEXT,
    atp_class          TEXT,                 -- FNA, FRC, FRA … placeholder, free text for now
    compartments       INT CHECK (compartments IS NULL OR compartments BETWEEN 1 AND 5),
    defrost            TEXT CHECK (defrost IS NULL OR defrost IN ('automatic', 'manual', 'hot_gas')),
    electric_standby   BOOLEAN,

    -- Heating only.
    heater_make        TEXT,
    heater_model       TEXT,
    heat_output_kw     NUMERIC(5,1) CHECK (heat_output_kw IS NULL OR heat_output_kw > 0),
    fuel               TEXT CHECK (fuel IS NULL OR fuel IN ('diesel', 'electric', 'lpg', 'engine_coolant')),
    thermostat         BOOLEAN,

    notes              TEXT,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- A cooling spec cannot carry heater fields and vice versa. Enforced here rather than
    -- in a handler, so no future code path can leave a row that means two things at once.
    CONSTRAINT order_specs_cooling_fields_only CHECK (
        form = 'cooling'
        OR num_nonnulls(cooling_unit_make, cooling_unit_model, atp_class, compartments,
                        defrost, electric_standby) = 0
    ),
    CONSTRAINT order_specs_heating_fields_only CHECK (
        form = 'heating'
        OR num_nonnulls(heater_make, heater_model, heat_output_kw, fuel, thermostat) = 0
    )
);
CREATE TRIGGER order_specs_touch BEFORE UPDATE ON order_specs
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
CREATE INDEX order_specs_form_idx ON order_specs (form);
