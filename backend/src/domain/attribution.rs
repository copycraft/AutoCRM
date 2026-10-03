//! Where a website lead came from: sorting the tags the website sends (UTM parameters and the
//! referrer) into one channel, so a report groups on a single reliable value instead of a
//! dozen spellings of "facebook".

use serde::{Deserialize, Serialize};

/// The site's own domain: a visitor arriving from another page of it came from nowhere new.
const OWN_DOMAIN: &str = "autotherm.hu";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// Paid ads (UTM medium cpc, ppc, paid, display...).
    Paid,
    /// Found through a search engine, unpaid.
    Organic,
    Social,
    /// A newsletter or other email.
    Email,
    /// Followed a link from some other site.
    Referral,
    /// Typed the address, a bookmark, or nothing was sent.
    Direct,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Paid => "paid",
            Channel::Organic => "organic",
            Channel::Social => "social",
            Channel::Email => "email",
            Channel::Referral => "referral",
            Channel::Direct => "direct",
        }
    }
}

const PAID_MEDIUMS: &[&str] = &[
    "cpc",
    "ppc",
    "paid",
    "paidsearch",
    "paid-search",
    "paidsocial",
    "paid-social",
    "paid_social",
    "display",
    "cpm",
    "banner",
    "retargeting",
];
const EMAIL_MEDIUMS: &[&str] = &["email", "e-mail", "newsletter", "hirlevel"];
const SOCIAL_MEDIUMS: &[&str] = &["social", "social-network", "social_media", "sm"];
const SEARCH_HOSTS: &[&str] = &[
    "google.",
    "bing.",
    "duckduckgo.",
    "yahoo.",
    "ecosia.",
    "seznam.",
    "yandex.",
    "startpage.",
    "search.brave.",
];
const SOCIAL_HOSTS: &[&str] = &[
    "facebook.",
    "fb.com",
    "instagram.",
    "linkedin.",
    "tiktok.",
    "youtube.",
    "youtu.be",
    "twitter.",
    "t.co",
    "x.com",
    "pinterest.",
    "reddit.",
];

/// The lower-cased host of a referrer URL, without `www.`. None for blank, unparseable or
/// the site's own pages.
fn referrer_host(referrer: Option<&str>) -> Option<String> {
    let url = referrer?.trim();
    let after_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = after_scheme
        .split(['/', '?', '#', ':'])
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    if host.is_empty() || host == OWN_DOMAIN || host.ends_with(&format!(".{OWN_DOMAIN}")) {
        None
    } else {
        Some(host)
    }
}

fn matches_any(value: &str, list: &[&str]) -> bool {
    list.iter().any(|item| value.contains(item))
}

/// Sorts the tags into a channel. An explicit UTM medium wins over what the referrer
/// suggests, because a person tagged the link on purpose.
pub fn classify(
    utm_source: Option<&str>,
    utm_medium: Option<&str>,
    referrer: Option<&str>,
) -> Channel {
    let source = utm_source.unwrap_or("").trim().to_lowercase();
    let medium = utm_medium.unwrap_or("").trim().to_lowercase();
    let host = referrer_host(referrer);

    if PAID_MEDIUMS.contains(&medium.as_str()) || medium.contains("paid") {
        return Channel::Paid;
    }
    if EMAIL_MEDIUMS.contains(&medium.as_str()) || EMAIL_MEDIUMS.contains(&source.as_str()) {
        return Channel::Email;
    }
    if SOCIAL_MEDIUMS.contains(&medium.as_str()) || matches_any(&source, SOCIAL_HOSTS) {
        return Channel::Social;
    }
    if medium == "organic" {
        return Channel::Organic;
    }
    if let Some(host) = &host {
        if matches_any(host, SOCIAL_HOSTS) {
            return Channel::Social;
        }
        if matches_any(host, SEARCH_HOSTS) {
            return Channel::Organic;
        }
        return Channel::Referral;
    }
    // Tagged with a source but no medium and no referrer: someone shared a tagged link.
    if !source.is_empty() {
        return Channel::Referral;
    }
    Channel::Direct
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(source: Option<&str>, medium: Option<&str>, referrer: Option<&str>) -> Channel {
        classify(source, medium, referrer)
    }

    #[test]
    fn nothing_sent_is_direct() {
        assert_eq!(c(None, None, None), Channel::Direct);
        assert_eq!(c(Some(" "), Some(""), Some("")), Channel::Direct);
        // The site's own pages are not a source.
        assert_eq!(
            c(None, None, Some("https://www.autotherm.hu/szolgaltatasok")),
            Channel::Direct
        );
        assert_eq!(
            c(None, None, Some("https://shop.autotherm.hu/")),
            Channel::Direct
        );
    }

    #[test]
    fn a_tagged_medium_wins_over_the_referrer() {
        assert_eq!(
            c(Some("google"), Some("cpc"), Some("https://www.google.com/")),
            Channel::Paid
        );
        assert_eq!(c(Some("fb"), Some("paid_social"), None), Channel::Paid);
        assert_eq!(
            c(
                Some("hirlevel"),
                Some("email"),
                Some("https://mail.google.com/")
            ),
            Channel::Email
        );
        assert_eq!(c(Some("newsletter"), None, None), Channel::Email);
        assert_eq!(c(Some("facebook"), Some("social"), None), Channel::Social);
    }

    #[test]
    fn the_referrer_decides_when_nothing_is_tagged() {
        assert_eq!(
            c(
                None,
                None,
                Some("https://www.google.com/search?q=hutos+furgon")
            ),
            Channel::Organic
        );
        assert_eq!(
            c(None, None, Some("https://www.bing.com/")),
            Channel::Organic
        );
        assert_eq!(
            c(None, None, Some("https://l.facebook.com/l.php?u=x")),
            Channel::Social
        );
        assert_eq!(
            c(None, None, Some("https://www.instagram.com/")),
            Channel::Social
        );
        assert_eq!(
            c(None, None, Some("https://autoklub.example/partnerek")),
            Channel::Referral
        );
        assert_eq!(c(None, Some("organic"), None), Channel::Organic);
        assert_eq!(c(Some("partnerlista"), None, None), Channel::Referral);
    }

    #[test]
    fn hosts_are_read_from_messy_referrers() {
        assert_eq!(
            referrer_host(Some("HTTPS://WWW.Google.COM:443/x?y#z")),
            Some("google.com".into())
        );
        assert_eq!(
            referrer_host(Some("google.com/search")),
            Some("google.com".into())
        );
        assert_eq!(referrer_host(Some("   ")), None);
        assert_eq!(referrer_host(None), None);
    }
}
