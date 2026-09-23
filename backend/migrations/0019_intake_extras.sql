-- Intake slip extras: fuel level, key count and valuables noted at takeover.
-- Optional (the gate only requires the mileage); printed on the job sheet next
-- to the mileage, because disputes are about fuel and keys as often as scratches.
ALTER TABLE orders ADD COLUMN fuel_level TEXT
    CHECK (fuel_level IS NULL OR fuel_level IN ('E', '1/4', '1/2', '3/4', 'F'));
ALTER TABLE orders ADD COLUMN key_count INTEGER
    CHECK (key_count IS NULL OR key_count >= 0);

-- Valuables are three-state on purpose, because "nobody asked" and "asked, nothing
-- in the car" are different defences and a single TEXT column cannot tell them apart:
--   NULL  — not recorded
--   false — recorded, nothing left in the vehicle
--   true  — recorded, and `valuables` says what
ALTER TABLE orders ADD COLUMN valuables_declared BOOLEAN;
ALTER TABLE orders ADD COLUMN valuables TEXT
    CHECK (valuables IS NULL OR valuables_declared);
