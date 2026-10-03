//! Client-facing enumerations, owned here so every client reads them from one place.
//!
//! The rule: anything the office might ask to rename, extend or drop lives on the
//! server and is published by `GET /config/lookups`. The phone and the web render
//! what that endpoint returns and cache it for offline use; nothing in this file
//! is duplicated in a client. Adding a damage type, a fuel level, a payment
//! method or a currency is a backend change plus a server deploy — no app update.
//!
//! Validation uses the same constants (`in_list` against `*_KEYS`), so what the
//! endpoint publishes and what the API accepts cannot drift apart: a value the
//! endpoint does not list is refused, and every listed value is accepted.

use serde::Serialize;
use utoipa::ToSchema;

use super::invoice::PaymentMethod;
use super::media::ImageCategory;
use super::money::Currency;

/// One selectable value with its Hungarian label.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LookupItem {
    pub key: String,
    pub label_hu: String,
}

fn item(key: &str, label_hu: &str) -> LookupItem {
    LookupItem {
        key: key.to_string(),
        label_hu: label_hu.to_string(),
    }
}

// ── Handover damage record ────────────────────────────────────────────────────

/// Accepted by `POST /inspections/{id}/damages`; the phone's damage form and the
/// web history both render `damage_types()`.
pub const DAMAGE_TYPE_KEYS: &[&str] = &[
    "scratch", "dent", "crack", "chip", "broken", "missing", "stain", "tear", "other",
];

pub fn damage_types() -> Vec<LookupItem> {
    [
        ("scratch", "Karcolás"),
        ("dent", "Horpadás"),
        ("crack", "Repedés"),
        ("chip", "Lepattanás"),
        ("broken", "Törött alkatrész"),
        ("missing", "Hiányzó alkatrész"),
        ("stain", "Folt"),
        ("tear", "Szakadás"),
        ("other", "Egyéb"),
    ]
    .into_iter()
    .map(|(key, label_hu)| item(key, label_hu))
    .collect()
}

/// Accepted alongside the damage type above.
pub const SEVERITY_KEYS: &[&str] = &["minor", "moderate", "severe"];

pub fn severities() -> Vec<LookupItem> {
    [
        ("minor", "Enyhe"),
        ("moderate", "Közepes"),
        ("severe", "Súlyos"),
    ]
    .into_iter()
    .map(|(key, label_hu)| item(key, label_hu))
    .collect()
}

/// Accepted by `POST /inspections/{id}/verdicts`; the verdict buttons on both
/// clients render `verdicts()`.
pub const VERDICT_KEYS: &[&str] = &["preexisting", "new", "dismissed"];

pub fn verdicts() -> Vec<LookupItem> {
    [
        ("preexisting", "Már megvolt"),
        ("new", "Új sérülés"),
        ("dismissed", "Nem sérülés"),
    ]
    .into_iter()
    .map(|(key, label_hu)| item(key, label_hu))
    .collect()
}

// ── Handover walkarounds ──────────────────────────────────────────────────────

/// `checkout` is the first walkaround, the vehicle arriving (átvétel);
/// `checkin` is the second, the vehicle leaving (kiadás). The keys are API
/// values and stay; the headings render from here on both clients.
pub const WALKAROUND_KEYS: &[&str] = &["checkout", "checkin"];

pub fn walkaround_kinds() -> Vec<LookupItem> {
    [("checkout", "Átvétel"), ("checkin", "Kiadás")]
        .into_iter()
        .map(|(key, label_hu)| item(key, label_hu))
        .collect()
}

// ── Fuel, tasks, money, invoicing ─────────────────────────────────────────────

/// The intake-slip and walkaround fuel chips, in gauge order. The order slip is
/// constrained by the CHECK in 0019_intake_extras; the chips on every client
/// come from here, so a new mark is one backend change, not three client ones.
pub const FUEL_LEVEL_KEYS: &[&str] = &["E", "1/4", "1/2", "3/4", "F"];

pub fn fuel_levels() -> Vec<LookupItem> {
    FUEL_LEVEL_KEYS.iter().map(|key| item(key, key)).collect()
}

/// How an order relates to the job it repairs or repeats. Validated in
/// `service::orders`; the badge on the order page comes from here.
pub const ORDER_RELATION_KEYS: &[&str] = &["warranty", "rework", "repeat"];

pub fn order_relations() -> Vec<LookupItem> {
    [
        ("warranty", "Garanciális"),
        ("rework", "Újramunkálás"),
        ("repeat", "Ismételt"),
    ]
    .into_iter()
    .map(|(key, label_hu)| item(key, label_hu))
    .collect()
}

/// The cooling build-spec defrost mode. Validated in `api::orders`; the select
/// on the order form comes from here.
pub const DEFROST_KEYS: &[&str] = &["automatic", "manual", "hot_gas"];

pub fn defrost_modes() -> Vec<LookupItem> {
    [
        ("automatic", "Automatikus"),
        ("manual", "Kézi"),
        ("hot_gas", "Forrógázas"),
    ]
    .into_iter()
    .map(|(key, label_hu)| item(key, label_hu))
    .collect()
}

/// The heating build-spec fuel. Validated in `api::orders` and by the
/// `order_specs_fuel_check` CHECK; the select on the order form comes from here.
pub const HEATING_FUEL_KEYS: &[&str] = &["diesel", "electric", "lpg", "engine_coolant"];

pub fn heating_fuels() -> Vec<LookupItem> {
    [
        ("diesel", "Dízel"),
        ("electric", "Elektromos"),
        ("lpg", "LPG"),
        ("engine_coolant", "Motorhűtőfolyadék"),
    ]
    .into_iter()
    .map(|(key, label_hu)| item(key, label_hu))
    .collect()
}

/// Records a task may be pinned to; `POST /tasks` refuses anything else.
pub const TASK_ENTITY_KEYS: &[&str] = &["order", "lead", "partner"];

pub fn task_entity_types() -> Vec<LookupItem> {
    [
        ("order", "Megrendelés"),
        ("lead", "Érdeklődő"),
        ("partner", "Partner"),
    ]
    .into_iter()
    .map(|(key, label_hu)| item(key, label_hu))
    .collect()
}

/// Every currency the money type can compute in. Adding one is backend work
/// (exponent, FX, NAV mapping) — but the pickers follow automatically.
pub fn currencies() -> Vec<LookupItem> {
    Currency::ALL
        .into_iter()
        .map(|c| {
            item(
                c.code(),
                match c {
                    Currency::HUF => "Forint",
                    Currency::EUR => "Euró",
                },
            )
        })
        .collect()
}

/// What an invoice may be issued with; refused at issue time, not by NAV.
pub fn invoice_payment_methods() -> Vec<LookupItem> {
    PaymentMethod::ALL
        .into_iter()
        .map(|key| {
            let label_hu = match key {
                "TRANSFER" => "Átutalás",
                "CASH" => "Készpénz",
                _ => key,
            };
            item(key, label_hu)
        })
        .collect()
}

/// NAV's technical-annulment codes; the annul dialog offers exactly these.
pub fn annulment_codes() -> Vec<LookupItem> {
    crate::service::invoicing::ANNULMENT_CODES
        .iter()
        .map(|key| item(key, key))
        .collect()
}

// ── Images ────────────────────────────────────────────────────────────────────

/// An image category with the two rules clients used to hard-code: whether the
/// bytes are evidence (`immutable`, the database refuses delete and re-filing)
/// and whether a person may file a photo by hand (`attachable`, production
/// only — intake and handover shots come from their own flows).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ImageCategoryEntry {
    pub key: String,
    pub label_hu: String,
    pub immutable: bool,
    pub attachable: bool,
}

pub fn image_categories() -> Vec<ImageCategoryEntry> {
    [
        (ImageCategory::Intake, "Bevétel"),
        (ImageCategory::Production, "Gyártás"),
        (ImageCategory::Completion, "Átadás/MEO"),
        (ImageCategory::Marketing, "Marketing"),
        (ImageCategory::Inspection, "Átvétel"),
    ]
    .into_iter()
    .map(|(category, label_hu)| ImageCategoryEntry {
        key: category.as_str().to_string(),
        label_hu: label_hu.to_string(),
        immutable: category.is_immutable(),
        attachable: matches!(category, ImageCategory::Production),
    })
    .collect()
}

// ── Email composer starters ───────────────────────────────────────────────────

/// A starter for the manual composer: picking one fills subject/hero/body, all
/// still editable before sending. Content lives here so a new starter or a
/// reworded one is a server deploy, not a web deploy.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EmailThemeEntry {
    pub key: String,
    pub label_hu: String,
    pub subject: String,
    pub hero: String,
    pub body: String,
}

pub fn email_themes() -> Vec<EmailThemeEntry> {
    vec![
        EmailThemeEntry {
            key: "quotation".to_string(),
            label_hu: "Árajánlat".to_string(),
            subject: "Árajánlatunk".to_string(),
            hero: "Megjött az Autotherm árajánlatod!".to_string(),
            body: [
                "Tisztelt Címzett!",
                "",
                "Köszönjük érdeklődését. Árajánlatunkat mellékelten küldjük.",
                "",
                "**Ajánlott ár:** …",
                "",
                "Kérdés esetén állunk rendelkezésére.",
            ]
            .join("\n"),
        },
        EmailThemeEntry {
            key: "promo".to_string(),
            label_hu: "Akció".to_string(),
            subject: "Autotherm akció".to_string(),
            hero: String::new(),
            body: [
                "# Újdonság",
                "",
                "Rövid bevezető ide.",
                "",
                "- pont 1",
                "- pont 2",
            ]
            .join("\n"),
        },
    ]
}

// ── Error texts ─────────────────────────────────────────────────────────────────

/// The user-facing Hungarian text per error code, from the shared catalog in
/// `crate::error` (the same table published in OpenAPI as `x-error-catalog`).
/// Clients layer this over their built-in map: the built-in map must stay for
/// cold start (a login failure happens before the first fetch), but every
/// reworded message and every new code reaches the clients with the next
/// lookups fetch — no app update.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ErrorTextEntry {
    pub code: String,
    pub text_hu: String,
}

pub fn error_texts() -> Vec<ErrorTextEntry> {
    crate::error::ERROR_CODES
        .iter()
        .map(|entry| ErrorTextEntry {
            code: entry.code.to_string(),
            text_hu: entry.hu.to_string(),
        })
        .collect()
}

// ── The endpoint payload ──────────────────────────────────────────────────────

/// Every client-facing enumeration in one document. Clients fetch it once,
/// cache it, and render selects, chips and labels from it.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Lookups {
    pub damage_types: Vec<LookupItem>,
    pub severities: Vec<LookupItem>,
    pub verdicts: Vec<LookupItem>,
    pub walkaround_kinds: Vec<LookupItem>,
    pub fuel_levels: Vec<LookupItem>,
    pub heating_fuels: Vec<LookupItem>,
    pub defrost_modes: Vec<LookupItem>,
    pub order_relations: Vec<LookupItem>,
    pub task_entity_types: Vec<LookupItem>,
    pub currencies: Vec<LookupItem>,
    pub invoice_payment_methods: Vec<LookupItem>,
    pub annulment_codes: Vec<LookupItem>,
    pub image_categories: Vec<ImageCategoryEntry>,
    pub email_themes: Vec<EmailThemeEntry>,
    pub error_texts: Vec<ErrorTextEntry>,
}

impl Lookups {
    pub fn current() -> Self {
        Lookups {
            damage_types: damage_types(),
            severities: severities(),
            verdicts: verdicts(),
            walkaround_kinds: walkaround_kinds(),
            fuel_levels: fuel_levels(),
            heating_fuels: heating_fuels(),
            defrost_modes: defrost_modes(),
            order_relations: order_relations(),
            task_entity_types: task_entity_types(),
            currencies: currencies(),
            invoice_payment_methods: invoice_payment_methods(),
            annulment_codes: annulment_codes(),
            image_categories: image_categories(),
            email_themes: email_themes(),
            error_texts: error_texts(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(items: &[LookupItem]) -> Vec<&str> {
        items.iter().map(|i| i.key.as_str()).collect()
    }

    #[test]
    fn published_keys_are_exactly_the_accepted_keys() {
        // What the endpoint lists is what validation accepts: no drift possible.
        assert_eq!(keys(&damage_types()), DAMAGE_TYPE_KEYS);
        assert_eq!(keys(&severities()), SEVERITY_KEYS);
        assert_eq!(keys(&verdicts()), VERDICT_KEYS);
        assert_eq!(keys(&walkaround_kinds()), WALKAROUND_KEYS);
        assert_eq!(keys(&fuel_levels()), FUEL_LEVEL_KEYS);
        assert_eq!(keys(&heating_fuels()), HEATING_FUEL_KEYS);
        assert_eq!(keys(&defrost_modes()), DEFROST_KEYS);
        assert_eq!(keys(&order_relations()), ORDER_RELATION_KEYS);
        assert_eq!(keys(&task_entity_types()), TASK_ENTITY_KEYS);
        assert_eq!(
            currencies()
                .iter()
                .map(|c| c.key.as_str())
                .collect::<Vec<_>>(),
            Currency::ALL.map(Currency::code),
        );
        assert_eq!(
            invoice_payment_methods()
                .iter()
                .map(|m| m.key.as_str())
                .collect::<Vec<_>>(),
            PaymentMethod::ALL,
        );
    }

    #[test]
    fn every_entry_has_a_label() {
        let all = Lookups::current();
        for list in [
            all.damage_types,
            all.severities,
            all.verdicts,
            all.walkaround_kinds,
            all.fuel_levels,
            all.heating_fuels,
            all.defrost_modes,
            all.order_relations,
            all.task_entity_types,
            all.currencies,
            all.invoice_payment_methods,
            all.annulment_codes,
        ] {
            assert!(!list.is_empty());
            for entry in &list {
                assert!(!entry.key.is_empty(), "empty key");
                assert!(
                    !entry.label_hu.trim().is_empty(),
                    "no label for {}",
                    entry.key
                );
            }
        }
        for category in &all.image_categories {
            assert!(!category.label_hu.trim().is_empty());
        }
        for theme in &all.email_themes {
            assert!(!theme.subject.trim().is_empty() || !theme.body.trim().is_empty());
        }
        assert_eq!(all.error_texts.len(), crate::error::ERROR_CODES.len());
        for entry in &all.error_texts {
            assert!(!entry.code.is_empty());
            assert!(
                !entry.text_hu.trim().is_empty(),
                "no text for {}",
                entry.code
            );
        }
    }

    #[test]
    fn exactly_production_is_hand_attachable_and_only_intake_is_immutable() {
        let categories = image_categories();
        assert_eq!(categories.len(), 5);
        for category in &categories {
            assert_eq!(
                category.attachable,
                category.key == "production",
                "{}",
                category.key
            );
            assert_eq!(
                category.immutable,
                category.key == "intake",
                "{}",
                category.key
            );
        }
    }
}
