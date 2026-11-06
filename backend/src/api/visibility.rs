//! What a person may see (0061), applied to what the API sends.
//!
//! Hiding on a screen is not enough: the phone keeps what it was sent in its cache, and the
//! JSON is one tap away in any browser. So the handlers leave out, here, what a person has
//! no right to see — prices without `ViewPrices`, invoicing without `ViewInvoices`, the
//! customer's email, phone and address without `ViewContacts`, the correspondence and the
//! enquiries without `ViewSales`. The record itself (the job, the customer's name) stays.

use serde_json::Value;

use crate::domain::media::DocumentKind;
use crate::domain::role::Capability;
use crate::repo::contacts::Contact;
use crate::repo::documents::Document;
use crate::repo::leads::LeadSummary;
use crate::repo::orders::OrderSummary;
use crate::repo::partners::Partner;
use crate::service::auth::AuthUser;

/// History entries about invoicing and payments: only with `ViewInvoices`.
const INVOICE_ACTIONS: &[&str] = &[
    "invoice_issue",
    "invoice_storno",
    "invoice_annul",
    "invoice_annulled",
    "invoice_rejected",
    "invoice_reconciled",
    "proforma_created",
    "paid",
    "unpaid",
    "payment",
    "payment_removed",
    "payment_reminder_sent",
    "reminders_off",
    "reminders_on",
    "invoice_language",
];

/// History entries about the sale: only with `ViewSales`.
const SALES_ACTIONS: &[&str] = &[
    "quote_recorded",
    "customer_replied",
    "followup_sent",
    // A partner's history lists the enquiries it made.
    "lead_created",
];

/// Whether a field of a change record carries money, at any depth.
pub fn is_money_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    k.contains("price")
        || k.ends_with("_minor")
        || k.contains("gross")
        || k.contains("amount")
        || k.contains("total")
        || k.contains("fx_rate")
        || k == "net"
        || k == "vat"
        || k == "value"
}

/// Removes every money field from a change record, however deep.
pub fn scrub_money(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|k, _| !is_money_key(k));
            for v in map.values_mut() {
                scrub_money(v);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(scrub_money),
        _ => {}
    }
}

/// Whether a history entry may be shown at all. `kind` is the timeline's (`file`, `email`,
/// `change`…; the audit log passes `change`), `action` the audit action or, for a file,
/// the document's kind.
pub fn history_entry_visible(me: &AuthUser, kind: &str, action: &str) -> bool {
    if INVOICE_ACTIONS.contains(&action) && !me.can(Capability::ViewInvoices) {
        return false;
    }
    if SALES_ACTIONS.contains(&action) && !me.can(Capability::ViewSales) {
        return false;
    }
    match kind {
        "email" => me.can(Capability::ViewSales),
        "file" => match action {
            "invoice" | "proforma" => me.can(Capability::ViewInvoices),
            "quote" => me.can(Capability::ViewSales),
            _ => true,
        },
        _ => true,
    }
}

/// Invoices and proformas need `ViewInvoices`; a quotation, or anything filed on an
/// enquiry, needs `ViewSales`. Drawings, certificates and the rest go with the job.
pub fn document_visible(me: &AuthUser, d: &Document) -> bool {
    match d.kind {
        DocumentKind::Invoice | DocumentKind::Proforma => me.can(Capability::ViewInvoices),
        DocumentKind::Quote => me.can(Capability::ViewSales),
        _ if d.lead_id.is_some() => me.can(Capability::ViewSales),
        _ => true,
    }
}

pub fn order_summaries(me: &AuthUser, rows: &mut [OrderSummary]) {
    if !me.can(Capability::ViewPrices) {
        for r in rows {
            r.total_minor = None;
        }
    }
}

pub fn partner(me: &AuthUser, p: &mut Partner) {
    if !me.can(Capability::ViewContacts) {
        p.email = None;
        p.phone = None;
        p.postal_code = None;
        p.city = None;
        p.address_line = None;
        p.notes = None;
    }
}

pub fn contacts(me: &AuthUser, rows: &mut [Contact]) {
    if !me.can(Capability::ViewContacts) {
        for c in rows {
            c.email = None;
            c.phone = None;
            c.notes = None;
        }
    }
}

/// Enquiries listed on a partner or in search: none at all without `ViewSales`, and no
/// quoted amount without `ViewPrices`.
pub fn lead_summaries(me: &AuthUser, rows: &mut Vec<LeadSummary>) {
    if !me.can(Capability::ViewSales) {
        rows.clear();
        return;
    }
    if !me.can(Capability::ViewPrices) {
        for r in rows {
            r.quoted_value_minor = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn money_goes_and_the_rest_stays() {
        let mut v = json!({
            "description": ["Hűtőgép", "Hűtőgép V-500"],
            "unit_price": [100, 200],
            "quantity": ["1", "2"],
            "nested": { "gross": 5, "line_total_minor": 7, "note": "x" },
            "items": [{ "net_amount": 1, "name": "y" }]
        });
        scrub_money(&mut v);
        assert_eq!(
            v,
            json!({
                "description": ["Hűtőgép", "Hűtőgép V-500"],
                "quantity": ["1", "2"],
                "nested": { "note": "x" },
                "items": [{ "name": "y" }]
            })
        );
    }
}
