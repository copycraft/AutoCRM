//! RFC 3161 trusted timestamps: a time-stamping authority (TSA) signs a statement that a
//! given sha256 existed at a given moment. Only the hash leaves the building.
//!
//! The request is a few fixed DER structures, so it is built by hand here rather than
//! pulling in an ASN.1 stack. The answer is read just far enough to check it is the one we
//! asked for (status granted, our hash, our nonce) and to show its time and serial. The
//! signature is not verified here: that is what the stored response is for. Anyone can run
//! `openssl ts -verify -in photo.tsr -digest <sha256> -CAfile tsa.pem` against it, without
//! trusting this system.

use std::time::Duration;

use anyhow::{Context, anyhow, bail};
use chrono::{DateTime, NaiveDateTime, Utc};

use crate::config::TsaConfig;

/// id-sha256 (2.16.840.1.101.3.4.2.1).
const SHA256_OID: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01];
/// id-signedData (1.2.840.113549.1.7.2).
const SIGNED_DATA_OID: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x02];
/// id-ct-TSTInfo (1.2.840.113549.1.9.16.1.4).
const TST_INFO_OID: &[u8] = &[
    0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x09, 0x10, 0x01, 0x04,
];

const SEQUENCE: u8 = 0x30;
const SET: u8 = 0x31;
const INTEGER: u8 = 0x02;
const BOOLEAN: u8 = 0x01;
const OCTET_STRING: u8 = 0x04;
const NULL: u8 = 0x05;
const OID: u8 = 0x06;
const GENERALIZED_TIME: u8 = 0x18;
const CONTEXT_0: u8 = 0xa0;

/// What the authority answered, checked against what we asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    /// The whole TimeStampResp, DER: what `openssl ts -verify -in` reads.
    pub response: Vec<u8>,
    pub gen_time: DateTime<Utc>,
    /// The authority's serial number for this stamp, hex.
    pub serial: String,
}

fn der_len(len: usize) -> Vec<u8> {
    if len < 0x80 {
        return vec![len as u8];
    }
    let bytes: Vec<u8> = len
        .to_be_bytes()
        .into_iter()
        .skip_while(|b| *b == 0)
        .collect();
    let mut out = vec![0x80 | bytes.len() as u8];
    out.extend(bytes);
    out
}

fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    out.extend(der_len(content.len()));
    out.extend_from_slice(content);
    out
}

/// A non-negative INTEGER: minimal big-endian, with a leading zero when the top bit is set.
fn der_uint(value: u64) -> Vec<u8> {
    let mut bytes: Vec<u8> = value
        .to_be_bytes()
        .into_iter()
        .skip_while(|b| *b == 0)
        .collect();
    if bytes.is_empty() {
        bytes.push(0);
    }
    if bytes[0] & 0x80 != 0 {
        bytes.insert(0, 0);
    }
    tlv(INTEGER, &bytes)
}

fn message_imprint(sha256: &[u8]) -> Vec<u8> {
    let algorithm = tlv(SEQUENCE, &[tlv(OID, SHA256_OID), tlv(NULL, &[])].concat());
    tlv(SEQUENCE, &[algorithm, tlv(OCTET_STRING, sha256)].concat())
}

/// `TimeStampReq { version 1, messageImprint, nonce, certReq TRUE }`. Asking for the
/// certificate puts the signer's certificate in the answer, so it verifies on its own.
pub fn request_der(sha256: &[u8], nonce: u64) -> Vec<u8> {
    tlv(
        SEQUENCE,
        &[
            der_uint(1),
            message_imprint(sha256),
            der_uint(nonce),
            tlv(BOOLEAN, &[0xff]),
        ]
        .concat(),
    )
}

/// One DER element: (tag, content, what follows it).
fn read(input: &[u8]) -> anyhow::Result<(u8, &[u8], &[u8])> {
    let (&tag, rest) = input
        .split_first()
        .ok_or_else(|| anyhow!("truncated element"))?;
    let (&first, rest) = rest
        .split_first()
        .ok_or_else(|| anyhow!("truncated length"))?;
    let (len, rest) = if first < 0x80 {
        (first as usize, rest)
    } else if first == 0x80 {
        bail!("indefinite length is not DER");
    } else {
        let n = (first & 0x7f) as usize;
        if n > 4 || rest.len() < n {
            bail!("bad length");
        }
        let len = rest[..n]
            .iter()
            .fold(0usize, |acc, b| (acc << 8) | *b as usize);
        (len, &rest[n..])
    };
    if rest.len() < len {
        bail!("element runs past the end");
    }
    Ok((tag, &rest[..len], &rest[len..]))
}

/// The children of a constructed element, in order.
fn children(content: &[u8]) -> anyhow::Result<Vec<(u8, &[u8])>> {
    let mut out = Vec::new();
    let mut rest = content;
    while !rest.is_empty() {
        let (tag, value, next) = read(rest)?;
        out.push((tag, value));
        rest = next;
    }
    Ok(out)
}

fn expect<'a>(item: Option<&(u8, &'a [u8])>, tag: u8, what: &str) -> anyhow::Result<&'a [u8]> {
    match item {
        Some((t, v)) if *t == tag => Ok(v),
        Some((t, _)) => bail!("{what}: expected tag {tag:#04x}, found {t:#04x}"),
        None => bail!("{what}: missing"),
    }
}

fn uint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0u64, |acc, b| (acc << 8) | *b as u64)
}

fn generalized_time(bytes: &[u8]) -> anyhow::Result<DateTime<Utc>> {
    let text = std::str::from_utf8(bytes).context("genTime is not text")?;
    let text = text
        .strip_suffix('Z')
        .ok_or_else(|| anyhow!("genTime is not UTC: {text}"))?;
    let parsed = NaiveDateTime::parse_from_str(text, "%Y%m%d%H%M%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(text, "%Y%m%d%H%M%S"))
        .with_context(|| format!("genTime '{text}'"))?;
    Ok(parsed.and_utc())
}

/// Reads a TimeStampResp and checks it answers our request: granted, for our hash, with
/// our nonce. Returns the time and serial it certifies.
pub fn parse_response(
    der: &[u8],
    sha256: &[u8],
    nonce: u64,
) -> anyhow::Result<(DateTime<Utc>, String)> {
    let (tag, resp, _) = read(der)?;
    if tag != SEQUENCE {
        bail!("not a TimeStampResp");
    }
    let parts = children(resp)?;
    let status_info = children(expect(parts.first(), SEQUENCE, "status")?)?;
    let status = uint(expect(status_info.first(), INTEGER, "status code")?);
    // 0 granted, 1 granted with modifications; everything else is a refusal.
    if status > 1 {
        bail!("the time-stamping authority refused the request (status {status})");
    }
    let content_info = children(expect(parts.get(1), SEQUENCE, "timeStampToken")?)?;
    if expect(content_info.first(), OID, "content type")? != SIGNED_DATA_OID {
        bail!("timeStampToken is not SignedData");
    }
    let (tag, signed_data, _) = read(expect(content_info.get(1), CONTEXT_0, "content")?)?;
    if tag != SEQUENCE {
        bail!("SignedData is not a SEQUENCE");
    }
    let signed = children(signed_data)?;
    expect(signed.first(), INTEGER, "SignedData version")?;
    expect(signed.get(1), SET, "digestAlgorithms")?;
    let encap = children(expect(signed.get(2), SEQUENCE, "encapContentInfo")?)?;
    if expect(encap.first(), OID, "eContentType")? != TST_INFO_OID {
        bail!("the signed content is not a TSTInfo");
    }
    let (tag, tst_der, _) = read(expect(encap.get(1), CONTEXT_0, "eContent")?)?;
    if tag != OCTET_STRING {
        bail!("eContent is not an OCTET STRING");
    }
    let (tag, tst, _) = read(tst_der)?;
    if tag != SEQUENCE {
        bail!("TSTInfo is not a SEQUENCE");
    }
    let tst = children(tst)?;
    expect(tst.first(), INTEGER, "TSTInfo version")?;
    expect(tst.get(1), OID, "policy")?;
    let imprint = children(expect(tst.get(2), SEQUENCE, "messageImprint")?)?;
    let algorithm = children(expect(imprint.first(), SEQUENCE, "hashAlgorithm")?)?;
    if expect(algorithm.first(), OID, "hash algorithm")? != SHA256_OID {
        bail!("the stamp is not over a sha256");
    }
    if expect(imprint.get(1), OCTET_STRING, "hashedMessage")? != sha256 {
        bail!("the stamp is for a different hash");
    }
    let serial = hex::encode(expect(tst.get(3), INTEGER, "serialNumber")?);
    let gen_time = generalized_time(expect(tst.get(4), GENERALIZED_TIME, "genTime")?)?;
    // accuracy, ordering, nonce, tsa, extensions follow, all optional.
    let echoed = tst[5..]
        .iter()
        .find(|(tag, _)| *tag == INTEGER)
        .map(|(_, v)| uint(v));
    if echoed != Some(nonce) {
        bail!("the stamp does not carry our nonce");
    }
    Ok((gen_time, serial))
}

/// Asks the authority to stamp `sha256`.
pub async fn stamp(cfg: &TsaConfig, sha256: &[u8]) -> anyhow::Result<Stamp> {
    // Positive and below 2^63, so it reads back as the same unsigned value.
    let nonce = rand::random::<u64>() >> 1;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let mut request = client
        .post(&cfg.url)
        .header("Content-Type", "application/timestamp-query")
        .header("Accept", "application/timestamp-reply")
        .body(request_der(sha256, nonce));
    if let Some(user) = &cfg.username {
        request = request.basic_auth(user, cfg.password.as_deref());
    }
    let response = request
        .send()
        .await
        .with_context(|| format!("reaching the time-stamping authority at {}", cfg.url))?;
    let status = response.status();
    if !status.is_success() {
        bail!("the time-stamping authority answered {status}");
    }
    let body = response.bytes().await?.to_vec();
    let (gen_time, serial) = parse_response(&body, sha256, nonce)?;
    Ok(Stamp {
        response: body,
        gen_time,
        serial,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A response shaped like a real one, minus the parts this module never reads
    /// (certificates, signer infos).
    fn fake_response(hash: &[u8], nonce: u64, status: u64, time: &str) -> Vec<u8> {
        let tst_info = tlv(
            SEQUENCE,
            &[
                der_uint(1),
                tlv(OID, &[0x2b, 0x06, 0x01, 0x04, 0x01]),
                message_imprint(hash),
                der_uint(0x1234),
                tlv(GENERALIZED_TIME, time.as_bytes()),
                der_uint(nonce),
            ]
            .concat(),
        );
        let encap = tlv(
            SEQUENCE,
            &[
                tlv(OID, TST_INFO_OID),
                tlv(CONTEXT_0, &tlv(OCTET_STRING, &tst_info)),
            ]
            .concat(),
        );
        let signed_data = tlv(
            SEQUENCE,
            &[der_uint(3), tlv(SET, &[]), encap, tlv(SET, &[])].concat(),
        );
        let token = tlv(
            SEQUENCE,
            &[tlv(OID, SIGNED_DATA_OID), tlv(CONTEXT_0, &signed_data)].concat(),
        );
        tlv(
            SEQUENCE,
            &[tlv(SEQUENCE, &der_uint(status)), token].concat(),
        )
    }

    #[test]
    fn the_request_is_the_der_openssl_writes() {
        let hash = [0xabu8; 32];
        let der = request_der(&hash, 0x0102);
        // SEQUENCE { INTEGER 1, SEQUENCE { SEQUENCE { OID sha256, NULL }, OCTET STRING }, INTEGER, BOOLEAN }
        // 3 (version) + 51 (imprint) + 4 (nonce) + 3 (certReq) = 61 bytes of content.
        assert_eq!(&der[..5], &[0x30, 0x3d, 0x02, 0x01, 0x01]);
        assert_eq!(&der[5..9], &[0x30, 0x31, 0x30, 0x0d]);
        assert!(der.ends_with(&[0x02, 0x02, 0x01, 0x02, 0x01, 0x01, 0xff]));
    }

    #[test]
    fn integers_are_minimal_and_never_negative() {
        assert_eq!(der_uint(0), vec![0x02, 0x01, 0x00]);
        assert_eq!(der_uint(0x7f), vec![0x02, 0x01, 0x7f]);
        assert_eq!(der_uint(0x80), vec![0x02, 0x02, 0x00, 0x80]);
    }

    #[test]
    fn long_lengths_use_the_long_form() {
        assert_eq!(der_len(0x7f), vec![0x7f]);
        assert_eq!(der_len(0x80), vec![0x81, 0x80]);
        assert_eq!(der_len(0x1234), vec![0x82, 0x12, 0x34]);
        let big = tlv(OCTET_STRING, &[0u8; 300]);
        let (tag, value, rest) = read(&big).unwrap();
        assert_eq!((tag, value.len(), rest.len()), (OCTET_STRING, 300, 0));
    }

    #[test]
    fn a_granted_answer_for_our_hash_and_nonce_is_read() {
        let hash = [7u8; 32];
        let der = fake_response(&hash, 99, 0, "20261006153012.25Z");
        let (time, serial) = parse_response(&der, &hash, 99).unwrap();
        assert_eq!(time.to_rfc3339(), "2026-10-06T15:30:12.250+00:00");
        assert_eq!(serial, "1234");
        let whole_seconds = fake_response(&hash, 99, 1, "20261006153012Z");
        assert!(parse_response(&whole_seconds, &hash, 99).is_ok());
    }

    #[test]
    fn a_refusal_another_hash_or_another_nonce_is_an_error() {
        let hash = [7u8; 32];
        let refused = fake_response(&hash, 99, 2, "20261006153012Z");
        assert!(
            parse_response(&refused, &hash, 99)
                .unwrap_err()
                .to_string()
                .contains("refused")
        );
        let other = fake_response(&[8u8; 32], 99, 0, "20261006153012Z");
        assert!(
            parse_response(&other, &hash, 99)
                .unwrap_err()
                .to_string()
                .contains("different hash")
        );
        let replayed = fake_response(&hash, 98, 0, "20261006153012Z");
        assert!(
            parse_response(&replayed, &hash, 99)
                .unwrap_err()
                .to_string()
                .contains("nonce")
        );
    }
}
