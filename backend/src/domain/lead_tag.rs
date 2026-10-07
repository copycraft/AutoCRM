//! Lead tags: which website domains a tag claims, and which tags a lead arriving with a
//! handful of URLs and hosts should get.

/// The bare host a tag stores for a domain the office typed: `https://www.HutoAutok.hu/x`
/// becomes `hutoautok.hu`. None when nothing host-like is left (no dot, or odd characters).
pub fn normalize_domain(input: &str) -> Option<String> {
    let s = input.trim();
    let after_scheme = s.split_once("://").map_or(s, |(_, rest)| rest);
    // user@host is a URL form too, and an email address someone pasted.
    let after_user = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('@')
        .next()
        .unwrap_or("");
    let host = after_user
        .split(':')
        .next()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    let valid = host.contains('.')
        && host.len() <= 253
        && !host.starts_with(['.', '-'])
        && !host.contains("..")
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || !c.is_ascii());
    valid.then(|| host.to_string())
}

/// Whether `host` is `domain` or one of its subdomains: `shop.hutoautok.hu` is
/// `hutoautok.hu`, `nothutoautok.hu` is not.
pub fn host_matches(host: &str, domain: &str) -> bool {
    host == domain
        || host
            .strip_suffix(domain)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

/// The tag's domain that one of `hosts` falls under, if any. `hosts` are already
/// normalised; the first host that matches wins, so callers list the strongest evidence
/// (the site the form is on) first.
pub fn first_match<'a>(hosts: &[String], domains: &'a [String]) -> Option<&'a str> {
    hosts.iter().find_map(|h| {
        domains
            .iter()
            .find(|d| host_matches(h, d))
            .map(String::as_str)
    })
}

/// Every distinct host among free-form values (URLs, bare hosts, a UTM source), in order.
/// Values with no host in them (`google`, `/szolgaltatasok`) are skipped.
pub fn hosts_of<'a>(values: impl IntoIterator<Item = Option<&'a str>>) -> Vec<String> {
    let mut hosts: Vec<String> = Vec::new();
    for host in values.into_iter().flatten().filter_map(normalize_domain) {
        if !hosts.contains(&host) {
            hosts.push(host);
        }
    }
    hosts
}

/// A market code: two lower-case letters (hu, ro, de, it).
pub fn valid_market(market: &str) -> bool {
    market.len() == 2 && market.chars().all(|c| c.is_ascii_lowercase())
}

/// A colour as `#rrggbb`, lower-cased. None for anything else.
pub fn normalize_color(color: &str) -> Option<String> {
    let c = color.trim().to_lowercase();
    let hex = c.strip_prefix('#')?;
    (hex.len() == 6 && hex.chars().all(|ch| ch.is_ascii_hexdigit())).then_some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domains_are_reduced_to_the_bare_host() {
        assert_eq!(
            normalize_domain("hutoautok.hu").as_deref(),
            Some("hutoautok.hu")
        );
        assert_eq!(
            normalize_domain(" https://www.HutoAutok.hu/ajanlat?x=1 ").as_deref(),
            Some("hutoautok.hu")
        );
        assert_eq!(
            normalize_domain("http://shop.example.at:8080").as_deref(),
            Some("shop.example.at")
        );
        assert_eq!(
            normalize_domain("info@bestattungswagen.at").as_deref(),
            Some("bestattungswagen.at")
        );
        assert_eq!(
            normalize_domain("HUTOAUTOK.HU.").as_deref(),
            Some("hutoautok.hu")
        );
    }

    #[test]
    fn values_without_a_host_are_not_domains() {
        assert_eq!(normalize_domain(""), None);
        assert_eq!(normalize_domain("google"), None);
        assert_eq!(normalize_domain("/szolgaltatasok/hutokamra"), None);
        assert_eq!(normalize_domain("not a domain.hu"), None);
        assert_eq!(normalize_domain(".hu"), None);
    }

    #[test]
    fn subdomains_match_but_lookalikes_do_not() {
        assert!(host_matches("hutoautok.hu", "hutoautok.hu"));
        assert!(host_matches("shop.hutoautok.hu", "hutoautok.hu"));
        assert!(!host_matches("nothutoautok.hu", "hutoautok.hu"));
        assert!(!host_matches("hutoautok.hu.evil.com", "hutoautok.hu"));
    }

    #[test]
    fn the_first_host_that_matches_wins() {
        let hosts = hosts_of([
            Some("https://www.furgonifunebri.it/contatti"),
            None,
            Some("google"),
            Some("https://www.google.com/"),
            Some("furgonifunebri.it"),
        ]);
        assert_eq!(hosts, vec!["furgonifunebri.it", "google.com"]);
        let domains = vec!["furgonifunebri.it".to_string()];
        assert_eq!(first_match(&hosts, &domains), Some("furgonifunebri.it"));
        assert_eq!(first_match(&hosts[1..], &domains), None);
    }

    #[test]
    fn markets_and_colours() {
        assert!(valid_market("hu"));
        assert!(!valid_market("HU"));
        assert!(!valid_market("hun"));
        assert_eq!(normalize_color(" #A33122 ").as_deref(), Some("#a33122"));
        assert_eq!(normalize_color("a33122"), None);
        assert_eq!(normalize_color("#a3312"), None);
    }
}
