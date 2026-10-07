//! NHTSA's free VIN decoder (vPIC). Knows North American vehicles well and European ones
//! partly: often the make, sometimes the model. Only asked when `VIN_DECODER_ONLINE` is on,
//! since the VIN leaves the country.

use std::time::Duration;

use serde::Deserialize;

const ENDPOINT: &str = "https://vpic.nhtsa.dot.gov/api/vehicles/DecodeVinValues";

pub struct Found {
    pub make: Option<String>,
    pub model: Option<String>,
    pub model_year: Option<i32>,
}

#[derive(Deserialize)]
struct Response {
    #[serde(rename = "Results")]
    results: Vec<Row>,
}

#[derive(Deserialize)]
struct Row {
    #[serde(rename = "Make")]
    make: Option<String>,
    #[serde(rename = "Model")]
    model: Option<String>,
    #[serde(rename = "ModelYear")]
    model_year: Option<String>,
}

fn present(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

pub async fn lookup(vin: &str) -> anyhow::Result<Found> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()?;
    let response: Response = http
        .get(format!("{ENDPOINT}/{vin}"))
        .query(&[("format", "json")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let row = response
        .results
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("empty answer"))?;
    Ok(Found {
        make: present(row.make),
        model: present(row.model),
        model_year: present(row.model_year).and_then(|y| y.parse().ok()),
    })
}
