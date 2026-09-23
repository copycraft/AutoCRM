-- Ready-for-pickup notification: queued when an order enters `completed`.
-- Same merge variables as order_stage_changed (order.number, order.vehicle,
-- order.stage, partner.name); the pickup email replaces the generic stage mail
-- for that move so the customer gets one letter, not two.
INSERT INTO email_templates (key, name, subject, body, is_automatic) VALUES
('order_ready_for_pickup', 'Elkészült értesítő (ügyfélnek)',
 '{{order.number}} elkészült – átvehető',
 'Tisztelt {{partner.name}}!

Örömmel értesítjük, hogy a(z) {{order.number}} számú projekt ({{order.vehicle}}) elkészült, az autó átvehető.

Üdvözlettel:
Autotherm', true)
ON CONFLICT (key) DO NOTHING;
