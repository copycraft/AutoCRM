-- Templates get a place, the way the MiniCRM "Sablonok" page sorted them: an area (the tabs:
-- Értékesítés, Projektek, Számlázó...), a folder (letters to customers, letters for the
-- office's own workflow, design samples to copy), and a bin. The key stays what code uses.
ALTER TABLE email_templates
    ADD COLUMN category TEXT NOT NULL DEFAULT 'general'
        CHECK (category IN ('sales', 'projects', 'billing', 'marketing', 'hr', 'general')),
    ADD COLUMN folder TEXT NOT NULL DEFAULT 'customer'
        CHECK (folder IN ('customer', 'workflow', 'design')),
    ADD COLUMN archived_at TIMESTAMPTZ;

UPDATE email_templates SET category = CASE
    WHEN key LIKE 'quote%' OR key LIKE 'lead%' OR key = 'website_lead_alert' THEN 'sales'
    WHEN key LIKE 'invoice%' OR key LIKE 'proforma%' THEN 'billing'
    WHEN key LIKE 'newsletter%' THEN 'marketing'
    WHEN key LIKE 'order%' OR key LIKE 'blocker%' OR key LIKE 'stalled%' THEN 'projects'
    ELSE 'general' END;
UPDATE email_templates SET folder = 'workflow' WHERE key IN ('website_lead_alert', 'stalled_order_alert');

-- The billing letters the office kept in MiniCRM, as starting points to edit. Nothing sends
-- them on its own: they are picked in the composer, about an order.
INSERT INTO email_templates (key, name, subject, body, category, folder) VALUES
('invoice_overdue_1', 'Számla fizetési határidő lejárt #1', 'Fizetési emlékeztető: {{invoice.number}}',
 'Tisztelt {{partner.name}}!

Szeretnénk emlékeztetni, hogy a(z) {{invoice.number}} számú, {{invoice.total}} összegű számlánk fizetési határideje ({{invoice.payment_date}}) lejárt.

Ha időközben rendezte, kérjük, tekintse levelünket tárgytalannak.

Üdvözlettel:
{{user.name}}
Autotherm', 'billing', 'customer'),
('invoice_overdue_2', 'Számla fizetési határidő lejárt #2', 'Ismételt fizetési felszólítás: {{invoice.number}}',
 'Tisztelt {{partner.name}}!

A(z) {{invoice.number}} számú, {{invoice.total}} összegű számlánk ellenértéke korábbi emlékeztetőnk ellenére sem érkezett meg (fizetési határidő: {{invoice.payment_date}}).

Kérjük, az összeget 8 napon belül szíveskedjen átutalni, vagy jelezze, ha a számlával kapcsolatban kérdése van.

Üdvözlettel:
{{user.name}}
Autotherm', 'billing', 'customer'),
('proforma_paid', 'Díjbekérő befizetve', 'Köszönjük a befizetést: {{proforma.number}}',
 'Tisztelt {{partner.name}}!

Köszönjük, a(z) {{proforma.number}} számú díjbekérőnk összege ({{proforma.total}}) megérkezett. A végszámlát hamarosan küldjük.

Üdvözlettel:
{{user.name}}
Autotherm', 'billing', 'customer'),
('invoice_paid', 'Számla befizetve', 'Köszönjük a befizetést: {{invoice.number}}',
 'Tisztelt {{partner.name}}!

Köszönjük, a(z) {{invoice.number}} számú számlánk összege ({{invoice.total}}) megérkezett.

Üdvözlettel:
{{user.name}}
Autotherm', 'billing', 'customer'),
('bank_account_changed', 'Bankszámlaszám megváltozott', 'Megváltozott bankszámlaszámunk',
 'Tisztelt {{partner.name}}!

Tájékoztatjuk, hogy bankszámlaszámunk megváltozott. Kérjük, a jövőben az új számlaszámra utaljon:

[új bankszámlaszám]

A változásról minden új számlán is tájékoztatjuk. Kérdés esetén keressen bennünket.

Üdvözlettel:
{{user.name}}
Autotherm', 'billing', 'customer'),
('payment_reminder_de', 'Zahlungserinnerung', 'Zahlungserinnerung: Rechnung {{invoice.number}}',
 'Sehr geehrte Damen und Herren,

wir möchten Sie freundlich daran erinnern, dass die Rechnung {{invoice.number}} über {{invoice.total}} am {{invoice.payment_date}} fällig war.

Sollten Sie die Zahlung bereits veranlasst haben, betrachten Sie dieses Schreiben bitte als gegenstandslos.

Mit freundlichen Grüßen
{{user.name}}
Autotherm', 'billing', 'customer'),
('e_invoice_issued_de', 'E-Rechnung ausgestellt', 'E-Rechnung {{invoice.number}}',
 'Sehr geehrte Damen und Herren,

anbei erhalten Sie unsere elektronische Rechnung {{invoice.number}} über {{invoice.total}}, zahlbar bis {{invoice.payment_date}}.

Mit freundlichen Grüßen
{{user.name}}
Autotherm', 'billing', 'customer'),
('e_invoice_issued_en', 'E-invoice exhibited', 'E-invoice {{invoice.number}}',
 'Dear Customer,

Please find attached our electronic invoice {{invoice.number}} for {{invoice.total}}, payable by {{invoice.payment_date}}.

Kind regards,
{{user.name}}
Autotherm', 'billing', 'customer'),
('staff_customer_paid', 'Munkatársaknak: ügyfél fizetett', 'Fizetett: {{partner.name}} – {{invoice.number}}',
 'A(z) {{partner.name}} kiegyenlítette a(z) {{invoice.number}} számú számlát ({{invoice.total}}).

Megrendelés: {{order.number}} – {{order.title}}', 'billing', 'workflow');
