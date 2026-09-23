-- Intake slip (átvételi lap): the mileage and condition recorded when the vehicle is
-- taken in. Leaving `intake` requires the mileage (domain/stage.rs); the slip rides
-- along to MEO and onto the printed job sheet, so "that scratch was already there"
-- has an answer.
ALTER TABLE orders ADD COLUMN mileage_in INTEGER CHECK (mileage_in IS NULL OR mileage_in >= 0);
ALTER TABLE orders ADD COLUMN intake_condition TEXT;
