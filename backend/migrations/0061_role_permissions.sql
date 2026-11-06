-- What each role may do, as an admin can change it: the grid behind Felhasználók →
-- Szerepkörök. Seeded with the defaults in `domain/role.rs` (`Role::can`); a test keeps the
-- two the same. Admins are not in it: they always have everything, so nobody can lock the
-- last admin out by unticking a box.
--
-- Seeing is a right of its own now (`view_*`): the shop floor gets the job, the vehicle and
-- its own department's part, not the price, the invoices or the customer's phone number.
CREATE TABLE role_permissions (
    role       user_role NOT NULL CHECK (role <> 'admin'),
    capability TEXT NOT NULL CHECK (capability ~ '^[a-z_]+$'),
    PRIMARY KEY (role, capability)
);

INSERT INTO role_permissions (role, capability) VALUES
    ('office', 'edit_orders'),
    ('office', 'delete_media'),
    ('office', 'view_original_images'),
    ('office', 'edit_leads'),
    ('office', 'edit_partners'),
    ('office', 'send_email'),
    ('office', 'issue_invoices'),
    ('office', 'change_stages'),
    ('office', 'manage_blockers'),
    ('office', 'upload_media'),
    ('office', 'comment'),
    ('office', 'view_prices'),
    ('office', 'view_contacts'),
    ('office', 'view_business_reports'),
    ('office', 'view_all_modules'),
    ('office', 'view_invoices'),
    ('office', 'view_incoming_invoices'),
    ('office', 'view_sales'),
    ('sales', 'edit_leads'),
    ('sales', 'edit_partners'),
    ('sales', 'send_email'),
    ('sales', 'upload_media'),
    ('sales', 'comment'),
    ('sales', 'view_prices'),
    ('sales', 'view_contacts'),
    ('sales', 'view_business_reports'),
    ('sales', 'view_all_modules'),
    ('sales', 'view_sales'),
    ('accountant', 'edit_partners'),
    ('accountant', 'send_email'),
    ('accountant', 'issue_invoices'),
    ('accountant', 'comment'),
    ('accountant', 'view_prices'),
    ('accountant', 'view_contacts'),
    ('accountant', 'view_business_reports'),
    ('accountant', 'view_all_modules'),
    ('accountant', 'view_invoices'),
    ('accountant', 'view_incoming_invoices'),
    ('designer', 'change_stages'),
    ('designer', 'manage_blockers'),
    ('designer', 'upload_media'),
    ('designer', 'comment'),
    ('viewer', 'view_prices'),
    ('viewer', 'view_contacts'),
    ('viewer', 'view_business_reports'),
    ('viewer', 'view_all_modules'),
    ('viewer', 'view_invoices'),
    ('viewer', 'view_incoming_invoices'),
    ('viewer', 'view_sales');
