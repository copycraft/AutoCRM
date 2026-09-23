//! Client for the NAV invoicing sidecar (`nav-sidecar/`).
//!
//! The sidecar owns everything NAV-shaped — the XML, the request signature, the exchange
//! token, the polling loop — and speaks flat JSON to us. So this module is only a typed
//! HTTP client: structs that match its contract, and an error type that keeps NAV's own
//! fault code intact all the way to the screen. A report NAV refused says why, in NAV's
//! words, because the person who has to fix it is the one reading.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::config::NavConfig;

// ── Requests ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
    pub postal_code: String,
    pub city: String,
    pub street_name: String,
    pub public_place_category: String,
    pub number: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Supplier {
    pub name: String,
    pub tax_number: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_account: Option<String>,
    pub address: Address,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Customer {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax_number: Option<String>,
    /// `DOMESTIC`, `OTHER` or `PRIVATE_PERSON`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vat_status: Option<String>,
    pub address: Address,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub description: String,
    /// Decimal string. Never a float: an invoice is reconciled, not approximated.
    pub quantity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Net unit price, as a decimal string in the invoice currency.
    pub unit_price: String,
    /// The fraction: "0.27", not "27".
    pub vat_percentage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nature: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceRequest {
    pub invoice_number: String,
    pub issue_date: NaiveDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_date: Option<NaiveDate>,
    pub currency: String,
    /// Rate to HUF. Required by NAV for anything but a HUF invoice.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exchange_rate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_numbers: Option<Vec<String>>,
    pub supplier: Supplier,
    pub customer: Customer,
    pub lines: Vec<Line>,
}

/// The proforma request is the invoice request plus rendering options. Same data, and no
/// NAV call at the other end.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProformaRequest {
    #[serde(flatten)]
    pub invoice: InvoiceRequest,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StornoRequest {
    pub storno_invoice_number: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnulRequest {
    /// `ERRATIC_DATA`, `ERRATIC_INVOICE_NUMBER`, `ERRATIC_INVOICE_ISSUE_DATE` or
    /// `ERRATIC_ELECTRONIC_HASH_VALUE`.
    pub code: String,
    pub reason: String,
}

// ── Responses ───────────────────────────────────────────────────────────────

/// One finding, as the sidecar flattens NAV's technical and business messages.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NavMessage {
    /// `technical`, `business`, `local` or `request`.
    pub source: String,
    /// `ERROR`, `WARN` or `INFO`.
    pub level: String,
    /// NAV's own fault code, where there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub currency: String,
    pub net: String,
    pub vat: String,
    pub gross: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmissionResponse {
    pub invoice_number: String,
    pub operation: String,
    pub transaction_id: String,
    /// NAV's per-invoice status: `DONE` when stored.
    pub status: String,
    pub accepted: bool,
    #[serde(default)]
    pub messages: Vec<NavMessage>,
    #[serde(default)]
    pub totals: Option<Totals>,
    #[serde(default)]
    pub original_invoice_number: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProformaResponse {
    pub id: String,
    pub invoice_number: String,
    /// Always false. The sidecar says so on every proforma, and so do we.
    pub reported_to_nav: bool,
    #[serde(default)]
    pub totals: Option<Totals>,
    pub pdf_base64: String,
    #[serde(default)]
    pub messages: Vec<NavMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChainElement {
    pub invoice_number: String,
    /// `CREATE`, `MODIFY` or `STORNO`.
    pub operation: String,
    pub supplier_tax_number: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customer_tax_number: Option<String>,
    pub ins_date: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_index: Option<i64>,
    pub original_request_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifies: Option<ChainReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChainReference {
    pub original_invoice_number: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modification_index: Option<i64>,
    pub modify_without_master: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainResponse {
    pub invoice_number: String,
    pub direction: String,
    #[serde(default)]
    pub elements: Vec<ChainElement>,
}

/// The sidecar's error envelope, kept whole.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavFault {
    /// `bad_request`, `validation`, `nav_rejected`, `nav_error`, `nav_unreachable`,
    /// `not_found`, `config` or `internal`.
    pub kind: String,
    pub message: String,
    #[serde(default)]
    pub nav_error_code: Option<String>,
    #[serde(default)]
    pub nav_func_code: Option<String>,
    #[serde(default)]
    pub transaction_id: Option<String>,
    #[serde(default)]
    pub messages: Vec<NavMessage>,
}

#[derive(Debug, Deserialize)]
struct FaultEnvelope {
    error: NavFault,
}

#[derive(Debug, thiserror::Error)]
pub enum NavError {
    /// The sidecar could not be reached or did not answer in time. Worth retrying: the
    /// report may not have been made at all, and the queue will come back to it.
    #[error("the NAV sidecar is unreachable: {0}")]
    Unreachable(String),
    /// The sidecar, or NAV through it, said no. Never retried: a rejection is an answer.
    #[error("{}", .0.message)]
    Refused(Box<NavFault>),
    /// Answered with something that is not the documented contract.
    #[error("the NAV sidecar answered with something unexpected: {0}")]
    Contract(String),
}

impl NavError {
    /// Whether running the same request again could plausibly succeed.
    ///
    /// A submission that reached NAV is never retried on our side regardless: the caller
    /// decides that, because a retried report is a duplicate invoice, not a repeated read.
    pub fn is_retryable(&self) -> bool {
        match self {
            NavError::Unreachable(_) => true,
            // The sidecar's own 504: NAV took the batch but the verdict never settled.
            // Asking again reads the same transaction rather than filing a second one.
            NavError::Refused(fault) => fault.kind == "nav_unreachable",
            NavError::Contract(_) => false,
        }
    }

    /// NAV's fault code, when the failure came from NAV rather than from the wire.
    pub fn nav_error_code(&self) -> Option<&str> {
        match self {
            NavError::Refused(fault) => fault
                .nav_error_code
                .as_deref()
                .or_else(|| fault.messages.iter().find_map(|m| m.code.as_deref())),
            _ => None,
        }
    }

    pub fn messages(&self) -> &[NavMessage] {
        match self {
            NavError::Refused(fault) => &fault.messages,
            _ => &[],
        }
    }

    pub fn fault(&self) -> Option<&NavFault> {
        match self {
            NavError::Refused(fault) => Some(fault),
            _ => None,
        }
    }
}

pub type NavResult<T> = Result<T, NavError>;

/// Percent-encodes one path segment.
///
/// NAV allows characters in an invoice number that a URL does not — a slash, most
/// awkwardly — and ours are tame, but the encoding is not optional for a value that comes
/// from data.
fn encode_segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[derive(Clone)]
pub struct NavSidecar {
    http: reqwest::Client,
    base_url: String,
}

impl NavSidecar {
    pub fn new(cfg: &NavConfig) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(cfg.timeout)
            .user_agent("autocrm/0.1 (nav invoicing)")
            .build()?;
        Ok(NavSidecar {
            http,
            base_url: cfg.sidecar_url.clone(),
        })
    }

    /// Reports a new invoice and waits for NAV's verdict (the sidecar polls for us).
    pub async fn create_invoice(&self, body: &InvoiceRequest) -> NavResult<SubmissionResponse> {
        self.post_json("/invoices", body).await
    }

    /// Cancels an issued invoice in full. The path names the original.
    pub async fn storno(
        &self,
        original_number: &str,
        body: &StornoRequest,
    ) -> NavResult<SubmissionResponse> {
        let path = format!("/invoices/{}/storno", encode_segment(original_number));
        self.post_json(&path, body).await
    }

    /// Technically annuls a data report that should never have been filed.
    pub async fn annul(
        &self,
        invoice_number: &str,
        body: &AnnulRequest,
    ) -> NavResult<SubmissionResponse> {
        let path = format!("/invoices/{}/annul", encode_segment(invoice_number));
        self.post_json(&path, body).await
    }

    /// Renders a proforma. No NAV call happens at the other end.
    pub async fn create_proforma(&self, body: &ProformaRequest) -> NavResult<ProformaResponse> {
        self.post_json("/proformas", body).await
    }

    /// The invoice's modification/storno chain, as NAV records it.
    pub async fn chain(&self, invoice_number: &str) -> NavResult<ChainResponse> {
        let path = format!("/invoices/{}/chain", encode_segment(invoice_number));
        let response = self.send(reqwest::Method::GET, &path, None::<&()>).await?;
        Self::json(response).await
    }

    /// The invoice as a PDF, rendered from what NAV holds.
    pub async fn invoice_pdf(&self, invoice_number: &str) -> NavResult<Vec<u8>> {
        let path = format!("/invoices/{}/pdf", encode_segment(invoice_number));
        let response = self.send(reqwest::Method::GET, &path, None::<&()>).await?;
        let response = Self::check_status(response).await?;
        let bytes = response
            .bytes()
            .await
            .map_err(|e| NavError::Unreachable(format!("reading the PDF: {e}")))?;
        if !bytes.starts_with(b"%PDF-") {
            return Err(NavError::Contract(
                "the invoice PDF endpoint did not return a PDF".into(),
            ));
        }
        Ok(bytes.to_vec())
    }

    /// Liveness. Uses no credentials and does not contact NAV, so it is safe to poll.
    pub async fn health(&self) -> NavResult<()> {
        let response = self
            .send(reqwest::Method::GET, "/health", None::<&()>)
            .await?;
        Self::check_status(response).await.map(|_| ())
    }

    async fn post_json<B: Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> NavResult<T> {
        let response = self
            .send(reqwest::Method::POST, path, Some(body))
            .await?;
        Self::json(response).await
    }

    async fn send<B: Serialize>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&B>,
    ) -> NavResult<reqwest::Response> {
        let url = format!("{}{path}", self.base_url);
        let mut request = self.http.request(method, &url);
        if let Some(body) = body {
            request = request.json(body);
        }
        request.send().await.map_err(|e| {
            // A timeout here is not proof that nothing was reported: the sidecar may have
            // submitted and be waiting on NAV. The caller must treat it as "unknown".
            NavError::Unreachable(format!("{e}"))
        })
    }

    async fn json<T: for<'de> Deserialize<'de>>(response: reqwest::Response) -> NavResult<T> {
        let response = Self::check_status(response).await?;
        let text = response
            .text()
            .await
            .map_err(|e| NavError::Unreachable(format!("reading the response: {e}")))?;
        serde_json::from_str(&text)
            .map_err(|e| NavError::Contract(format!("{e}; body was: {}", truncate(&text, 400))))
    }

    /// Turns a non-2xx answer into a `Refused` carrying the sidecar's envelope, so NAV's
    /// fault code survives the trip.
    async fn check_status(response: reqwest::Response) -> NavResult<reqwest::Response> {
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        match serde_json::from_str::<FaultEnvelope>(&text) {
            Ok(envelope) => Err(NavError::Refused(Box::new(envelope.error))),
            Err(_) => {
                // Not the sidecar's envelope: something else answered, or it fell over.
                let message = format!("HTTP {status}: {}", truncate(&text, 400));
                if status.is_server_error() {
                    Err(NavError::Unreachable(message))
                } else {
                    Err(NavError::Contract(message))
                }
            }
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_string();
    }
    trimmed.chars().take(max).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_segments_are_encoded() {
        assert_eq!(encode_segment("AT2026-0001"), "AT2026-0001");
        assert_eq!(encode_segment("2026/A 1"), "2026%2FA%201");
    }

    #[test]
    fn a_rejection_keeps_navs_fault_code() {
        let body = r#"{"error":{"kind":"nav_rejected","message":"NAV rejected the invoice",
            "transactionId":"4Q7ZXY8K2M1N",
            "messages":[{"source":"business","level":"ERROR","code":"INVOICE_NUMBER_ALREADY_EXISTS",
                         "message":"Invoice number already exists"}]}}"#;
        let envelope: FaultEnvelope = serde_json::from_str(body).unwrap();
        let error = NavError::Refused(Box::new(envelope.error));
        assert_eq!(error.nav_error_code(), Some("INVOICE_NUMBER_ALREADY_EXISTS"));
        assert!(!error.is_retryable());
        assert_eq!(error.messages().len(), 1);
    }

    #[test]
    fn a_stalled_transaction_is_retryable_but_a_rejection_is_not() {
        let stalled: FaultEnvelope = serde_json::from_str(
            r#"{"error":{"kind":"nav_unreachable","message":"did not settle","messages":[]}}"#,
        )
        .unwrap();
        assert!(NavError::Refused(Box::new(stalled.error)).is_retryable());

        let refused: FaultEnvelope = serde_json::from_str(
            r#"{"error":{"kind":"validation","message":"bad tax number","messages":[]}}"#,
        )
        .unwrap();
        assert!(!NavError::Refused(Box::new(refused.error)).is_retryable());
        assert!(NavError::Unreachable("connection refused".into()).is_retryable());
    }

    #[test]
    fn the_invoice_request_serialises_the_way_the_sidecar_expects() {
        let request = InvoiceRequest {
            invoice_number: "AT2026-0001".into(),
            issue_date: NaiveDate::from_ymd_opt(2026, 9, 21).unwrap(),
            delivery_date: None,
            payment_date: Some(NaiveDate::from_ymd_opt(2026, 9, 29).unwrap()),
            currency: "HUF".into(),
            exchange_rate: None,
            payment_method: Some("TRANSFER".into()),
            order_numbers: Some(vec!["2026-0042".into()]),
            supplier: Supplier {
                name: "Autotherm Kft".into(),
                tax_number: "12345678".into(),
                bank_account: None,
                address: Address {
                    country_code: None,
                    postal_code: "1117".into(),
                    city: "Budapest".into(),
                    street_name: "Kossuth".into(),
                    public_place_category: "utca".into(),
                    number: "12".into(),
                },
            },
            customer: Customer {
                name: "Vevő Kft".into(),
                tax_number: Some("99887764".into()),
                vat_status: Some("DOMESTIC".into()),
                address: Address {
                    country_code: Some("HU".into()),
                    postal_code: "6000".into(),
                    city: "Kecskemét".into(),
                    street_name: "Petőfi".into(),
                    public_place_category: "tér".into(),
                    number: "3".into(),
                },
            },
            lines: vec![Line {
                description: "Hűtőfelépítmény".into(),
                quantity: "1".into(),
                unit: Some("PIECE".into()),
                unit_price: "1200000.00".into(),
                vat_percentage: "0.27".into(),
                nature: Some("SERVICE".into()),
            }],
        };
        let json = serde_json::to_value(&request).unwrap();
        // camelCase keys, and nothing the sidecar's strict schema would refuse.
        assert_eq!(json["invoiceNumber"], "AT2026-0001");
        assert_eq!(json["issueDate"], "2026-09-21");
        assert_eq!(json["lines"][0]["vatPercentage"], "0.27");
        assert_eq!(json["supplier"]["taxNumber"], "12345678");
        assert!(json.get("deliveryDate").is_none(), "absent fields are omitted");
        assert!(json["supplier"].get("bankAccount").is_none());
    }
}
