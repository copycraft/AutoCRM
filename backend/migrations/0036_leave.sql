-- HR: leave and absence. HR (anyone with AccessHr) records it for employees; employees
-- have no login of their own here.

-- Days of paid annual leave per calendar year. 20 is the statutory base in Hungary; age and
-- children add days, which HR sets per employee.
ALTER TABLE employees
    ADD COLUMN annual_leave_days INT NOT NULL DEFAULT 20
        CHECK (annual_leave_days BETWEEN 0 AND 366);

CREATE TABLE absences (
    id          BIGSERIAL PRIMARY KEY,
    employee_id BIGINT NOT NULL REFERENCES employees(id),
    kind        TEXT NOT NULL CHECK (kind IN ('annual', 'sick', 'unpaid', 'other')),
    start_date  DATE NOT NULL,
    end_date    DATE NOT NULL,
    note        TEXT,
    created_by  BIGINT REFERENCES users(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (end_date >= start_date)
);
CREATE INDEX absences_employee_idx ON absences (employee_id, start_date);
CREATE INDEX absences_range_idx ON absences (start_date, end_date);
