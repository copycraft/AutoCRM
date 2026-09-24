//! Deliberately dumb email templates: `{{path}}` substitution from a fixed whitelist.
//! No expressions, no loops, no conditionals. If a template needs a condition, that's
//! two templates.
//!
//! Missing values render as a visible `{{MISSING:path}}` marker rather than an empty
//! string, and unknown paths as `{{UNKNOWN:path}}`, so a broken email says what broke.

use std::collections::BTreeMap;

/// Every variable a template may use, with a Hungarian description for the settings UI.
pub const VARIABLES: &[(&str, &str)] = &[
    ("order.number", "Projekt száma"),
    ("order.title", "Projekt megnevezése"),
    ("order.stage", "Projekt aktuális szakasza"),
    ("order.vehicle", "Jármű (gyártmány, típus, rendszám)"),
    ("order.plate", "Rendszám"),
    ("order.due_date", "Vállalt határidő"),
    ("order.days_in_stage", "Napok száma az aktuális szakaszban"),
    ("order.open_blockers", "Nyitott akadályok száma"),
    ("partner.name", "Partner neve"),
    ("contact.name", "Kapcsolattartó neve"),
    ("lead.title", "Érdeklődés tárgya"),
    ("lead.quoted_total", "Ajánlott ár (pénznemmel)"),
    ("lead.quote_valid_until", "Ajánlat érvényessége"),
    // Invoicing. Not reachable from an order id alone — an order can carry several
    // invoices — so these are resolved by the caller that knows which one the letter is
    // about, and merged over the rest.
    ("invoice.number", "Számla sorszáma"),
    ("invoice.original_number", "Sztornózott számla sorszáma"),
    ("invoice.issue_date", "Számla kelte"),
    ("invoice.payment_date", "Számla fizetési határideje"),
    ("invoice.total", "Számla végösszege"),
    ("invoice.payment_method", "Számla fizetési módja"),
    ("proforma.number", "Díjbekérő sorszáma"),
    ("proforma.payment_date", "Díjbekérő fizetési határideje"),
    ("proforma.total", "Díjbekérő végösszege"),
    ("blocker.what", "Mire várunk"),
    ("blocker.due_date", "Akadály határideje"),
    (
        "blocker.days_overdue",
        "Hány napja járt le az akadály határideje",
    ),
    ("user.name", "Küldő munkatárs neve"),
];

pub fn is_known_variable(path: &str) -> bool {
    VARIABLES.iter().any(|(name, _)| *name == path)
}

/// Values available for one rendering. Keys must be whitelisted paths.
pub type TemplateValues = BTreeMap<&'static str, String>;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment<'a> {
    Text(&'a str),
    Variable(&'a str),
}

fn is_variable_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.')
}

fn parse(src: &str) -> Vec<Segment<'_>> {
    let mut segments = Vec::new();
    let mut rest = src;
    while let Some(open) = rest.find("{{") {
        let after_open = &rest[open + 2..];
        let Some(close) = after_open.find("}}") else {
            break;
        };
        let name = after_open[..close].trim();
        if is_variable_name(name) {
            if open > 0 {
                segments.push(Segment::Text(&rest[..open]));
            }
            segments.push(Segment::Variable(name));
            rest = &after_open[close + 2..];
        } else {
            // Not a variable (e.g. "{{MISSING:x}}" or "{{ a b }}"): keep the braces literally.
            segments.push(Segment::Text(&rest[..open + 2]));
            rest = after_open;
        }
    }
    if !rest.is_empty() {
        segments.push(Segment::Text(rest));
    }
    segments
}

/// Variables referenced by a template that are not on the whitelist. Used to reject
/// template edits before they can break a send.
pub fn unknown_variables(src: &str) -> Vec<String> {
    let mut unknown: Vec<String> = parse(src)
        .into_iter()
        .filter_map(|s| match s {
            Segment::Variable(name) if !is_known_variable(name) => Some(name.to_string()),
            _ => None,
        })
        .collect();
    // Each name once, in first-seen order (`dedup` alone only drops adjacent repeats).
    let mut seen = std::collections::BTreeSet::new();
    unknown.retain(|name| seen.insert(name.clone()));
    unknown
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub output: String,
    /// Paths that rendered as MISSING or UNKNOWN markers. Non-empty means automatic
    /// mail must not go out.
    pub unresolved: Vec<String>,
}

pub fn render(src: &str, values: &TemplateValues) -> Rendered {
    let mut output = String::with_capacity(src.len() + 64);
    let mut unresolved = Vec::new();
    for segment in parse(src) {
        match segment {
            Segment::Text(t) => output.push_str(t),
            Segment::Variable(name) if !is_known_variable(name) => {
                output.push_str(&format!("{{{{UNKNOWN:{name}}}}}"));
                unresolved.push(name.to_string());
            }
            Segment::Variable(name) => match values.get(name).map(|v| v.trim()) {
                Some(v) if !v.is_empty() => output.push_str(v),
                _ => {
                    output.push_str(&format!("{{{{MISSING:{name}}}}}"));
                    unresolved.push(name.to_string());
                }
            },
        }
    }
    Rendered { output, unresolved }
}

/// Subjects are single-line: any line breaks (from template or values) become spaces.
pub fn single_line(s: &str) -> String {
    s.split(['\r', '\n'])
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// The HTML part is derived from the rendered plain text: blank lines separate paragraphs,
/// single newlines become <br>. Everything is escaped, values included.
///
/// The paragraphs sit in a branded, email-client-safe layout: tables and inline styles only
/// (no <style> blocks, no external assets — those get stripped or blocked). The document
/// still ends with `</body></html>` and the content starts right after a `<body ...>` tag,
/// because the redirect banner and the download links are spliced into those spots later.
pub fn text_to_html(text: &str) -> String {
    email_html(text)
}

/// The branded wrapper around already-escaped paragraph HTML. One place, so every letter —
/// automatic or manual, invoice or nudge — looks like it came from the same office.
pub fn email_html(text: &str) -> String {
    layout(None, &paragraphs(text))
}

/// Same, with a shout across the top: the quotation letter's "MEGJÖTT AZ ÁRAJÁNLATOD".
/// Reserved for the one letter per relationship that deserves it; everything shouting
/// means nothing is heard.
pub fn email_html_hero(hero: &str, text: &str) -> String {
    layout(Some(hero), &paragraphs(text))
}

/// The body written in Markdown, rendered to HTML and wrapped in the same layout.
/// Substitution happens before parsing: `{{variables}}` render first, then Markdown.
/// Authors are staff, so inline HTML passes through (pulldown-cmark keeps it); the inbox
/// renders stored HTML sandboxed, and mail clients strip what they do not like.
pub fn markdown_to_html(md: &str) -> String {
    layout(None, &markdown_fragment(md))
}

/// Markdown with the hero band, for the quotation letter when the office writes that one
/// by hand too.
pub fn markdown_to_html_hero(hero: &str, md: &str) -> String {
    layout(Some(hero), &markdown_fragment(md))
}

fn paragraphs(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n");
    let mut body = String::new();
    for paragraph in normalized.split("\n\n") {
        let paragraph = paragraph.trim_matches('\n');
        if paragraph.trim().is_empty() {
            continue;
        }
        body.push_str(
            "<p style=\"margin:0 0 12px 0;font-size:14px;line-height:1.6;color:#292524;\">",
        );
        body.push_str(&escape_html(paragraph).replace('\n', "<br>\n"));
        body.push_str("</p>\n");
    }
    body
}

fn markdown_fragment(md: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(md, options);
    let mut fragment = String::new();
    html::push_html(&mut fragment, parser);
    // pulldown-cmark emits bare tags; the layout carries the type, so scope its
    // descendants instead of rewriting every tag inline.
    fragment
}

/// Inline images: `doc:ID` references resolved to embedded attachments.
///
/// The author writes `![alt](doc:42)` in Markdown (or `src="doc:42"` in rare hand HTML)
/// and lists 42 among the embedded documents. This rewrites the HTML reference to
/// `cid:doc-42` — the Content-ID the attachment is sent with — and the text part to the
/// bare filename, so the plain-text letter does not quote a pointer nobody can follow.
/// A reference without a matching embedded document is returned, and the caller refuses
/// the send naming it: a red X on the customer's screen is worse than no picture.
///
/// `embeds` is `(document id, filename)` pairs; filenames are escaped on the way in.
pub fn resolve_embed_refs(
    body_html: &str,
    body_text: &str,
    embeds: &[(i64, String)],
) -> Result<(String, String), Vec<i64>> {
    use std::collections::{BTreeMap, BTreeSet};
    let by_id: BTreeMap<i64, &str> = embeds.iter().map(|(id, name)| (*id, name.as_str())).collect();

    let mut referenced = BTreeSet::new();
    let mut rest = body_html;
    while let Some(start) = rest.find("doc:") {
        let digits: String = rest[start + 4..].chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(id) = digits.parse::<i64>() {
            referenced.insert(id);
        }
        rest = &rest[start + 4..];
    }
    let unknown: Vec<i64> = referenced
        .into_iter()
        .filter(|id| !by_id.contains_key(id))
        .collect();
    if !unknown.is_empty() {
        return Err(unknown);
    }

    let mut html = body_html.to_string();
    let mut text = body_text.to_string();
    for (id, name) in &by_id {
        let from = format!("doc:{id}");
        html = html.replace(&format!("\"{from}\""), &format!("\"cid:doc-{id}\""));
        html = html.replace(&format!("'{from}'"), &format!("'cid:doc-{id}'"));
        let safe_name = escape_html(name);
        // Markdown image first (`![alt](doc:42)` → the filename), then any bare pointer.
        let mut rebuilt = String::new();
        let mut rest = text.as_str();
        while let Some(open) = rest.find("![") {
            if let Some(close) = rest[open..].find(&format!("]({from})")) {
                rebuilt.push_str(&rest[..open]);
                rebuilt.push_str(&safe_name);
                rest = &rest[open + close + 3 + from.len()..];
            } else {
                rebuilt.push_str(&rest[..open + 2]);
                rest = &rest[open + 2..];
            }
        }
        rebuilt.push_str(rest);
        text = rebuilt.replace(&from, &safe_name);
    }
    Ok((html, text))
}

fn layout(hero: Option<&str>, content: &str) -> String {
    let hero_row = match hero {
        Some(title) => format!(
            "<tr><td style=\"background-color:#b91c1c;padding:26px 28px;text-align:center;\">\n\
             <div style=\"font-family:Arial,Helvetica,sans-serif;font-size:24px;font-weight:bold;\
             letter-spacing:1px;color:#ffffff;\">{}</div>\n\
             </td></tr>",
            escape_html(title)
        ),
        None => String::new(),
    };
    format!(
        "<!doctype html><html lang=\"hu\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"></head>\n\
<body style=\"margin:0;padding:0;background-color:#f0eeea;\">\n\
<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" \
style=\"background-color:#f0eeea;padding:24px 12px;\"><tr><td align=\"center\">\n\
<table role=\"presentation\" width=\"600\" cellpadding=\"0\" cellspacing=\"0\" \
style=\"width:100%;max-width:600px;background-color:#ffffff;border-radius:10px;overflow:hidden;\
border:1px solid #e2e0da;\"><tr><td \
style=\"background-color:#1c1917;padding:20px 28px;\">\n\
<div style=\"font-family:Arial,Helvetica,sans-serif;font-size:20px;font-weight:bold;\
letter-spacing:3px;color:#ffffff;\">AUTOTHERM</div>\n\
<div style=\"font-family:Arial,Helvetica,sans-serif;font-size:12px;color:#d6d0c7;\
margin-top:4px;\">Hűtős felépítmények &middot; automata értesítés</div>\n\
</td></tr>{hero_row}<tr><td class=\"autotherm-body\" \
style=\"padding:24px 28px;font-family:Arial,Helvetica,sans-serif;font-size:14px;\
line-height:1.6;color:#292524;\">\n\
{content}</td></tr><tr><td \
style=\"background-color:#f7f6f3;padding:14px 28px;border-top:1px solid #e2e0da;\">\n\
<div style=\"font-family:Arial,Helvetica,sans-serif;font-size:11px;line-height:1.5;\
color:#8a847a;\">Ezt a levelet az Autotherm CRM küldte automatikusan. \
Kérdés esetén válaszoljon erre a levélre — megkeresését munkatársunk olvassa.</div>\n\
</td></tr></table>\n\
<div style=\"font-family:Arial,Helvetica,sans-serif;font-size:11px;color:#a8a29e;\
margin-top:12px;\">Autotherm Kft.</div>\n\
</td></tr></table>\n\
</body></html>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(pairs: &[(&'static str, &str)]) -> TemplateValues {
        pairs.iter().map(|(k, v)| (*k, v.to_string())).collect()
    }

    #[test]
    fn substitutes_known_variables() {
        let r = render(
            "Várjuk: {{blocker.what}} ({{ order.number }})",
            &values(&[
                ("blocker.what", "ATP tanúsítvány"),
                ("order.number", "2026-0042"),
            ]),
        );
        assert_eq!(r.output, "Várjuk: ATP tanúsítvány (2026-0042)");
        assert!(r.unresolved.is_empty());
    }

    #[test]
    fn missing_and_empty_values_are_visible() {
        let r = render(
            "waiting for {{blocker.what}} on {{order.number}}",
            &values(&[("order.number", "  ")]),
        );
        assert_eq!(
            r.output,
            "waiting for {{MISSING:blocker.what}} on {{MISSING:order.number}}"
        );
        assert_eq!(r.unresolved, vec!["blocker.what", "order.number"]);
    }

    #[test]
    fn unknown_variables_are_flagged_not_evaluated() {
        let r = render("{{order.secret_margin}}", &values(&[]));
        assert_eq!(r.output, "{{UNKNOWN:order.secret_margin}}");
        assert_eq!(
            unknown_variables("{{order.number}} {{nope}} {{nope}}"),
            vec!["nope"]
        );
    }

    #[test]
    fn unknown_variables_are_listed_once_each() {
        assert_eq!(
            unknown_variables("{{nope}} {{order.number}} {{other}} {{nope}}"),
            vec!["nope", "other"]
        );
    }

    #[test]
    fn non_variable_braces_are_literal() {
        let r = render("a {{ not valid }} b {{ unterminated", &values(&[]));
        assert_eq!(r.output, "a {{ not valid }} b {{ unterminated");
        assert!(r.unresolved.is_empty());
    }

    #[test]
    fn values_are_not_re_rendered() {
        let r = render(
            "{{partner.name}}",
            &values(&[("partner.name", "{{order.number}}")]),
        );
        assert_eq!(r.output, "{{order.number}}");
    }

    #[test]
    fn html_part_escapes_and_paragraphs() {
        let html = text_to_html("Hello <b>Müller</b> & co\nline two\n\n\nsecond para");
        assert!(html.contains("Hello &lt;b&gt;Müller&lt;/b&gt; &amp; co<br>\nline two"));
        assert!(html.contains("second para"));
        assert!(html.contains("AUTOTHERM"));
        assert!(html.ends_with("</body></html>"));
    }

    #[test]
    fn markdown_renders_inside_the_same_layout() {
        let html = markdown_to_html("# Árajánlat\n\nKedves **János**!\n\n- tétel 1\n- tétel 2\n");
        assert!(html.contains("<h1>Árajánlat</h1>"), "{html}");
        assert!(html.contains("<strong>János</strong>"), "{html}");
        assert!(html.contains("<li>tétel 1</li>"), "{html}");
        assert!(html.contains("AUTOTHERM"));
        assert!(html.ends_with("</body></html>"));
    }

    #[test]
    fn markdown_keeps_staff_inline_html() {
        // Authors are authenticated staff; inline HTML passes through by design, and the
        // inbox renders stored HTML sandboxed. Scripts never execute there.
        let html = markdown_to_html("szép <b>kiemelés</b> vége");
        assert!(html.contains("szép <b>kiemelés</b> vége"), "{html}");
    }

    #[test]
    fn hero_band_shouts_once() {
        let html = email_html_hero("MEGJÖTT AZ ÁRAJÁNLATOD", "Kedves János!");
        assert!(html.contains("MEGJÖTT AZ ÁRAJÁNLATOD"), "{html}");
        assert!(html.contains("background-color:#b91c1c"), "{html}");
        assert!(html.contains("Kedves János!"), "{html}");
    }

    #[test]
    fn embed_refs_resolve_to_cid_and_filename() {
        let embeds = vec![(42, "arlista.png".to_string())];
        let (html, text) = resolve_embed_refs(
            "<p>Nézd:<img src=\"doc:42\" alt=\"ár\"></p>",
            "Nézd: ![ár](doc:42) és doc:42",
            &embeds,
        )
        .unwrap();
        assert!(html.contains("src=\"cid:doc-42\""), "{html}");
        assert!(!html.contains("doc:42"), "{html}");
        assert!(text.contains("arlista.png"), "{text}");
        assert!(!text.contains("doc:42"), "{text}");
    }

    #[test]
    fn embed_refs_to_unknown_ids_are_reported() {
        let err = resolve_embed_refs("x doc:7 y", "x", &[]).unwrap_err();
        assert_eq!(err, vec![7]);
    }

    #[test]
    fn subjects_are_single_line() {
        assert_eq!(single_line("a\r\nBcc: evil@x\nb"), "a Bcc: evil@x b");
    }
}
