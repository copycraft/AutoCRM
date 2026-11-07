//! Reading a `/check-vat-number` reply. VIES reports errors in the body, with HTTP 200 or
//! not, and proxies in front of it can answer with HTML, so the rule order matters:
//! errorWrappers first, then transport status, then the success shape.

use serde::Deserialize;

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ViesRawResponse {
    pub country_code: Option<String>,
    pub vat_number: Option<String>,
    pub request_date: Option<String>,
    pub valid: Option<bool>,
    pub request_identifier: Option<String>,
    pub name: Option<String>,
    pub address: Option<String>,
    pub action_succeed: Option<bool>,
    #[serde(default)]
    pub error_wrappers: Vec<ErrorWrapper>,
}

#[derive(Deserialize, Debug)]
pub struct ErrorWrapper {
    pub error: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub valid: bool,
    pub name: Option<String>,
    pub address: Option<String>,
    pub request_identifier: Option<String>,
    pub request_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    Verdict(Verdict),
    /// A VIES error code, `HTTP_<status>` or `PARSE_ERROR`.
    Fault(String),
}

/// The parsed reply plus the body as JSON (or as a JSON string when it wasn't JSON), kept
/// for the audit log.
pub fn parse_check_reply(http_status: u16, body: &str) -> (Reply, serde_json::Value) {
    let json: Option<serde_json::Value> = serde_json::from_str(body).ok();
    let raw = json
        .clone()
        .unwrap_or_else(|| serde_json::Value::String(body.chars().take(2000).collect()));
    let parsed: Option<ViesRawResponse> = json
        .filter(|v| v.is_object())
        .and_then(|v| serde_json::from_value(v).ok());

    if let Some(code) = parsed
        .as_ref()
        .and_then(|p| p.error_wrappers.first())
        .map(|w| w.error.trim().to_string())
    {
        return (Reply::Fault(code), raw);
    }
    let Some(parsed) = parsed.filter(|_| (200..300).contains(&http_status)) else {
        let code = if (200..300).contains(&http_status) {
            "PARSE_ERROR".to_string()
        } else {
            format!("HTTP_{http_status}")
        };
        return (Reply::Fault(code), raw);
    };
    let Some(valid) = parsed.valid else {
        return (Reply::Fault("PARSE_ERROR".into()), raw);
    };
    (
        Reply::Verdict(Verdict {
            valid,
            name: published(parsed.name),
            address: published(parsed.address),
            request_identifier: published(parsed.request_identifier),
            request_date: published(parsed.request_date),
        }),
        raw,
    )
}

/// Member states that don't publish a field answer "---".
fn published(value: Option<String>) -> Option<String> {
    value.filter(|v| {
        let t = v.trim();
        !t.is_empty() && t != "---"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success() {
        let body = r#"{"countryCode":"HU","vatNumber":"12345678","requestDate":"2026-10-28T08:55:12.345Z","valid":true,"requestIdentifier":"WAPIAAAAZ","name":"ACME KFT","address":"BUDAPEST\nFŐ UTCA 1.","traderName":"---","traderNameMatch":"NOT_PROCESSED"}"#;
        let (reply, raw) = parse_check_reply(200, body);
        assert_eq!(
            reply,
            Reply::Verdict(Verdict {
                valid: true,
                name: Some("ACME KFT".into()),
                address: Some("BUDAPEST\nFŐ UTCA 1.".into()),
                request_identifier: Some("WAPIAAAAZ".into()),
                request_date: Some("2026-10-28T08:55:12.345Z".into()),
            })
        );
        assert_eq!(raw["countryCode"], "HU");
    }

    #[test]
    fn dashes_become_none() {
        let body = r#"{"valid":true,"name":"---","address":"---","requestIdentifier":""}"#;
        let (Reply::Verdict(v), _) = parse_check_reply(200, body) else {
            panic!()
        };
        assert_eq!(
            (v.name, v.address, v.request_identifier),
            (None, None, None)
        );
    }

    #[test]
    fn invalid_number_is_a_verdict() {
        let (Reply::Verdict(v), _) = parse_check_reply(200, r#"{"valid":false}"#) else {
            panic!()
        };
        assert!(!v.valid);
    }

    #[test]
    fn error_wrappers_win_over_http_status() {
        let body =
            r#"{"actionSucceed":false,"errorWrappers":[{"error":"MS_UNAVAILABLE","message":"x"}]}"#;
        assert_eq!(
            parse_check_reply(200, body).0,
            Reply::Fault("MS_UNAVAILABLE".into())
        );
        assert_eq!(
            parse_check_reply(500, body).0,
            Reply::Fault("MS_UNAVAILABLE".into())
        );
    }

    #[test]
    fn html_502_is_a_transport_error() {
        let (reply, raw) = parse_check_reply(502, "<html><body>Bad Gateway</body></html>");
        assert_eq!(reply, Reply::Fault("HTTP_502".into()));
        assert!(raw.as_str().unwrap().contains("Bad Gateway"));
    }

    #[test]
    fn missing_valid_is_a_parse_error() {
        assert_eq!(
            parse_check_reply(200, r#"{"countryCode":"HU"}"#).0,
            Reply::Fault("PARSE_ERROR".into())
        );
        assert_eq!(
            parse_check_reply(200, "not json").0,
            Reply::Fault("PARSE_ERROR".into())
        );
    }
}
