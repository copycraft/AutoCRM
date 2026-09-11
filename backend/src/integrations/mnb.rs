//! Magyar Nemzeti Bank official exchange rates (the rates Hungarian accounting expects).
//!
//! The service is SOAP over plain http (the https endpoint returns 404). A MITM could
//! alter rates in transit; the risk is accepted for reference data, and every fetched rate
//! is stored with its fetch time so anomalies are traceable.

use std::time::Duration;

use anyhow::{Context, anyhow, bail};
use chrono::NaiveDate;
use quick_xml::Reader;
use quick_xml::events::Event;
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MnbRate {
    pub day: NaiveDate,
    pub currency: String,
    /// HUF per one unit of `currency` (already divided by MNB's unit, e.g. 100 JPY).
    pub huf_per_unit: Decimal,
}

#[derive(Clone)]
pub struct MnbClient {
    http: reqwest::Client,
    endpoint: String,
}

impl MnbClient {
    pub fn new(endpoint: &str) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("autocrm/0.1 (fx rates)")
            .build()?;
        Ok(MnbClient {
            http,
            endpoint: endpoint.to_string(),
        })
    }

    pub async fn exchange_rates(
        &self,
        from: NaiveDate,
        to: NaiveDate,
        currencies: &[&str],
    ) -> anyhow::Result<Vec<MnbRate>> {
        let body = format!(
            r#"<soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" xmlns:web="http://www.mnb.hu/webservices/"><soapenv:Header/><soapenv:Body><web:GetExchangeRates><web:startDate>{from}</web:startDate><web:endDate>{to}</web:endDate><web:currencyNames>{}</web:currencyNames></web:GetExchangeRates></soapenv:Body></soapenv:Envelope>"#,
            currencies.join(",")
        );
        let response = self
            .http
            .post(&self.endpoint)
            .header("Content-Type", "text/xml; charset=utf-8")
            .header(
                "SOAPAction",
                "\"http://www.mnb.hu/webservices/MNBArfolyamServiceSoap/GetExchangeRates\"",
            )
            .body(body)
            .send()
            .await
            .context("calling MNB")?;
        let status = response.status();
        let text = response.text().await.context("reading MNB response")?;
        if !status.is_success() {
            bail!(
                "MNB returned HTTP {status}: {}",
                text.chars().take(300).collect::<String>()
            );
        }
        parse_response(&text)
    }
}

/// The SOAP result is itself an XML document, escaped inside <GetExchangeRatesResult>.
pub fn parse_response(soap: &str) -> anyhow::Result<Vec<MnbRate>> {
    let inner = extract_result(soap)?;
    parse_rates(&inner)
}

fn extract_result(soap: &str) -> anyhow::Result<String> {
    let mut reader = Reader::from_str(soap);
    let mut inside = false;
    let mut out = String::new();
    loop {
        match reader.read_event()? {
            Event::Start(e) if e.local_name().as_ref() == b"GetExchangeRatesResult" => {
                inside = true
            }
            Event::End(e) if e.local_name().as_ref() == b"GetExchangeRatesResult" => {
                return Ok(out);
            }
            Event::Text(t) if inside => out.push_str(&t.unescape()?),
            Event::Eof => bail!("GetExchangeRatesResult not found in MNB response"),
            _ => {}
        }
    }
}

fn parse_rates(xml: &str) -> anyhow::Result<Vec<MnbRate>> {
    let mut reader = Reader::from_str(xml);
    let mut rates = Vec::new();
    let mut day: Option<NaiveDate> = None;
    let mut pending: Option<(String, Decimal)> = None; // (currency, unit)
    loop {
        match reader.read_event()? {
            Event::Start(e) if e.local_name().as_ref() == b"Day" => {
                let date = attribute(&e, b"date")?.ok_or_else(|| anyhow!("Day without date"))?;
                day = Some(
                    NaiveDate::parse_from_str(&date, "%Y-%m-%d")
                        .with_context(|| format!("bad date {date}"))?,
                );
            }
            Event::Start(e) if e.local_name().as_ref() == b"Rate" => {
                let currency =
                    attribute(&e, b"curr")?.ok_or_else(|| anyhow!("Rate without curr"))?;
                let unit: Decimal = attribute(&e, b"unit")?
                    .as_deref()
                    .unwrap_or("1")
                    .parse()
                    .context("bad unit")?;
                if unit <= Decimal::ZERO {
                    bail!("non-positive unit for {currency}");
                }
                pending = Some((currency, unit));
            }
            Event::Text(t) => {
                if let (Some(d), Some((currency, unit))) = (day, pending.take()) {
                    // Hungarian decimal comma: "365,22000"
                    let raw = t.unescape()?.trim().replace(',', ".");
                    let value: Decimal = raw
                        .parse()
                        .with_context(|| format!("bad rate '{raw}' for {currency}"))?;
                    if value <= Decimal::ZERO {
                        bail!("non-positive rate for {currency} on {d}");
                    }
                    rates.push(MnbRate {
                        day: d,
                        currency,
                        huf_per_unit: value / unit,
                    });
                }
            }
            Event::End(e) if e.local_name().as_ref() == b"Day" => day = None,
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(rates)
}

fn attribute(e: &quick_xml::events::BytesStart<'_>, name: &[u8]) -> anyhow::Result<Option<String>> {
    for attr in e.attributes() {
        let attr = attr?;
        if attr.key.local_name().as_ref() == name {
            return Ok(Some(attr.unescape_value()?.into_owned()));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Captured from the live service on 2026-09-10.
    const SAMPLE: &str = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><GetExchangeRatesResponse xmlns="http://www.mnb.hu/webservices/" xmlns:i="http://www.w3.org/2001/XMLSchema-instance"><GetExchangeRatesResult>&lt;MNBExchangeRates&gt;&lt;Day date="2026-09-08"&gt;&lt;Rate unit="1" curr="EUR"&gt;365,22000&lt;/Rate&gt;&lt;Rate unit="100" curr="JPY"&gt;204,01000&lt;/Rate&gt;&lt;/Day&gt;&lt;Day date="2026-09-07"&gt;&lt;Rate unit="1" curr="EUR"&gt;362,10000&lt;/Rate&gt;&lt;Rate unit="100" curr="JPY"&gt;201,70000&lt;/Rate&gt;&lt;/Day&gt;&lt;Day date="2026-09-04"&gt;&lt;Rate unit="1" curr="EUR"&gt;363,61000&lt;/Rate&gt;&lt;Rate unit="100" curr="JPY"&gt;199,96000&lt;/Rate&gt;&lt;/Day&gt;&lt;/MNBExchangeRates&gt;</GetExchangeRatesResult></GetExchangeRatesResponse></s:Body></s:Envelope>"#;

    fn d(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    #[test]
    fn parses_live_sample() {
        let rates = parse_response(SAMPLE).unwrap();
        assert_eq!(rates.len(), 6);
        assert_eq!(
            rates[0],
            MnbRate {
                day: "2026-09-08".parse().unwrap(),
                currency: "EUR".into(),
                huf_per_unit: d("365.22")
            }
        );
        // JPY is quoted per 100 units.
        assert_eq!(rates[1].huf_per_unit, d("2.0401"));
        // Weekend (09-05, 09-06) is simply absent.
        assert!(rates.iter().all(|r| r.day.to_string() != "2026-09-05"));
    }

    #[test]
    fn empty_result_is_empty() {
        let soap = r#"<Envelope><Body><GetExchangeRatesResult>&lt;MNBExchangeRates /&gt;</GetExchangeRatesResult></Body></Envelope>"#;
        assert!(parse_response(soap).unwrap().is_empty());
    }

    #[test]
    fn missing_result_is_an_error() {
        assert!(parse_response("<Envelope><Body><Fault>nope</Fault></Body></Envelope>").is_err());
    }
}
