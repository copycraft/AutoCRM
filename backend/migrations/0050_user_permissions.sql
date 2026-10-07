-- Per-user grants on top of the role, like hr_access but for every other capability:
-- capability keys such as 'edit_orders' or 'send_email'. Admins have everything anyway.
ALTER TABLE users ADD COLUMN permissions TEXT[] NOT NULL DEFAULT '{}';
