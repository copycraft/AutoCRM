-- Lead tags: the office's own buckets for enquiries, one list per market (the language
-- the customer is served in), like the per-language sales pipelines kept in MiniCRM. A
-- lead may carry several. A tag can name website domains; a lead that arrives from one of
-- them is tagged on arrival (domain::lead_tag decides what "from" means).
CREATE TABLE lead_tags (
    id          BIGSERIAL PRIMARY KEY,
    -- Two-letter market code: hu, ro, de, it... The screen names the known ones.
    market      TEXT NOT NULL CHECK (market ~ '^[a-z]{2}$'),
    label       TEXT NOT NULL CHECK (btrim(label) <> ''),
    color       TEXT NOT NULL DEFAULT '#64748b' CHECK (color ~ '^#[0-9a-f]{6}$'),
    -- Bare lower-case hosts without www. (hutoautok.hu); subdomains match too.
    domains     TEXT[] NOT NULL DEFAULT '{}',
    position    INT NOT NULL DEFAULT 0,
    archived_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX lead_tags_market_label_idx ON lead_tags (market, lower(label)) WHERE archived_at IS NULL;

CREATE TABLE lead_tag_links (
    lead_id        BIGINT NOT NULL REFERENCES leads(id),
    tag_id         BIGINT NOT NULL REFERENCES lead_tags(id),
    -- Set when the server tagged the lead itself: the domain it recognised.
    matched_domain TEXT,
    added_by       BIGINT REFERENCES users(id),
    added_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (lead_id, tag_id)
);
CREATE INDEX lead_tag_links_tag_idx ON lead_tag_links (tag_id);

-- The lists as they stood in MiniCRM. Rename, recolour and reorder freely in the app.
INSERT INTO lead_tags (market, label, color, domains, position) VALUES
    ('hu', 'Online ajánlatkérők',              '#dbe8ff', '{}', 10),
    ('hu', 'Black Friday',                     '#1f2228', '{}', 20),
    ('hu', 'Igényfelmérés',                    '#c3d9ff', '{}', 30),
    ('hu', 'Ajánlatkészítés',                  '#93bcff', '{}', 40),
    ('hu', 'Árajánlat utánkövetés',            '#5b93f5', '{}', 50),
    ('hu', 'Régi vevő (ÚJ AJÁNLAT)',           '#3d6fd6', '{}', 60),
    ('hu', 'Szerződéskötés (ÚJ VEVŐ)',         '#4f8a52', '{}', 70),
    ('hu', 'Szerződéskötés (RÉGI VEVŐ)',       '#1e3320', '{}', 80),
    ('hu', 'Vesztett ajánlatkérés',            '#5a1a12', '{}', 90),
    ('hu', 'Lezárt (EGYÉB OK MIATT)',          '#5b3a14', '{}', 100),
    ('hu', 'JEGELVE',                          '#d9862e', '{}', 110),
    ('hu', 'Folyamatban lévők',                '#a33122', '{}', 120),
    ('hu', 'Autókereskedésnek átadva',         '#6b7685', '{}', 130),
    ('hu', 'Pályázatos',                       '#e8988a', '{}', 140),

    ('ro', 'Online ajánlatkérők',              '#dbe8ff', '{}', 10),
    ('ro', 'Black Friday',                     '#1f2228', '{}', 20),
    ('ro', 'Igényfelmérés',                    '#c3d9ff', '{}', 30),
    ('ro', 'Ajánlatkészítés',                  '#93bcff', '{}', 40),
    ('ro', 'Árajánlat utánkövetés',            '#5b93f5', '{}', 50),
    ('ro', 'Régi vevő (ÚJ AJÁNLAT)',           '#3d6fd6', '{}', 60),
    ('ro', 'JEGELVE',                          '#1f3a75', '{}', 70),
    ('ro', 'Szerződéskötés (ÚJ VEVŐ)',         '#4f8a52', '{}', 80),
    ('ro', 'Szerződéskötés (RÉGI VEVŐ)',       '#1e3320', '{}', 90),
    ('ro', 'Vesztett ajánlatkérés',            '#5a1a12', '{}', 100),
    ('ro', 'Lezárt (EGYÉB OK MIATT)',          '#5b3a14', '{}', 110),

    ('de', 'Online ajánlatkérők',              '#dbe8ff', '{}', 10),
    ('de', 'Black Friday 2019',                '#1f2228', '{}', 20),
    ('de', 'Igényfelmérés',                    '#c3d9ff', '{}', 30),
    ('de', 'Ajánlatkészítés',                  '#93bcff', '{}', 40),
    ('de', 'Árajánlat utánkövetés',            '#5b93f5', '{}', 50),
    ('de', 'Régi vevő (ÚJ AJÁNLAT)',           '#3d6fd6', '{}', 60),
    ('de', 'JEGELVE',                          '#1f3a75', '{}', 70),
    ('de', 'Szerződéskötés (ÚJ VEVŐ)',         '#4f8a52', '{}', 80),
    ('de', 'Szerződéskötés (RÉGI VEVŐ)',       '#1e3320', '{}', 90),
    ('de', 'Vesztett ajánlatkérés',            '#5a1a12', '{}', 100),
    ('de', 'Lezárt (EGYÉB OK MIATT)',          '#5b3a14', '{}', 110),
    ('de', 'Viszonteladók',                    '#7e5219', '{}', 120),
    ('de', 'bestattungswagen.at',              '#c9402a', '{bestattungswagen.at}', 130),
    ('de', 'ÁTADVA partnernek',                '#2b313b', '{}', 140),

    ('it', 'furgonifunebri.it',                '#a33122', '{furgonifunebri.it}', 10),
    ('it', 'Online ajánlatkérők',              '#dbe8ff', '{}', 20),
    ('it', 'Igényfelmérés',                    '#c3d9ff', '{}', 30),
    ('it', 'Ajánlatkészítés',                  '#93bcff', '{}', 40),
    ('it', 'Árajánlat utánkövetés',            '#5b93f5', '{}', 50),
    ('it', 'Régi vevő (ÚJ AJÁNLAT)',           '#3d6fd6', '{}', 60),
    ('it', 'JEGELVE',                          '#d9862e', '{}', 70),
    ('it', 'Szerződéskötés (ÚJ VEVŐ)',         '#4f8a52', '{}', 80),
    ('it', 'Szerződéskötés (RÉGI VEVŐ)',       '#1e3320', '{}', 90),
    ('it', 'Lezárt (EGYÉB OK MIATT)',          '#5b3a14', '{}', 100),
    ('it', 'Viszonteladók',                    '#7e5219', '{}', 110);
