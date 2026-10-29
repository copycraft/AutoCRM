use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

/// Replays scripted answers in order; records every check request it saw.
#[derive(Default)]
struct Script {
    answers: Mutex<VecDeque<(u16, String)>>,
    status: Option<String>,
    seen: Mutex<Vec<serde_json::Value>>,
    status_calls: AtomicU32,
    delay: Duration,
}

fn fault(code: &str) -> (u16, String) {
    (200, format!(r#"{{"actionSucceed":false,"errorWrappers":[{{"error":"{code}"}}]}}"#))
}

fn valid() -> (u16, String) {
    (200, r#"{"valid":true,"name":"ACME","address":"---","requestIdentifier":"WAPI1"}"#.into())
}

impl Transport for Arc<Script> {
    async fn call(
        &self,
        path: &str,
        body: Option<serde_json::Value>,
        _timeout: Duration,
    ) -> Result<(u16, String), String> {
        if path == "/check-status" {
            self.status_calls.fetch_add(1, Ordering::SeqCst);
            return self.status.clone().map(|s| (200, s)).ok_or("NETWORK_ERROR".into());
        }
        self.seen.lock().unwrap().push(body.unwrap());
        tokio::time::sleep(self.delay).await;
        Ok(self.answers.lock().unwrap().pop_front().unwrap_or_else(|| fault("SERVICE_UNAVAILABLE")))
    }
}

fn client(script: Script, requester: Option<&str>) -> (ViesClient<Arc<Script>>, Arc<Script>) {
    let script = Arc::new(script);
    let config = ViesConfig {
        requester_number: requester.map(Into::into),
        backoff_base: Duration::ZERO,
        ..ViesConfig::default()
    };
    (ViesClient::with_transport(script.clone(), config), script)
}

fn script(answers: Vec<(u16, String)>) -> Script {
    Script { answers: Mutex::new(answers.into()), ..Script::default() }
}

fn de() -> NormalizedVat {
    normalize_vat("DE123456789", None).unwrap()
}

#[tokio::test]
async fn concurrency_errors_are_retried_until_a_verdict() {
    let (client, s) = client(script(vec![fault("MS_MAX_CONCURRENT_REQ"), fault("MS_MAX_CONCURRENT_REQ"), valid()]), None);
    let out = client.check(&de(), CheckOptions::default()).await;
    assert_eq!(out.status, VatCheckStatus::Valid);
    assert_eq!(out.attempts, 3);
    assert_eq!(out.name.as_deref(), Some("ACME"));
    assert_eq!(out.address, None);
    assert_eq!(s.seen.lock().unwrap().len(), 3);
}

#[tokio::test]
async fn invalid_input_is_final() {
    let (client, s) = client(script(vec![fault("INVALID_INPUT"), valid()]), None);
    let out = client.check(&de(), CheckOptions::default()).await;
    assert_eq!(out.status, VatCheckStatus::Error);
    assert_eq!(out.error_code.as_deref(), Some("INVALID_INPUT"));
    assert_eq!(s.seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn known_member_state_outage_skips_retries() {
    let mut sc = script(vec![fault("MS_UNAVAILABLE"), valid()]);
    sc.status = Some(r#"{"vow":{"available":true},"countries":[{"countryCode":"DE","availability":"Unavailable"}]}"#.into());
    let (client, s) = client(sc, None);
    let out = client.check(&de(), CheckOptions::default()).await;
    assert_eq!(out.status, VatCheckStatus::Unavailable);
    assert_eq!(out.error_code.as_deref(), Some("MS_UNAVAILABLE"));
    assert_eq!(s.seen.lock().unwrap().len(), 1);

    // The status is cached.
    client.status().await;
    assert_eq!(s.status_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn member_state_outage_is_retried_when_status_says_available() {
    let mut sc = script(vec![fault("MS_UNAVAILABLE"), valid()]);
    sc.status = Some(r#"{"vow":{"available":true},"countries":[{"countryCode":"DE","availability":"Available"}]}"#.into());
    let (client, _) = client(sc, None);
    assert_eq!(client.check(&de(), CheckOptions::default()).await.status, VatCheckStatus::Valid);
}

#[tokio::test]
async fn rejected_requester_is_dropped_and_retried_once() {
    let (client, s) = client(script(vec![fault("INVALID_REQUESTER_INFO"), valid()]), Some("87654321"));
    let out = client.check(&de(), CheckOptions::default()).await;
    assert_eq!(out.status, VatCheckStatus::Valid);
    assert_eq!(out.hints, vec![HINT_REQUESTER_REJECTED]);
    assert_eq!(out.requester, None);
    let seen = s.seen.lock().unwrap();
    assert_eq!(seen[0]["requesterNumber"], "87654321");
    assert_eq!(seen[0]["requesterMemberStateCode"], "HU");
    assert!(seen[1].get("requesterNumber").is_none());
}

#[tokio::test]
async fn requester_is_recorded_when_accepted() {
    let (client, _) = client(script(vec![valid()]), Some("87654321"));
    let out = client.check(&de(), CheckOptions::default()).await;
    assert_eq!(out.requester.as_deref(), Some("HU87654321"));
    assert_eq!(out.request_identifier.as_deref(), Some("WAPI1"));
}

#[tokio::test]
async fn exhausted_retries_are_unavailable_never_invalid() {
    let (client, s) = client(script(vec![]), None);
    let out = client.check(&de(), CheckOptions::default()).await;
    assert_eq!(out.status, VatCheckStatus::Unavailable);
    assert_eq!(out.error_code.as_deref(), Some("SERVICE_UNAVAILABLE"));
    assert_eq!(s.seen.lock().unwrap().len(), 3);
}

#[tokio::test]
async fn unknown_codes_retry_once() {
    let (client, s) = client(script(vec![fault("SOMETHING_NEW"), fault("SOMETHING_NEW"), valid()]), None);
    let out = client.check(&de(), CheckOptions::default()).await;
    assert_eq!(out.status, VatCheckStatus::Unavailable);
    assert_eq!(s.seen.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn ip_blocked_is_an_error() {
    let (client, _) = client(script(vec![fault("IP_BLOCKED")]), None);
    let out = client.check(&de(), CheckOptions::default()).await;
    assert_eq!((out.status, out.error_code.as_deref()), (VatCheckStatus::Error, Some("IP_BLOCKED")));
}

#[tokio::test]
async fn concurrent_checks_of_one_number_share_a_request() {
    let mut sc = script(vec![valid(), valid()]);
    sc.delay = Duration::from_millis(50);
    let (client, s) = client(sc, None);
    let vat = de();
    let (a, b) = tokio::join!(
        client.check(&vat, CheckOptions::default()),
        client.check(&vat, CheckOptions::default())
    );
    assert_eq!((a.status, b.status), (VatCheckStatus::Valid, VatCheckStatus::Valid));
    assert_eq!(s.seen.lock().unwrap().len(), 1);

    // Once finished, the next check goes upstream again.
    client.check(&vat, CheckOptions::default()).await;
    assert_eq!(s.seen.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn result_merges_format_and_outcome_hints() {
    let (client, _) = client(script(vec![fault("INVALID_REQUESTER_INFO"), valid()]), Some("1"));
    let vat = normalize_vat("12345678-4-06", Some("HU")).unwrap();
    let out = client.check(&vat, CheckOptions::default()).await;
    let result = VatCheckResult::from_outcome(&vat, &out, chrono::Utc::now(), None);
    assert_eq!(result.hints, vec!["HU_VAT_GROUP_MEMBER", "REQUESTER_REJECTED"]);
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["status"], "VALID");
    assert_eq!(json["hu_domestic_tax_number"], "12345678406");
}
