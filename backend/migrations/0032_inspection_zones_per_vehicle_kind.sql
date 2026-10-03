-- Walkaround zone lists per vehicle kind and per walkaround kind.
--
-- Until now one list (`default`, plus a `cooling` extra for refrigerated project types)
-- served every vehicle and both walkarounds. A bare chassis cab arriving for a box needs
-- different photos from the finished vehicle leaving, and a chassis-with-box needs
-- different ones from a converted van. A list is now addressed by
-- (project type, walkaround kind):
--
--   project_type_id NULL  the general list, used by any project type without its own
--   kind 'checkout'       the first walkaround, the vehicle arriving (átvétel / intake)
--   kind 'checkin'        the second, the vehicle leaving (kiadás / outgo), compared
--                         against the first for new damage
--
-- The kind names stay as they were: they are API values and stored on every inspection.
-- Only the labels changed, on both clients.
--
-- Lookup is the project type's own list for the kind, else the general list for it.
-- A list is whole: nothing is merged any more, so what the office sees in settings is
-- exactly what the phone walks.

ALTER TABLE inspection_zone_templates
    DROP CONSTRAINT inspection_zone_templates_set_key_zone_key_key,
    DROP CONSTRAINT inspection_zone_templates_set_key_check,
    ALTER COLUMN set_key DROP NOT NULL,
    ALTER COLUMN set_key DROP DEFAULT,
    ADD COLUMN project_type_id BIGINT REFERENCES project_types(id) ON DELETE CASCADE,
    ADD COLUMN kind TEXT CHECK (kind IN ('checkout', 'checkin')),
    -- The short name the phone shows as the zone's heading; `instruction` is the sentence
    -- under it. Free text, so lists the office builds need no change in the apps.
    ADD COLUMN title TEXT;

-- Titles for the rows that exist: the names the phone used to hard-code per zone key.
UPDATE inspection_zone_templates SET title = CASE zone_key
    WHEN 'front' THEN 'Elöl'
    WHEN 'front_left' THEN 'Bal első sarok'
    WHEN 'left_side' THEN 'Bal oldal'
    WHEN 'rear_left' THEN 'Bal hátsó sarok'
    WHEN 'rear' THEN 'Hátul'
    WHEN 'rear_right' THEN 'Jobb hátsó sarok'
    WHEN 'right_side' THEN 'Jobb oldal'
    WHEN 'front_right' THEN 'Jobb első sarok'
    WHEN 'roof' THEN 'Tető'
    WHEN 'wheels' THEN 'Kerekek, gumik'
    WHEN 'glass' THEN 'Szélvédő, üvegek'
    WHEN 'interior_front' THEN 'Belső: első ülések'
    WHEN 'interior_rear' THEN 'Belső: hátsó ülések'
    WHEN 'interior_dashboard' THEN 'Belső: műszerfal'
    WHEN 'interior_boot' THEN 'Belső: csomagtartó'
    WHEN 'cargo_box' THEN 'Rakodótér'
    WHEN 'cargo_doors' THEN 'Rakodótér ajtók'
    WHEN 'refrigeration_unit' THEN 'Hűtőaggregát'
    ELSE left(instruction, 100)
END;

-- Project types that used to get `default` + `cooling` keep exactly that list, now as
-- their own, for both walkarounds. Alváz with box ("Hűtőfelépítmény") is left out: it
-- gets its own lists below. A zone the office had put in both sets counts once.
INSERT INTO inspection_zone_templates
    (project_type_id, kind, zone_key, position, title, instruction, optional, required)
SELECT pt.id, k.kind, z.zone_key, z.position, z.title, z.instruction, z.optional, z.required
  FROM project_types pt
 CROSS JOIN (VALUES ('checkout'), ('checkin')) AS k(kind)
 CROSS JOIN LATERAL (
        SELECT DISTINCT ON (zone_key) zone_key, position, title, instruction, optional, required
          FROM inspection_zone_templates
         WHERE set_key IN ('default', 'cooling')
         ORDER BY zone_key, (set_key = 'default') DESC
      ) z
 WHERE pt.spec_form = 'cooling' AND pt.key <> 'refrigerated_body';

-- The general list becomes the general intake list, and is copied as the general outgo
-- list: until the office says otherwise, both walkarounds ask for the same photos.
INSERT INTO inspection_zone_templates
    (project_type_id, kind, zone_key, position, title, instruction, optional, required)
SELECT NULL, 'checkin', zone_key, position, title, instruction, optional, required
  FROM inspection_zone_templates
 WHERE set_key = 'default';
UPDATE inspection_zone_templates SET kind = 'checkout' WHERE set_key = 'default';
DELETE FROM inspection_zone_templates WHERE set_key = 'cooling';

ALTER TABLE inspection_zone_templates
    DROP COLUMN set_key,
    ALTER COLUMN kind SET NOT NULL,
    ALTER COLUMN title SET NOT NULL,
    ADD CONSTRAINT inspection_zone_templates_title_check
        CHECK (char_length(title) BETWEEN 1 AND 100);

CREATE UNIQUE INDEX inspection_zone_templates_general
    ON inspection_zone_templates (kind, zone_key) WHERE project_type_id IS NULL;
CREATE UNIQUE INDEX inspection_zone_templates_per_type
    ON inspection_zone_templates (project_type_id, kind, zone_key) WHERE project_type_id IS NOT NULL;

-- Alváz with a box ("Hűtőfelépítmény"): generated from the office's example set of 38
-- photos of a finished vehicle. Intake = the shots that exist on the bare chassis cab;
-- outgo = every photo of the example, in the order it was taken, plus an optional
-- odometer shot. Every intake zone key is also an outgo key, so a damage seen at outgo
-- is compared with the same zone at intake.
INSERT INTO inspection_zone_templates
    (project_type_id, kind, zone_key, position, title, instruction, optional, required)
SELECT pt.id, v.kind, v.zone_key, v.position, v.title, v.instruction, v.optional, NOT v.optional
  FROM project_types pt
 CROSS JOIN (VALUES
    ('checkout', 'type_plate', 1, 'Gyári adattábla', 'Gyári adattábla (típustábla), jól olvashatóan', false),
    ('checkout', 'vin_windshield', 2, 'Alvázszám (VIN)', 'Az alvázszám a szélvédő alatt, olvashatóan', false),
    ('checkout', 'engine_bay', 3, 'Motortér', 'Motortér, a motorháztető nyitva', false),
    ('checkout', 'interior_dashboard', 4, 'Műszerfal, km', 'Műszerfal és kilométeróra-állás', true),
    ('checkout', 'cab_driver_side', 5, 'Fülke: kormány, műszerfal', 'Fülke belülről, az utasoldali ajtóból: ülések, kormány, műszerfal', false),
    ('checkout', 'door_left_inner', 6, 'Bal ajtó belül', 'Bal oldali ajtó belső burkolata, az ajtó nyitva', false),
    ('checkout', 'cab_passenger_side', 7, 'Fülke: utasülés, konzol', 'Fülke belülről: utasülés, középkonzol, kesztyűtartó', false),
    ('checkout', 'door_right_inner', 8, 'Jobb ajtó belül', 'Jobb oldali ajtó belső burkolata, az ajtó nyitva', false),
    ('checkout', 'front_left', 9, 'Bal első sarok', 'Elölről-balról: a fülke és az alváz egészben', false),
    ('checkout', 'headlight_bumper', 10, 'Fényszóró, lökhárító', 'Első fényszóró és lökhárító közelről', false),
    ('checkout', 'mirror_left', 11, 'Bal tükör', 'Bal külső tükör közelről', false),
    ('checkout', 'left_side', 12, 'Bal oldal', 'Bal oldal: a fülke és az alváz egészben', false),
    ('checkout', 'wheel_rear_1', 13, 'Hátsó kerék (1)', 'Hátsó kerék, gumi és felni közelről (1. oldal)', false),
    ('checkout', 'chassis_side', 14, 'Alváz oldala', 'Az alváz oldala hosszában', false),
    ('checkout', 'rear_lights_1', 15, 'Hátsó lámpák (1)', 'Hátsó lámpa és helyzetjelző közelről (1. oldal)', false),
    ('checkout', 'rear', 16, 'Hátul', 'Hátulról, szemből: az alváz vége és a hátsó lámpák', false),
    ('checkout', 'rear_lights_2', 17, 'Hátsó lámpák (2)', 'Hátsó lámpa és helyzetjelző közelről (2. oldal)', false),
    ('checkout', 'wheel_rear_2', 18, 'Hátsó kerék (2)', 'Hátsó kerék, gumi és felni közelről (2. oldal)', false),
    ('checkout', 'mirror_right', 19, 'Jobb tükör', 'Jobb külső tükör az irányjelzővel, közelről', false),
    ('checkout', 'front_right', 20, 'Jobb első sarok', 'Elölről-jobbról: a fülke és az alváz egészben', false),
    ('checkin', 'type_plate', 1, 'Gyári adattábla', 'Gyári adattábla (típustábla), jól olvashatóan', false),
    ('checkin', 'engine_bay', 2, 'Motortér', 'Motortér, a motorháztető nyitva', false),
    ('checkin', 'cab_reefer_display', 3, 'Hűtő kijelzője a fülkében', 'A hűtőgép vezérlő-kijelzője a fülkében', false),
    ('checkin', 'interior_dashboard', 4, 'Műszerfal, km', 'Műszerfal és kilométeróra-állás', true),
    ('checkin', 'cab_driver_side', 5, 'Fülke: kormány, műszerfal', 'Fülke belülről, az utasoldali ajtóból: ülések, kormány, műszerfal', false),
    ('checkin', 'door_right_inner', 6, 'Jobb ajtó belül', 'Jobb oldali ajtó belső burkolata, az ajtó nyitva', false),
    ('checkin', 'cab_passenger_side', 7, 'Fülke: utasülés, konzol', 'Fülke belülről: utasülés, középkonzol, kesztyűtartó', false),
    ('checkin', 'door_left_inner', 8, 'Bal ajtó belül', 'Bal oldali ajtó belső burkolata, az ajtó nyitva', false),
    ('checkin', 'box_side_door', 9, 'Doboz oldalajtaja', 'A doboz oldalajtaja nyitva, kívülről', false),
    ('checkin', 'box_side_threshold', 10, 'Oldalajtó küszöbe', 'Az oldalajtó küszöbe és padlóéle', false),
    ('checkin', 'box_interior_side', 11, 'Raktér (oldalajtónál)', 'Raktér belülről, az oldalajtó felől', false),
    ('checkin', 'box_interior_rear', 12, 'Raktér (hátulról)', 'Raktér hátulról nézve: küszöb, hátsó lámpák, rendszámtábla', false),
    ('checkin', 'reefer_unit_inside', 13, 'Belső hűtőegység', 'A belső hűtőegység (elpárologtató) közelről, alulról', false),
    ('checkin', 'box_interior_full', 14, 'Raktér teljes hossza', 'Raktér a hátsó ajtónyílásból: az aggregát és a padló is látsszon', false),
    ('checkin', 'wheelhouse_1', 15, 'Kerékdob-burkolat (1)', 'Kerékdob-burkolat a raktérben, közelről (1. oldal)', false),
    ('checkin', 'wheelhouse_2', 16, 'Kerékdob-burkolat (2)', 'Kerékdob-burkolat a raktérben, közelről (2. oldal)', false),
    ('checkin', 'rear_doors_open', 17, 'Hátsó ajtók nyitva', 'Mindkét hátsó ajtószárny nyitva, hátulról', false),
    ('checkin', 'rear_door_1', 18, 'Hátsó ajtószárny (1)', 'Hátsó ajtószárny nyitva: belső oldal és tömítés', false),
    ('checkin', 'rear_door_2', 19, 'Hátsó ajtószárny (2)', 'A másik hátsó ajtószárny nyitva: belső oldal és tömítés', false),
    ('checkin', 'rear_hinge', 20, 'Ajtózsanér', 'Hátsó ajtózsanér és tetőprofil közelről', false),
    ('checkin', 'roof_corner_1', 21, 'Tetősarok (1)', 'A doboz tetősarka a helyzetjelzővel (1)', false),
    ('checkin', 'roof_corner_2', 22, 'Tetősarok (2)', 'A doboz tetősarka a helyzetjelzővel (2)', false),
    ('checkin', 'reefer_unit_roof', 23, 'Tetőaggregát', 'A tetőre szerelt hűtőaggregát', false),
    ('checkin', 'front_right', 24, 'Jobb első sarok', 'Elölről-jobbról: a fülke, a doboz és az aggregát a képben', false),
    ('checkin', 'mirror_right', 25, 'Jobb tükör', 'Jobb külső tükör az irányjelzővel, közelről', false),
    ('checkin', 'shore_power', 26, '230 V csatlakozó', 'A külső 230 V-os csatlakozó (dugalj)', false),
    ('checkin', 'wheel_rear_1', 27, 'Hátsó kerék (1)', 'Hátsó kerék és kerékív-burkolat', false),
    ('checkin', 'rear_lights_1', 28, 'Hátsó lámpák (1)', 'Hátsó lámpa, helyzetjelző és ütközésvédő sarok', false),
    ('checkin', 'rear_right', 29, 'Jobb hátsó sarok', 'Hátulról-jobbról: az ajtók és az oldalfal', false),
    ('checkin', 'rear', 30, 'Hátul', 'Hátulról, szemből: a zárt ajtók és a rendszámtábla', false),
    ('checkin', 'rear_left', 31, 'Bal hátsó sarok', 'Hátulról-balról: az ajtók és az oldalfal', false),
    ('checkin', 'rear_lights_2', 32, 'Hátsó lámpák (2)', 'A másik hátsó sarok: lámpák és helyzetjelző közelről', false),
    ('checkin', 'wheel_rear_2', 33, 'Hátsó kerék (2)', 'A másik hátsó kerék: gumi és felni közelről', false),
    ('checkin', 'chassis_side', 34, 'Alváz oldala', 'Az alváz oldala hosszában, a dobozzal együtt', false),
    ('checkin', 'left_side', 35, 'Bal oldal', 'Bal oldal: a fülke és a doboz egészben', false),
    ('checkin', 'mirror_left', 36, 'Bal tükör', 'Bal külső tükör közelről', false),
    ('checkin', 'headlight_bumper', 37, 'Fényszóró, lökhárító', 'Első fényszóró és lökhárító közelről', false),
    ('checkin', 'front_left', 38, 'Bal első sarok', 'Elölről-balról: az egész autó a képben', false),
    ('checkin', 'vin_windshield', 39, 'Alvázszám (VIN)', 'Az alvázszám a szélvédő alatt, olvashatóan', false)
 ) AS v(kind, zone_key, position, title, instruction, optional)
 WHERE pt.key = 'refrigerated_body';
