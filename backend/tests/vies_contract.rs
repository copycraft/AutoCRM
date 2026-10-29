//! Contract tests against the real VIES test service. Opt-in: `VIES_CONTRACT_TESTS=1`.

use std::time::Duration;

use autocrm::integrations::vies::{
    CheckOptions, VatCheckStatus, ViesClient, ViesConfig, normalize_vat,
};

#[tokio::test]
async fn test_service_answers_map_to_our_statuses() {
    if std::env::var("VIES_CONTRACT_TESTS").as_deref() != Ok("1") {
        eprintln!("skipped: set VIES_CONTRACT_TESTS=1");
        return;
    }
    let client = ViesClient::new(ViesConfig {
        use_test_service: true,
        backoff_base: Duration::from_millis(200),
        ..ViesConfig::default()
    })
    .unwrap();
    let cases = [
        ("100", VatCheckStatus::Valid, None),
        ("200", VatCheckStatus::Invalid, None),
        ("201", VatCheckStatus::Error, Some("INVALID_INPUT")),
        ("300", VatCheckStatus::Unavailable, None),
        ("301", VatCheckStatus::Unavailable, None),
        ("302", VatCheckStatus::Unavailable, None),
        ("401", VatCheckStatus::Error, Some("IP_BLOCKED")),
        ("600", VatCheckStatus::Unavailable, None),
    ];
    for (number, status, code) in cases {
        let vat = normalize_vat(&format!("DE{number}"), None).unwrap();
        let out = client.check(&vat, CheckOptions::default()).await;
        assert_eq!(out.status, status, "{number}: {out:?}");
        if let Some(code) = code {
            assert_eq!(out.error_code.as_deref(), Some(code), "{number}");
        }
        if number == "600" {
            assert_eq!(out.attempts, 3, "600 should be retried");
        }
    }
}
