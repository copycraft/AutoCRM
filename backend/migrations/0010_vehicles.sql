-- The vehicle becomes an entity (V2.1).
--
-- Before this, a vehicle was four nullable text columns on an order. That breaks the
-- questions this business actually asks: "has this van been here before", "which order do
-- these MEO photos belong to when one job covers three Sprinters", "which vehicle does this
-- ATP certificate cover". Adding it after 1000 orders are migrated would mean re-parsing
-- raw_import per account-specific custom field, reconciling against free text staff typed
-- in the meantime, and a manual photo-by-photo reclassification pass on orders that already
-- hold 80–120 images. Adding it now costs one migration and a mapping field.

CREATE TABLE vehicles (
    id         BIGSERIAL PRIMARY KEY,
    vin        TEXT,
    plate      TEXT,
    -- Same rule as domain/order.rs::normalize_plate and the orders_plate_idx expression:
    -- "abc-123", "ABC 123" and "ABC123" are one vehicle.
    plate_norm TEXT GENERATED ALWAYS AS (
        upper(regexp_replace(coalesce(plate, ''), '[^A-Za-z0-9]', '', 'g'))
    ) STORED,
    make       TEXT,
    model      TEXT,
    year       INT CHECK (year IS NULL OR (year BETWEEN 1900 AND 2200)),
    -- The vehicle's usual owner. Nullable: a van can change hands, and the order's partner
    -- remains the customer of record for that job.
    partner_id BIGINT REFERENCES partners(id),
    notes      TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (vin IS NOT NULL OR plate IS NOT NULL)
);
CREATE INDEX vehicles_plate_norm_idx ON vehicles (plate_norm) WHERE plate IS NOT NULL;
CREATE INDEX vehicles_vin_idx ON vehicles (upper(vin)) WHERE vin IS NOT NULL;
CREATE INDEX vehicles_partner_idx ON vehicles (partner_id);
CREATE TRIGGER vehicles_touch BEFORE UPDATE ON vehicles FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

-- Many-to-many, because one order can cover several identical vans (scenario 6) and one
-- van comes back for warranty work years later.
CREATE TABLE order_vehicles (
    order_id   BIGINT NOT NULL REFERENCES orders(id),
    vehicle_id BIGINT NOT NULL REFERENCES vehicles(id),
    PRIMARY KEY (order_id, vehicle_id)
);
CREATE INDEX order_vehicles_vehicle_idx ON order_vehicles (vehicle_id);

-- Which van a photo or a certificate is of, on a multi-vehicle order. Nullable: most
-- orders have one vehicle and the link is redundant there.
ALTER TABLE images    ADD COLUMN vehicle_id BIGINT REFERENCES vehicles(id);
ALTER TABLE documents ADD COLUMN vehicle_id BIGINT REFERENCES vehicles(id);
CREATE INDEX images_vehicle_idx    ON images (vehicle_id)    WHERE vehicle_id IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX documents_vehicle_idx ON documents (vehicle_id) WHERE vehicle_id IS NOT NULL AND deleted_at IS NULL;

-- The four text columns stay, and the migration writes both, because deduplicating plates
-- typed by hand over thirty years may turn out wrong and this is the fallback.
COMMENT ON COLUMN orders.vehicle_make  IS 'DEPRECATED (V2.1): use order_vehicles → vehicles.make. Kept as the migration fallback.';
COMMENT ON COLUMN orders.vehicle_model IS 'DEPRECATED (V2.1): use order_vehicles → vehicles.model. Kept as the migration fallback.';
COMMENT ON COLUMN orders.vehicle_plate IS 'DEPRECATED (V2.1): use order_vehicles → vehicles.plate. Kept as the migration fallback.';
COMMENT ON COLUMN orders.vehicle_vin   IS 'DEPRECATED (V2.1): use order_vehicles → vehicles.vin. Kept as the migration fallback.';
