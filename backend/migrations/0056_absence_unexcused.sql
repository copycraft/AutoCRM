-- Unexcused absence (igazolatlan távollét): the employee did not turn up and there is no
-- paper for it. Kept apart from 'other' so the yearly summary can count it on its own.
ALTER TABLE absences DROP CONSTRAINT IF EXISTS absences_kind_check;
ALTER TABLE absences
    ADD CONSTRAINT absences_kind_check
        CHECK (kind IN ('annual', 'sick', 'unpaid', 'unexcused', 'other'));
