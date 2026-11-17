//! Conservative matching of imported point claims to one independently verified excerpt.
use super::Rates;
use super::catalog::{Band, Tier};
use super::proposal::{Kind, Operation, Proposal, Replacement, Target, Value};

pub(super) fn supports_operation(op: &Operation, proposal: &Proposal) -> bool {
    if let (Target::Proxy { proxy }, kind, replacement, expected) =
        (&op.target, &op.kind, &op.replacement, &op.expected)
    {
        let entries = match (kind, replacement, expected) {
            (Kind::Add | Kind::ReplaceValue, Some(Replacement::Proxy { points }), _) => points,
            (Kind::Remove, None, Value::Proxy(points)) => points,
            _ => return false,
        };
        return !entries.is_empty()
            && entries.iter().all(|entry| {
                op.evidence_ids
                    .iter()
                    .filter_map(|id| proposal.evidence.iter().find(|e| &e.id == id))
                    .any(|e| {
                        html_to_text(&e.excerpt).lines().any(|line| {
                            field(line, "proxy") == Some(proxy.as_str())
                                && field(line, "target") == Some(entry.target.as_str())
                                && field(line, "effective date") == entry.effective_from.as_deref()
                        })
                    })
            });
    }
    op.evidence_ids
        .iter()
        .filter_map(|id| proposal.evidence.iter().find(|e| &e.id == id))
        .any(|e| {
            let text = html_to_text(&e.excerpt);
            match (&op.target, &op.kind, &op.replacement, &op.expected) {
                (
                    Target::Point { .. },
                    Kind::Add | Kind::ReplaceValue | Kind::Move,
                    Some(Replacement::Point { key, rates }),
                    _,
                ) => {
                    let lower = text.to_ascii_lowercase();
                    let unit = lower
                        .find("per 1m tokens")
                        .or_else(|| lower.find("per million tokens"));
                    if !matches!((unit, text.find(&key.model)), (Some(u), Some(m)) if u < m) {
                        return false;
                    }
                    table_supports(&text, &key.model, key.tier, key.context, *rates)
                        || prose_supports(&text, &key.model, key.tier, key.context, *rates)
                }
                (
                    Target::Point { key }
                    | Target::OverlayPoint { key }
                    | Target::Tombstone { key },
                    Kind::Remove,
                    None,
                    _,
                ) => {
                    table_supports(&text, &key.model, key.tier, key.context, Rates::default())
                        || prose_supports(
                            &text,
                            &key.model,
                            key.tier,
                            key.context,
                            Rates::default(),
                        )
                }
                (Target::Threshold { model }, kind, replacement, expected) => {
                    text.lines().any(|line| {
                        if field(line, "model") != Some(model.as_str()) {
                            return false;
                        }
                        match (kind, replacement, expected) {
                            (
                                Kind::Add | Kind::ReplaceValue,
                                Some(Replacement::Threshold { threshold }),
                                _,
                            ) => {
                                field(line, "threshold").and_then(|v| v.parse::<u64>().ok())
                                    == Some(*threshold)
                            }
                            (Kind::Remove, None, Value::Threshold(_)) => field(line, "threshold")
                                .and_then(|v| v.parse::<u64>().ok())
                                .is_some(),
                            _ => false,
                        }
                    })
                }
                _ => false,
            }
        })
}

fn tier_name(tier: Tier) -> &'static str {
    match tier {
        Tier::Standard => "standard",
        Tier::Fast => "fast",
        Tier::Ultrafast => "ultrafast",
    }
}
fn context_name(band: Band) -> &'static str {
    match band {
        Band::Short => "short",
        Band::Long => "long",
    }
}

// HTML is converted conservatively: cells and row/heading boundaries stay explicit;
// inline tags become spaces, never concatenating text into a new model or number.
fn html_to_text(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    if lower.contains("<!--") || lower.contains("<script") || lower.contains("<style") {
        return String::new();
    }
    let mut result = String::new();
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c != '<' {
            result.push(c);
            continue;
        }
        let mut tag = String::new();
        let mut closed = false;
        for c in chars.by_ref() {
            if c == '>' {
                closed = true;
                break;
            }
            tag.push(c);
        }
        if !closed {
            return String::new();
        }
        let name = tag
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        match name.as_str() {
            "tr" | "/tr" | "/h1" | "/h2" | "/h3" | "p" | "/p" => result.push('\n'),
            "h1" | "h2" | "h3" => result.push_str("\n# "),
            "td" | "th" => (),
            "/td" | "/th" => result.push('|'),
            _ => result.push(' '),
        }
    }
    result.replace("&dollar;", "$").replace("&nbsp;", " ")
}
fn cells(line: &str) -> Vec<&str> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = line.strip_suffix('|').unwrap_or(line);
    line.split('|').map(str::trim).collect()
}
fn heading(line: &str) -> Option<Tier> {
    let s = line
        .trim()
        .trim_start_matches('#')
        .trim()
        .to_ascii_lowercase();
    match s.as_str() {
        "standard" | "standard pricing" | "standard pricing data" => Some(Tier::Standard),
        "fast" | "fast pricing" | "fast pricing data" => Some(Tier::Fast),
        "ultrafast" | "ultrafast pricing" | "ultrafast pricing data" => Some(Tier::Ultrafast),
        _ => None,
    }
}
fn component_header(header: &str, context: Band, component: &str) -> bool {
    let h = header.to_ascii_lowercase();
    let words: Vec<_> = h
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|x| !x.is_empty())
        .collect();
    if !words.contains(&context_name(context)) {
        return false;
    }
    match component {
        "input" => {
            words.contains(&"input")
                && !words.contains(&"cached")
                && !words.contains(&"cache")
                && !words.contains(&"write")
        }
        "cached" => {
            words.contains(&"input")
                && (words.contains(&"cached") || words.contains(&"cache"))
                && !words.contains(&"write")
        }
        "write" => {
            words.contains(&"write")
                && (words.contains(&"30m") || (words.contains(&"30") && words.contains(&"minute")))
        }
        "output" => words.contains(&"output"),
        _ => false,
    }
}
fn money(cell: &str) -> Option<f64> {
    let s = cell.trim().strip_prefix('$')?.trim();
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    s.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0)
}
fn table_supports(text: &str, model: &str, tier: Tier, context: Band, rates: Rates) -> bool {
    let mut current_tier = None;
    let mut header: Option<Vec<String>> = None;
    let mut matches = 0;
    let mut supported = false;
    for line in text.lines() {
        if let Some(t) = heading(line) {
            current_tier = Some(t);
            header = None;
            continue;
        }
        // Any other Markdown heading closes the preceding section.
        if line.trim_start().starts_with('#') {
            current_tier = None;
            header = None;
            continue;
        }
        let row = cells(line);
        if row.len() < 2 {
            continue;
        }
        if row[0].eq_ignore_ascii_case("model") {
            header = Some(row.iter().map(|s| s.to_string()).collect());
            continue;
        }
        if current_tier != Some(tier) || row[0] != model {
            continue;
        }
        matches += 1;
        let Some(h) = &header else {
            continue;
        };
        if h.len() != row.len()
            || !h.iter().any(|col| {
                component_header(col, context, "input") || component_header(col, context, "output")
            })
        {
            continue;
        }
        let requirements = [
            ("input", rates.input),
            ("cached", rates.cached_input),
            ("write", rates.cache_write_input),
            ("output", rates.output),
        ];
        if requirements.iter().all(|(component, rate)| match rate {
            None => true,
            Some(expected) => {
                let indexes: Vec<_> = h
                    .iter()
                    .enumerate()
                    .filter(|(_, col)| component_header(col, context, component))
                    .map(|(i, _)| i)
                    .collect();
                indexes.len() == 1 && money(row[indexes[0]]) == Some(*expected)
            }
        }) {
            supported = true;
        }
    }
    matches == 1 && supported
}
fn field<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let mut fields = line
        .split(';')
        .filter_map(|p| p.trim().split_once(':'))
        .filter(|(k, _)| k.trim().eq_ignore_ascii_case(name))
        .map(|(_, v)| v.trim());
    let first = fields.next()?;
    if fields.next().is_some() {
        None
    } else {
        Some(first)
    }
}
fn prose_supports(text: &str, model: &str, tier: Tier, context: Band, rates: Rates) -> bool {
    // One semicolon-separated record, with explicit labels, avoids binding facts
    // across paragraphs, tiers, models or contexts.
    let mut matches = 0;
    let mut supported = false;
    for line in text.lines() {
        let fields: Vec<_> = line
            .split(';')
            .filter_map(|part| part.trim().split_once(':'))
            .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim()))
            .collect();
        let get = |name: &str| -> Option<&str> {
            let mut found = fields.iter().filter(|(k, _)| k == name).map(|(_, v)| *v);
            let v = found.next()?;
            if found.next().is_some() {
                None
            } else {
                Some(v)
            }
        };
        if get("model") != Some(model)
            || get("tier").map(str::to_ascii_lowercase).as_deref() != Some(tier_name(tier))
            || get("context").map(str::to_ascii_lowercase).as_deref() != Some(context_name(context))
        {
            continue;
        }
        matches += 1;
        let requirements = [
            ("input", rates.input),
            ("cached input", rates.cached_input),
            ("30m write", rates.cache_write_input),
            ("output", rates.output),
        ];
        if requirements
            .iter()
            .all(|(name, rate)| rate.is_none_or(|v| get(name).and_then(money) == Some(v)))
        {
            supported = true;
        }
    }
    matches == 1 && supported
}

#[cfg(test)]
mod tests {
    use super::super::catalog::PointKey;
    use super::super::proposal::{Kind, Value};
    use super::*;
    use crate::pricing_evidence::Evidence;
    fn claim(excerpt: &str) -> (Operation, Proposal) {
        let key = PointKey {
            model: "gpt-5".into(),
            tier: Tier::Fast,
            context: Band::Short,
            effective_from: "2026-01-01".into(),
        };
        let op = Operation {
            id: "op".into(),
            group: "op".into(),
            kind: Kind::Add,
            target: Target::Point { key: key.clone() },
            expected: Value::Absent,
            replacement: Some(Replacement::Point {
                key,
                rates: Rates {
                    input: Some(1.0),
                    cached_input: Some(0.1),
                    cache_write_input: Some(1.25),
                    output: Some(2.0),
                },
            }),
            evidence_ids: vec!["e".into()],
            date_basis: "researched".into(),
        };
        let proposal = Proposal {
            proposal_schema_version: 1,
            catalog_schema_version: 1,
            operations: vec![],
            evidence: vec![Evidence {
                id: "e".into(),
                url: "https://example.invalid".into(),
                retrieved_at: "2026-01-01T00:00:00Z".into(),
                excerpt: excerpt.into(),
            }],
            coverage: vec![],
        };
        (op, proposal)
    }
    const TABLE: &str = "Prices per 1M tokens. Effective 2026-01-01\n### Fast pricing data\n| Model | Short context input | Short context output | Short context cached input | Short context 30m write | Long context input |\n| gpt-5 | $1 | $2 | $0.1 | $1.25 | $8 |";
    #[test]
    fn binds_rate_components_to_exact_model_tier_and_context() {
        let (op, p) = claim(TABLE);
        assert!(supports_operation(&op, &p));
        for bad in [
            TABLE.replace("gpt-5 |", "gpt-5.5 |"),
            TABLE.replace("### Fast", "### Ultrafast"),
            TABLE.replace("Short context", "Long context"),
            TABLE.replace("$1 | $2", "$2 | $1"),
            format!("{TABLE}\n| gpt-5 | $9 | $2 | $0.1 | $1.25 | $8 |"),
        ] {
            let (op, p) = claim(&bad);
            assert!(!supports_operation(&op, &p), "{bad}");
        }
    }
    #[test]
    fn accepts_explicit_prose_and_html_without_combining_claims() {
        let prose = "Per 1M tokens, effective 2026-01-01\nModel: gpt-5; Tier: fast; Context: short; Input: $1; Cached input: $0.1; 30m write: $1.25; Output: $2";
        let (op, p) = claim(prose);
        assert!(supports_operation(&op, &p));
        let html = "<p>Prices per 1M tokens. Effective 2026-01-01</p><h3>Fast pricing</h3><tr><th>Model</th><th>Short context input</th><th>Short context output</th><th>Short context cached input</th><th>Short context 30m write</th></tr><tr><td>gpt-5</td><td>$1</td><td>$2</td><td>$0.1</td><td>$1.25</td></tr>";
        let (op, p) = claim(html);
        assert!(supports_operation(&op, &p));
        let (mut op, mut p) = claim(
            "Prices per 1M tokens effective 2026-01-01\n### Fast\n| Model | Short context input |\n| gpt-5 | $1 |",
        );
        let mut extra = p.evidence[0].clone();
        extra.id = "other".into();
        extra.excerpt="Model: gpt-5; Tier: fast; Context: short; Cached input: $0.1; 30m write: $1.25; Output: $2".into();
        p.evidence.push(extra);
        op.evidence_ids.push("other".into());
        assert!(!supports_operation(&op, &p));
    }
    #[test]
    fn rejects_hidden_unrelated_sections_and_missing_cells() {
        let hidden = format!("<!-- {TABLE} -->");
        let (op, p) = claim(&hidden);
        assert!(!supports_operation(&op, &p));
        let misleading = TABLE.replace("| gpt-5 |", "<h3>Batch</h3>\n| gpt-5 |");
        let (op, p) = claim(&misleading);
        assert!(!supports_operation(&op, &p));
        let missing = TABLE.replace("| $1 | $2 | $0.1 |", "| | $1 | $2 | $0.1 |");
        let (op, p) = claim(&missing);
        assert!(!supports_operation(&op, &p));
        let (mut op, mut p) = claim(&TABLE.replace("Prices per 1M tokens.", "Pricing"));
        let mut unit = p.evidence[0].clone();
        unit.id = "unit".into();
        unit.excerpt = "<!-- Prices per 1M tokens -->".into();
        p.evidence.push(unit);
        op.evidence_ids.push("unit".into());
        assert!(!supports_operation(&op, &p));
    }
    #[test]
    fn supports_removal_unknown_components_and_scoped_proxy_histories() {
        let (mut op, p) = claim(TABLE);
        if let Some(Replacement::Point { rates, .. }) = &mut op.replacement {
            *rates = Rates::default();
        }
        assert!(supports_operation(&op, &p));
        op.kind = Kind::Remove;
        op.replacement = None;
        op.expected = Value::Rates(Rates::default());
        assert!(supports_operation(&op, &p));
        let (mut op, mut p) = claim("Proxy: alias; Target: first; Effective date: 2026-01-01");
        op.target = Target::Proxy {
            proxy: "alias".into(),
        };
        op.replacement = Some(Replacement::Proxy {
            points: vec![
                super::super::catalog::ProxyEntry {
                    target: "first".into(),
                    effective_from: Some("2026-01-01".into()),
                },
                super::super::catalog::ProxyEntry {
                    target: "second".into(),
                    effective_from: Some("2026-02-01".into()),
                },
            ],
        });
        let mut extra = p.evidence[0].clone();
        extra.id = "second".into();
        extra.excerpt = "Proxy: alias; Target: second; Effective date: 2026-02-01".into();
        p.evidence.push(extra);
        op.evidence_ids.push("second".into());
        assert!(supports_operation(&op, &p));
        p.evidence[1].excerpt = "Proxy: another; Target: second; Effective date: 2026-02-01".into();
        assert!(!supports_operation(&op, &p));
    }
}
