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
    unknown.dedup();
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
pub fn text_to_html(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n");
    let mut html = String::from(
        "<!doctype html><html><body style=\"font-family:Arial,Helvetica,sans-serif;font-size:14px;line-height:1.5;color:#222\">\n",
    );
    for paragraph in normalized.split("\n\n") {
        let paragraph = paragraph.trim_matches('\n');
        if paragraph.trim().is_empty() {
            continue;
        }
        html.push_str("<p>");
        html.push_str(&escape_html(paragraph).replace('\n', "<br>\n"));
        html.push_str("</p>\n");
    }
    html.push_str("</body></html>");
    html
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
        assert!(html.contains("<p>Hello &lt;b&gt;Müller&lt;/b&gt; &amp; co<br>\nline two</p>"));
        assert!(html.contains("<p>second para</p>"));
    }

    #[test]
    fn subjects_are_single_line() {
        assert_eq!(single_line("a\r\nBcc: evil@x\nb"), "a Bcc: evil@x b");
    }
}
