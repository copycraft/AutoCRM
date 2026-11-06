-- Átvétel/átadás is a right of its own (`handle_handovers`): the walkaround protocols when
-- a vehicle comes in and goes back used to need `change_stages`, but whoever takes the car
-- in is not necessarily who plans the build. Everyone who could do them still can.
INSERT INTO role_permissions (role, capability)
SELECT role, 'handle_handovers' FROM role_permissions WHERE capability = 'change_stages'
ON CONFLICT DO NOTHING;

-- The two new roles' defaults, as in `Role::can`.
INSERT INTO role_permissions (role, capability) VALUES
    ('worker', 'upload_media'),
    ('worker', 'comment'),
    ('purchaser', 'handle_handovers'),
    ('purchaser', 'upload_media'),
    ('purchaser', 'comment'),
    ('purchaser', 'view_contacts'),
    ('purchaser', 'view_incoming_invoices')
ON CONFLICT DO NOTHING;
