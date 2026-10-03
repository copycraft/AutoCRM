-- The letter the office gets when the website files a new lead (POST /api/leads/website).
INSERT INTO email_templates (key, name, subject, body, is_automatic) VALUES
('website_lead_alert', 'Új weboldali érdeklődés (belső értesítő)',
 'Új weboldali érdeklődés: {{lead.title}}',
 'Új érdeklődés érkezett az autotherm.hu weboldalról.

Név: {{contact.name}}
E-mail: {{lead.contact_email}}
Telefon: {{lead.contact_phone}}
Forrás: {{lead.source}}

Üzenet:
{{lead.description}}

Megnyitás a CRM-ben: {{lead.url}}', true);
