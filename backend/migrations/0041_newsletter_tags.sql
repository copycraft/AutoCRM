-- Newsletter tags: the lists the office sorts subscribers into (bakeries, hauliers,
-- Austrian funeral homes...) and where they stand as buyers, grouped into sections the
-- way the MiniCRM newsletter sidebar was. A subscriber may carry several; a blast can go
-- to the subscribers of chosen tags instead of everyone.
CREATE TABLE newsletter_tags (
    id          BIGSERIAL PRIMARY KEY,
    -- The heading the tag is listed under: Listák, Értékesítés, Vevők, Nem vevők...
    section     TEXT NOT NULL CHECK (btrim(section) <> ''),
    label       TEXT NOT NULL CHECK (btrim(label) <> ''),
    color       TEXT NOT NULL DEFAULT '#dbe8ff' CHECK (color ~ '^#[0-9a-f]{6}$'),
    -- One order across all sections; a section is listed where its first tag is.
    position    INT NOT NULL DEFAULT 0,
    archived_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX newsletter_tags_label_idx
    ON newsletter_tags (lower(section), lower(label)) WHERE archived_at IS NULL;

-- Deleting a subscription takes its tags with it; a tag is archived, never deleted.
CREATE TABLE newsletter_subscription_tags (
    subscription_id BIGINT NOT NULL REFERENCES newsletter_subscriptions(id) ON DELETE CASCADE,
    tag_id          BIGINT NOT NULL REFERENCES newsletter_tags(id),
    added_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (subscription_id, tag_id)
);
CREATE INDEX newsletter_subscription_tags_tag_idx ON newsletter_subscription_tags (tag_id);

-- The lists as they stood in MiniCRM. Rename, recolour and reorder freely in the app.
INSERT INTO newsletter_tags (section, label, color, position) VALUES
    ('Listák', 'Csali feliratkozók',                 '#dbe8ff', 10),
    ('Listák', 'Autókereskedések',                   '#dbe8ff', 20),
    ('Listák', 'Autókereskedések (friss)',           '#dbe8ff', 30),
    ('Listák', 'Húsosok',                            '#dbe8ff', 40),
    ('Listák', 'Fuvarozók',                          '#dbe8ff', 50),
    ('Listák', 'Cukrászok',                          '#dbe8ff', 60),
    ('Listák', 'Cukrászdák',                         '#dbe8ff', 70),
    ('Listák', 'Gyógyszeresek',                      '#dbe8ff', 80),
    ('Listák', 'Virágosok',                          '#dbe8ff', 90),
    ('Listák', 'Élelmiszer Nagykerek',               '#dbe8ff', 100),
    ('Listák', 'Új magyar temetkezési',              '#dbe8ff', 110),
    ('Listák', 'Temetkezések',                       '#dbe8ff', 120),
    ('Listák', 'Mozgó Vendéglátósok',                '#dbe8ff', 130),
    ('Listák', 'Zöldségesek',                        '#dbe8ff', 140),
    ('Listák', 'Édesség Nagykerek',                  '#dbe8ff', 150),
    ('Listák', 'Pékségek',                           '#dbe8ff', 160),
    ('Listák', 'Tejesek',                            '#dbe8ff', 170),
    ('Listák', 'Próba küldések',                     '#dbe8ff', 180),
    ('Listák', 'Űrlapot kitöltők',                   '#dbe8ff', 190),
    ('Listák', 'Román - halottasok',                 '#dbe8ff', 200),
    ('Listák', 'Román - pékségek',                   '#dbe8ff', 210),
    ('Listák', 'Román - catering',                   '#dbe8ff', 220),
    ('Listák', 'Román - logisztikai',                '#dbe8ff', 230),
    ('Listák', 'Román - húsosok',                    '#dbe8ff', 240),
    ('Listák', 'Osztrák Alsó-Ausztria, Bécs',        '#dbe8ff', 250),
    ('Listák', 'Bécsi gyümölcskereskedők',           '#dbe8ff', 260),
    ('Listák', 'Osztrák temetkezési',                '#dbe8ff', 270),
    ('Listák', 'Német temetkezési',                  '#dbe8ff', 280),
    ('Listák', 'Svájci temetkezési',                 '#dbe8ff', 290),
    ('Listák', 'Szerbiai temetkezési',               '#dbe8ff', 300),

    ('Értékesítés', 'Black Friday ajánlatkérők',     '#dbe8ff', 1010),
    ('Értékesítés', 'Online ajánlatkérők',           '#dbe8ff', 1020),
    ('Értékesítés', 'Igényfelmérés',                 '#c3d9ff', 1030),
    ('Értékesítés', 'Ajánlatkészítés',               '#3d6fd6', 1040),
    ('Értékesítés', 'Árajánlat utánkövetés',         '#1f3a75', 1050),
    ('Értékesítés', 'Ajánlatkészítés Román',         '#a33122', 1060),
    ('Értékesítés', 'Árajánlat utánkövetés Román',   '#5a1a12', 1070),
    ('Értékesítés', 'Régi vevő (ÚJ AJÁNLAT)',        '#5aa05a', 1080),
    ('Értékesítés', 'Hűtős szerviz',                 '#1f2228', 1090),

    ('Vevők', 'Szerződéskötés',                      '#2e4a30', 2010),
    ('Vevők', 'Régi vevők',                          '#dbe8ff', 2020),

    ('Nem vevők', 'Vesztett ajánlatkérők',           '#dbe8ff', 3010),
    ('Nem vevők', 'Mástól vásárolt',                 '#1f2228', 3020),
    ('Nem vevők', 'Drága volt',                      '#2b313b', 3030),
    ('Nem vevők', 'Minőségi kifogás',                '#4b5563', 3040),
    ('Nem vevők', 'Bizalom hiánya',                  '#6b7685', 3050),
    ('Nem vevők', 'Nem érdekli',                     '#8f99a6', 3060),
    ('Nem vevők', 'Elérhetetlen',                    '#bcc3cc', 3070),
    ('Nem vevők', 'Később vásárol',                  '#dde1e6', 3080),
    ('Nem vevők', 'Csődbe ment',                     '#5b3a14', 3090),
    ('Nem vevők', 'Eltévedt-Elveszett-Nem létező',   '#7e5219', 3100),
    ('Nem vevők', 'Lezárt egyéb ok miatt',           '#d9862e', 3110),
    ('Nem vevők', 'Használt hűtőautót vásárolt',     '#a33122', 3120),
    ('Nem vevők', 'Román vesztett ajánlatkérők',     '#1e3320', 3130);
