-- A zone is either required or optional, never both and never neither.
--
-- `required` and `optional` are two columns for one fact. The phone reads `optional` (a zone
-- it may skip; the sign-off checks every other one), and the seed from 0026 set the roof to
-- optional while leaving `required` at its default of true, so the pair could disagree and
-- nothing said which one meant what. Now the pair is forced to agree: what the office sets
-- in settings is what the phone enforces.
UPDATE inspection_zone_templates SET required = NOT optional WHERE required = optional;

ALTER TABLE inspection_zone_templates
    ADD CONSTRAINT inspection_zone_templates_required_xor_optional CHECK (required <> optional);
