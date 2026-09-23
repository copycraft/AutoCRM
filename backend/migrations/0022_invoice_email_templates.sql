-- The four letters invoicing sends. Editable afterwards in the settings screen, like every
-- other template; the wording here is the starting point, not a constant in the code.
--
-- The proforma letter is deliberately worded unlike the other three. A díjbekérő is a
-- request for payment, not a tax document, and saying so on the letter is the difference
-- between a customer who pays it and an accountant who tries to book it.
INSERT INTO email_templates (key, name, subject, body, is_automatic) VALUES
('invoice_issued', 'Számla kiállítva (ügyfélnek)',
 'Számla: {{invoice.number}} ({{order.number}})',
 'Tisztelt {{partner.name}}!

Mellékelten küldjük a(z) {{order.number}} számú projekthez ({{order.vehicle}}) tartozó számlánkat.

Számla sorszáma: {{invoice.number}}
Kiállítás kelte: {{invoice.issue_date}}
Fizetési határidő: {{invoice.payment_date}}
Végösszeg: {{invoice.total}}

A számla adatait a NAV Online Számla rendszerébe továbbítottuk.

Üdvözlettel:
Autotherm', true),

('invoice_stornoed', 'Sztornó számla (ügyfélnek)',
 'Sztornó számla: {{invoice.number}}',
 'Tisztelt {{partner.name}}!

A(z) {{invoice.original_number}} sorszámú számlánkat sztornóztuk. A sztornó számlát mellékeljük.

Sztornó számla sorszáma: {{invoice.number}}
Sztornózott számla sorszáma: {{invoice.original_number}}
Kiállítás kelte: {{invoice.issue_date}}
Végösszeg: {{invoice.total}}

Kérjük, a(z) {{invoice.original_number}} sorszámú számlát tekintse érvénytelennek. A sztornózást a NAV Online Számla rendszerében is jelentettük.

Üdvözlettel:
Autotherm', true),

('invoice_annulled', 'Számla technikai érvénytelenítése (ügyfélnek)',
 'Számla érvénytelenítése: {{invoice.number}}',
 'Tisztelt {{partner.name}}!

Tájékoztatjuk, hogy a(z) {{invoice.number}} sorszámú számlánk adatszolgáltatását technikai érvénytelenítésre jelentettük be a NAV Online Számla rendszerében, mert az hibásan került rögzítésre.

A helyes bizonylatot külön megküldjük. Kérjük, addig a(z) {{invoice.number}} sorszámú számlát ne könyvelje.

Elnézést kérünk a kellemetlenségért.

Üdvözlettel:
Autotherm', true),

('proforma_created', 'Díjbekérő (ügyfélnek)',
 'Díjbekérő: {{proforma.number}} ({{order.number}})',
 'Tisztelt {{partner.name}}!

Mellékelten küldjük a(z) {{order.number}} számú projekthez ({{order.vehicle}}) tartozó díjbekérőnket.

Díjbekérő sorszáma: {{proforma.number}}
Fizetendő összeg: {{proforma.total}}
Fizetési határidő: {{proforma.payment_date}}

Felhívjuk szíves figyelmét, hogy a díjbekérő fizetési felszólítás, nem adóügyi bizonylat: áfa levonására nem jogosít, és a NAV felé nem kerül bejelentésre. A számlát a fizetés beérkezése után állítjuk ki és küldjük meg.

Üdvözlettel:
Autotherm', true)
ON CONFLICT (key) DO NOTHING;
