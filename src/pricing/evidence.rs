//! Bounded acquisition and verification of public pricing evidence.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::time::{Duration, Instant};

const PRICING_URL: &str = "https://developers.openai.com/api/docs/pricing.md";
const MAX_BYTES: u64 = 2_000_000;
const MAX_REQUESTS: usize = 64;
const TOTAL_TIME: Duration = Duration::from_secs(60);
const REQUEST_TIME: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Evidence {
    pub id: String,
    pub url: String,
    pub retrieved_at: String,
    pub excerpt: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ObservedRate {
    pub model: String,
    pub tier: String,
    pub context: String,
    /// Input, cached input, cache write, output (dollars per million tokens).
    pub rates: [Option<f64>; 4],
    pub effective_from: Option<String>,
    pub evidence_id: String,
}

#[derive(Debug, Default)]
pub(crate) struct Collection {
    pub observations: Vec<ObservedRate>,
    pub evidence: Vec<Evidence>,
    pub unresolved: Vec<String>,
    pub fetched: FetchCache,
}

#[derive(Debug)]
pub(crate) struct FetchCache {
    bytes: BTreeMap<String, String>,
    failures: BTreeMap<String, String>,
    started: Instant,
    requests: usize,
    total_bytes: u64,
}

impl Default for FetchCache {
    fn default() -> Self {
        Self {
            bytes: BTreeMap::new(),
            failures: BTreeMap::new(),
            started: Instant::now(),
            requests: 0,
            total_bytes: 0,
        }
    }
}

// Only documented official sources and exact archived captures of those sources.
fn official(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let Some((host, path)) = rest.split_once('/') else {
        return false;
    };
    matches!(
        host,
        "developers.openai.com" | "platform.openai.com" | "openai.com" | "help.openai.com"
    ) && !path.is_empty()
        && !url.contains(['#', '\\', '@', '\r', '\n', ' '])
}
fn allowed(url: &str) -> bool {
    if official(url) {
        return true;
    }
    let Some(rest) = url.strip_prefix("https://web.archive.org/web/") else {
        return false;
    };
    let Some((capture, original)) = rest.split_once('/') else {
        return false;
    };
    let timestamp = capture.strip_suffix("id_").unwrap_or(capture);
    timestamp.len() == 14 && timestamp.bytes().all(|b| b.is_ascii_digit()) && official(original)
}

fn archive_target(url: &str) -> Option<&str> {
    url.strip_prefix("https://web.archive.org/web/")?
        .split_once('/')
        .map(|(_, original)| original)
}

fn archive_capture(url: &str) -> Option<&str> {
    url.strip_prefix("https://web.archive.org/web/")?
        .split_once('/')
        .map(|(capture, _)| capture.strip_suffix("id_").unwrap_or(capture))
}

impl FetchCache {
    fn get(&mut self, url: &str) -> Result<String, String> {
        if let Some(content) = self.bytes.get(url) {
            return Ok(content.clone());
        }
        if let Some(error) = self.failures.get(url) {
            return Err(error.clone());
        }
        let result = self.fetch(url);
        match &result {
            Ok(content) => {
                self.bytes.insert(url.to_string(), content.clone());
            }
            Err(error) => {
                self.failures.insert(url.to_string(), error.clone());
            }
        }
        result
    }
    fn fetch(&mut self, url: &str) -> Result<String, String> {
        if !allowed(url) {
            return Err("URL is not an allowed official source or exact archive capture".into());
        }
        let mut current = url.to_string();
        let archive_original = archive_target(url);
        let original_capture = archive_capture(url);
        let mut redirects = 0;
        let content = loop {
            if self.requests >= MAX_REQUESTS {
                return Err("evidence request limit reached".into());
            }
            let remaining = TOTAL_TIME
                .checked_sub(self.started.elapsed())
                .ok_or("evidence time limit reached")?;
            self.requests += 1;
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .max_redirects(0)
                .http_status_as_error(false)
                .timeout_global(Some(remaining.min(REQUEST_TIME)))
                .build()
                .into();
            let mut response = agent
                .get(&current)
                .call()
                .map_err(|e| format!("fetch failed: {e}"))?;
            if response.status().is_redirection() {
                if redirects >= 4 {
                    return Err("evidence redirect limit reached".into());
                }
                let location = response
                    .headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                    .ok_or("invalid redirect")?;
                let next = if location.starts_with("https://") {
                    location.to_string()
                } else if location.starts_with('/') && !location.starts_with("//") {
                    let origin = current
                        .strip_prefix("https://")
                        .and_then(|v| v.split_once('/'))
                        .map(|(h, _)| h)
                        .ok_or("invalid redirect origin")?;
                    format!("https://{origin}{location}")
                } else {
                    return Err("unsupported redirect location".into());
                };
                if !allowed(&next)
                    || archive_target(&next) != archive_original
                    || archive_capture(&next) != original_capture
                    || (url == PRICING_URL && next != PRICING_URL)
                {
                    return Err("redirect changes source or archive original identity".into());
                }
                redirects += 1;
                current = next;
                continue;
            }
            if !response.status().is_success() {
                return Err(format!("fetch returned HTTP {}", response.status()));
            }
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|x| x.to_str().ok())
                .unwrap_or("")
                .to_ascii_lowercase();
            if url == PRICING_URL
                && !(content_type.starts_with("text/markdown")
                    || content_type.starts_with("text/plain"))
            {
                return Err("current pricing source is not Markdown".into());
            }
            if !(content_type.starts_with("text/")
                || content_type.starts_with("application/xhtml+xml"))
            {
                return Err("unsupported evidence representation".into());
            }
            let budget = MAX_BYTES.min(8_000_000u64.saturating_sub(self.total_bytes));
            if budget == 0 {
                return Err("evidence total size limit reached".into());
            }
            let bytes = match read_decoded_limited(response.body_mut().as_reader(), budget) {
                Ok(bytes) => bytes,
                Err(error) => {
                    self.total_bytes += budget;
                    return Err(error);
                }
            };
            self.total_bytes += bytes.len() as u64;
            break String::from_utf8(bytes).map_err(|_| "evidence is not UTF-8".to_string())?;
        };
        Ok(content)
    }
}

fn read_decoded_limited(reader: impl Read, budget: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .take(budget + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("evidence body failed: {e}"))?;
    if bytes.len() as u64 > budget {
        return Err("evidence decoded body size limit reached".into());
    }
    Ok(bytes)
}

/// Verify every supplied citation. A fresh cache must be used for imported files;
/// only deterministic collection may pass its own cache.
pub(crate) fn verify_citations(
    evidence: &[Evidence],
    cache: &mut FetchCache,
) -> BTreeMap<String, Result<(), String>> {
    let mut result = BTreeMap::new();
    for item in evidence {
        let verified = if time::OffsetDateTime::parse(
            &item.retrieved_at,
            &time::format_description::well_known::Rfc3339,
        )
        .is_err()
        {
            Err("invalid retrieval timestamp".into())
        } else if item.excerpt.trim().is_empty() {
            Err("empty excerpt".into())
        } else {
            cache.get(&item.url).and_then(|content| {
                let quote = normalize(&item.excerpt);
                if quote.is_empty() || !contains_quote(&normalize(&content), &quote) {
                    Err("excerpt is absent from retrieved source".into())
                } else {
                    Ok(())
                }
            })
        };
        if result.insert(item.id.clone(), verified).is_some() {
            result.insert(item.id.clone(), Err("duplicate evidence ID".into()));
        }
    }
    result
}

fn contains_quote(content: &str, quote: &str) -> bool {
    content.match_indices(quote).any(|(start, _)| {
        let end = start + quote.len();
        let first = quote.chars().next().unwrap_or(' ');
        let last = quote.chars().last().unwrap_or(' ');
        let before = content[..start].chars().next_back();
        let after = content[end..].chars().next();
        let after_decimal = after == Some('.')
            && content[end + 1..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit());
        let before_decimal = before == Some('.')
            && content[..start - 1]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_digit());
        !(first.is_ascii_alphanumeric()
            && (before.is_some_and(|c| c.is_ascii_alphanumeric()) || before_decimal)
            || last.is_ascii_alphanumeric()
                && (after.is_some_and(|c| c.is_ascii_alphanumeric()) || after_decimal)
            || (first == '$' || first.is_ascii_digit())
                && before.is_some_and(|c| matches!(c, '-' | '+' | '−')))
    })
}

// Deliberately limited normalization: preserve punctuation, signs, digits, units,
// and table separators. Unknown entities and malformed tags remain unmatched.
fn normalize(input: &str) -> String {
    let mut visible = String::new();
    let mut chars = input.chars().peekable();
    let mut hidden: Option<String> = None;
    while let Some(c) = chars.next() {
        if c == '<' && chars.clone().take(3).collect::<String>() == "!--" {
            chars.next();
            chars.next();
            chars.next();
            let mut previous = ['\0', '\0'];
            let mut closed = false;
            for x in chars.by_ref() {
                if previous == ['-', '-'] && x == '>' {
                    closed = true;
                    break;
                }
                previous = [previous[1], x];
            }
            if !closed {
                return String::new();
            }
            visible.push(' ');
        } else if c == '<'
            && chars
                .peek()
                .is_some_and(|x| x.is_ascii_alphabetic() || *x == '/' || *x == '!')
        {
            let mut tag = String::new();
            let mut closed = false;
            for x in chars.by_ref() {
                if x == '>' {
                    closed = true;
                    break;
                }
                tag.push(x);
            }
            if !closed {
                return String::new();
            }
            let name = tag
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            if matches!(name.as_str(), "script" | "style") {
                hidden = Some(name);
            } else if hidden.as_ref().is_some_and(|h| name == format!("/{h}")) {
                hidden = None;
            }
            if hidden.is_none() {
                visible.push(' ');
            }
        } else if hidden.is_none() {
            visible.push(c);
        }
    }
    // Decode recognized entities once: &amp;lt; remains &lt;, not '<'.
    let mut decoded = String::new();
    let mut rest = visible.as_str();
    while let Some(index) = rest.find('&') {
        decoded.push_str(&rest[..index]);
        rest = &rest[index..];
        let replacement = [
            ("&amp;", "&"),
            ("&lt;", "<"),
            ("&gt;", ">"),
            ("&quot;", "\""),
            ("&#39;", "'"),
            ("&nbsp;", " "),
        ]
        .into_iter()
        .find(|(entity, _)| rest.starts_with(entity));
        if let Some((entity, value)) = replacement {
            decoded.push_str(value);
            rest = &rest[entity.len()..];
        } else {
            decoded.push('&');
            rest = &rest[1..];
        }
    }
    decoded.push_str(rest);
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn collect_current() -> Collection {
    let mut fetched = FetchCache::default();
    let mut result = Collection {
        observations: Vec::new(),
        evidence: Vec::new(),
        unresolved: Vec::new(),
        fetched: FetchCache::default(),
    };
    match fetched.get(PRICING_URL) {
        Ok(content) => {
            let retrieved_at = time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default();
            extract_pricing(&content, &retrieved_at, &mut result);
        }
        Err(error) => result
            .unresolved
            .push(format!("current pricing source: {error}")),
    }
    result.fetched = fetched;
    result
}

// The flagship section has an explicit unit, speed headings and context-labeled
// columns. Do not interpret other sections, flattened tables or pricing modifiers.
pub(super) const LEGEND: &str = "flagship-context-legend";

/// Facts derived from whole retrieved documents, never from proposal-supplied scope.
pub(crate) fn scoped_documents(cache: &FetchCache) -> BTreeMap<String, Collection> {
    cache
        .bytes
        .iter()
        .filter_map(|(url, text)| {
            if !(url == PRICING_URL
                || (archive_target(url) == Some(PRICING_URL)
                    && url
                        .split_once("id_/")
                        .is_some_and(|(_, original)| original == PRICING_URL)))
                || text.trim_start().starts_with('<')
            {
                return None;
            }
            let mut facts = Collection::default();
            extract_pricing(text, "", &mut facts);
            Some((url.clone(), facts))
        })
        .collect()
}

pub(super) fn cutoff(legend: &str) -> Option<u64> {
    fn tokens(s: &str) -> Option<u64> {
        let (digits, scale) = s.strip_suffix('K').map_or((s, 1), |d| (d, 1000));
        if !digits.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        digits.parse::<u64>().ok()?.checked_mul(scale)
    }
    let (short, long) = legend
        .strip_prefix("Short context: ≤")?
        .strip_suffix(" input tokens.")?
        .split_once(" input tokens. Long context: >")?;
    let n = tokens(short)?;
    (n > 0 && n < u64::MAX && tokens(long)? == n).then_some(n)
}

fn flagship_region(content: &str) -> Option<&str> {
    let mut start = None;
    let mut offset = 0;
    for line in content.split_inclusive('\n') {
        let label = line.trim();
        if let Some(start) = start {
            if label.ends_with(" models") && !label.contains('|') {
                let after = &content[offset + line.len()..];
                return (label != "Flagship models"
                    && (label == "Cyber models"
                        || after
                            .lines()
                            .map(str::trim)
                            .find(|s| !s.is_empty())
                            .is_some_and(|s| s.starts_with("Prices per 1M tokens"))))
                .then_some(&content[start..offset]);
            }
        } else if label == "Flagship models" {
            start = Some(offset + line.len());
        }
        offset += line.len();
    }
    None
}

pub(super) fn extract_pricing(content: &str, retrieved_at: &str, out: &mut Collection) {
    let Some(flagship) = flagship_region(content) else {
        out.unresolved
            .push("flagship pricing section unavailable".into());
        return;
    };
    // A second unit declaration can indicate an intervening product block.
    // Do not bind its legend or rows to the first block.
    let mut units = flagship
        .lines()
        .map(str::trim)
        .filter(|s| s.starts_with("Prices per "));
    if units.next() != Some("Prices per 1M tokens.")
        || units.next().is_some()
        || !flagship
            .split("### ")
            .next()
            .unwrap_or("")
            .lines()
            .any(|s| s.trim() == "Prices per 1M tokens.")
    {
        out.unresolved
            .push("flagship pricing units unavailable".into());
        return;
    }
    out.evidence.push(Evidence {
        id: "pricing-units".into(),
        url: PRICING_URL.into(),
        retrieved_at: retrieved_at.into(),
        excerpt: "Prices per 1M tokens.".into(),
    });
    let mut legends = flagship.lines().map(str::trim).filter(|s| {
        !s.starts_with('|') && (s.contains("Short context:") || s.contains("Long context:"))
    });
    if let Some(line) = legends.next()
        && legends.next().is_none()
        && cutoff(line).is_some()
        && !flagship.contains("<!--")
        && !flagship.contains("```")
    {
        out.evidence.push(Evidence {
            id: LEGEND.into(),
            url: PRICING_URL.into(),
            retrieved_at: retrieved_at.into(),
            excerpt: line.into(),
        });
    }
    let mut tier = None;
    let mut promo = false;
    let mut header = false;
    let mut rows = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for line in flagship.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("### ") {
            promo = matches!(
                trimmed,
                "### Ultrafast promotion pricing data" | "### Ultrafast list pricing data"
            );
            tier = match trimmed {
                "### Standard pricing data" => Some("standard"),
                "### Fast pricing data" => Some("fast"),
                "### Ultrafast pricing data" => Some("ultrafast"),
                _ => None,
            };
            header = false;
            if let Some(label) = tier {
                let id = format!("pricing-tier-{label}");
                if !out.evidence.iter().any(|e| e.id == id) {
                    out.evidence.push(Evidence {
                        id,
                        url: PRICING_URL.into(),
                        retrieved_at: retrieved_at.into(),
                        excerpt: trimmed.into(),
                    });
                }
            }
        }
        if trimmed.starts_with("| Model |") {
            header = trimmed
                == "| Model | Short context input | Short context output | Short context cached input | Short context 30m write | Short context 12h write | Long context input | Long context output | Long context cached input | Long context 30m write | Long context 12h write |";
            if header && let Some(label) = tier {
                let id = format!("pricing-header-{label}");
                if !out.evidence.iter().any(|e| e.id == id) {
                    out.evidence.push(Evidence {
                        id,
                        url: PRICING_URL.into(),
                        retrieved_at: retrieved_at.into(),
                        excerpt: trimmed.into(),
                    });
                }
            }
            continue;
        }
        if tier.is_none() && !promo {
            continue;
        }
        if !header || !trimmed.starts_with('|') {
            continue;
        }
        let cells: Vec<_> = trimmed
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        if cells.len() != 11 || cells[0].starts_with('-') {
            continue;
        }
        let model = cells[0];
        if !model.starts_with("gpt-")
            || !model
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'.' || c == b'-')
        {
            out.unresolved
                .push(format!("unrecognized model label: {model}"));
            continue;
        }
        if promo {
            let gap =
                format!("{model} / ultrafast: promotional/list pricing is not verified by refresh");
            if !out.unresolved.contains(&gap) {
                out.unresolved.push(gap);
            }
            continue;
        }
        let Some(tier) = tier else { continue };
        let row_key = (model.to_string(), tier.to_string());
        if !rows.insert(row_key.clone()) {
            if duplicates.insert(row_key.clone()) {
                out.unresolved
                    .push(format!("duplicate {tier} pricing rows for {model}"));
                let ids: BTreeSet<_> = out
                    .observations
                    .iter()
                    .filter(|o| o.model == model && o.tier == tier)
                    .map(|o| o.evidence_id.clone())
                    .collect();
                out.observations
                    .retain(|o| o.model != model || o.tier != tier);
                out.evidence.retain(|e| !ids.contains(&e.id));
            }
            continue;
        }
        for (context, offset) in [("short", 1), ("long", 6)] {
            let parsed: Option<Vec<Option<f64>>> = [offset, offset + 2, offset + 3, offset + 1]
                .into_iter()
                .map(|i| parse_rate(cells[i]))
                .collect();
            let Some(rates) = parsed else {
                out.unresolved
                    .push(format!("unsupported {tier} {context} rate for {model}"));
                continue;
            };
            if rates.iter().all(Option::is_none) {
                continue;
            }
            // 12-hour writes are a different accounting dimension and cannot be
            // substituted for the catalog's ordinary cache-write component.
            let id = format!("pricing-{}-{}-{}", model, tier, context);
            let excerpt = trimmed.to_string();
            out.evidence.push(Evidence {
                id: id.clone(),
                url: PRICING_URL.into(),
                retrieved_at: retrieved_at.into(),
                excerpt,
            });
            out.observations.push(ObservedRate {
                model: model.into(),
                tier: tier.into(),
                context: context.into(),
                rates: rates.try_into().unwrap(),
                effective_from: None,
                evidence_id: id,
            });
        }
    }
    // One bounded shared excerpt per speed includes the unit and the labeled
    // table in their original order, so imported proposals can verify scope.
    let unit_start = flagship.find("Prices per 1M tokens.").unwrap();
    for (name, heading) in [
        ("standard", "### Standard pricing data"),
        ("fast", "### Fast pricing data"),
        ("ultrafast", "### Ultrafast pricing data"),
    ] {
        let positions: Vec<_> = flagship.match_indices(heading).map(|(i, _)| i).collect();
        if positions.len() != 1 || positions[0] < unit_start {
            continue;
        }
        let table_start = positions[0];
        let end = flagship[table_start + heading.len()..]
            .find("\n### ")
            .map(|n| table_start + heading.len() + n)
            .unwrap_or(flagship.len());
        out.evidence.push(Evidence {
            id: format!("pricing-section-{name}"),
            url: PRICING_URL.into(),
            retrieved_at: retrieved_at.into(),
            excerpt: flagship[unit_start..end].trim().into(),
        });
    }
    // Threshold and cache-write duration semantics require separate evidence and
    // cannot be guessed from the column names. The pipeline must treat this as partial.
    if out.observations.is_empty() {
        out.unresolved
            .push("no supported labeled pricing rows found".into());
    }
}
fn parse_rate(s: &str) -> Option<Option<f64>> {
    if s == "-" {
        return Some(None);
    }
    let amount = s.strip_prefix('$')?;
    if amount.is_empty() || !amount.bytes().all(|c| c.is_ascii_digit() || c == b'.') {
        return None;
    }
    let n: f64 = amount.parse().ok()?;
    n.is_finite().then_some(Some(n))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn citation_verification_preserves_numbers_and_source_identity() {
        let mut cache = FetchCache::default();
        let url = "https://developers.openai.com/test";
        cache.bytes.insert(
            url.into(),
            "<p>Input &gt; 272K: $2.50 per 1M tokens</p><script>fake $0</script>".into(),
        );
        let make = |id: &str, excerpt: &str| Evidence {
            id: id.into(),
            url: url.into(),
            retrieved_at: "2026-01-01T00:00:00Z".into(),
            excerpt: excerpt.into(),
        };
        let results = verify_citations(
            &[
                make("real", "Input > 272K: $2.50 per 1M tokens"),
                make("wrong", "Input > 272K: $2.05 per 1M tokens"),
                make("script", "fake $0"),
            ],
            &mut cache,
        );
        assert_eq!(
            cache.requests, 0,
            "same-run bytes must be reused without a fetch"
        );
        let mut unavailable = make("unavailable", "text");
        unavailable.url = "https://untrusted.example/source".into();
        assert!(
            verify_citations(&[unavailable], &mut FetchCache::default())["unavailable"].is_err()
        );
        assert!(results["real"].is_ok());
        assert!(results["wrong"].is_err());
        assert!(results["script"].is_err());
        assert!(!contains_quote("rate $20 per 1M", "rate $2"));
        assert!(!contains_quote("rate -$2 per 1M", "$2 per 1M"));
        assert!(allowed(
            "https://web.archive.org/web/20260101000000id_/https://openai.com/news/a"
        ));
        assert!(allowed(
            "https://web.archive.org/web/20260101000000/https://openai.com/news/a"
        ));
        assert!(!allowed(
            "https://web.archive.org/web/20260101000000id_/https://evil.example/openai.com"
        ));
        assert!(!allowed("https://developers.openai.com.evil.example/a"));
    }
    #[test]
    fn imported_cutoffs_require_scoped_rows_and_fail_atomically() {
        use crate::pricing::{Catalog, catalog::Overlay, proposal};
        let fixture = include_str!("../../tests/fixtures/pricing/flagship.md");
        let fixture = fixture.replace("gpt-6.2-", "gpt-test-");
        let page = fixture.as_str();
        let mut collection = Collection::default();
        extract_pricing(page, "2026-11-18T12:00:00Z", &mut collection);
        let catalog = Catalog::embedded().unwrap();
        let overlay = Overlay::default();
        let p = proposal::from_collection(&catalog, &overlay, &collection).unwrap();
        let check = |page: &str, p: &proposal::Proposal| {
            let mut cache = FetchCache::default();
            cache.bytes.insert(PRICING_URL.into(), page.into());
            let verified = verify_citations(&p.evidence, &mut cache);
            proposal::prepare_scoped(
                &catalog,
                &overlay,
                p,
                &verified,
                false,
                &scoped_documents(&cache),
            )
            .unwrap()
        };
        let imported = check(page, &p);
        for model in ["gpt-test-astra", "gpt-test-sol"] {
            assert!(
                imported
                    .candidate
                    .thresholds
                    .iter()
                    .any(|t| t.model == model && t.threshold == Some(272000)),
                "{:?}",
                imported.unresolved
            );
            assert_eq!(
                imported
                    .candidate
                    .points
                    .iter()
                    .filter(|p| p.key.model == model)
                    .count(),
                4
            );
        }
        let mut stale = p.clone();
        stale
            .operations
            .iter_mut()
            .find(|op| {
                op.group == "onboard-gpt-test-sol"
                    && matches!(op.target, proposal::Target::Point { .. })
            })
            .unwrap()
            .expected = proposal::Value::Rates(crate::pricing::Rates::default());
        let partial = check(page, &stale);
        assert_eq!(partial.candidate.thresholds.len(), 1);
        assert!(
            partial
                .candidate
                .points
                .iter()
                .all(|p| p.key.model != "gpt-test-sol")
        );
        let legend = collection.evidence.iter().find(|e| e.id == LEGEND).unwrap();
        let moved = format!("{}\n{}", page.replace(&legend.excerpt, ""), legend.excerpt);
        assert!(check(&moved, &p).candidate.thresholds.is_empty());
    }
    #[test]
    fn shared_cutoff_stays_inside_flagship_family() {
        let page = include_str!("../../tests/fixtures/pricing/flagship.md");
        let line = "Short context: ≤272K input tokens. Long context: >272K input tokens.";
        let legend = |page: &str| {
            let mut c = Collection::default();
            extract_pricing(page, "", &mut c);
            c.evidence
                .iter()
                .find(|e| e.id == LEGEND)
                .and_then(|e| cutoff(&e.excerpt))
        };
        assert_eq!(legend(page), Some(272_000));
        for bad in [
            page.replace(line, &format!("For Astra: {line}")),
            page.replace(line, &format!("{line}\n{}", line.replace("272K", "128K"))),
            page.replace("Multimodal models", "missing"),
            format!("{}\n{line}", page.replace(line, "")),
        ] {
            assert_eq!(legend(&bad), None);
        }
    }
    #[test]
    fn extraction_requires_labeled_context_and_ignores_discount_tiers() {
        let text = "Flagship models\nPrices per 1M tokens.\n### Standard pricing data\n| Model | Short context input | Short context output | Short context cached input | Short context 30m write | Short context 12h write | Long context input | Long context output | Long context cached input | Long context 30m write | Long context 12h write |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n| gpt-6-sol | $2.00 | $10.00 | $0.20 | $2.50 | $5.00 | $4.00 | $15.00 | $0.40 | $5.00 | $10.00 |\n### Batch pricing data\n| gpt-6-sol | $1.00 | $5.00 | $0.10 | $1.25 | $2.50 | $2.00 | $7.50 | $0.20 | $2.50 | $5.00 |\nCyber models";
        let mut out = Collection {
            observations: vec![],
            evidence: vec![],
            unresolved: vec![],
            fetched: FetchCache::default(),
        };
        extract_pricing(text, "2026-01-01T00:00:00Z", &mut out);
        assert_eq!(out.observations.len(), 2);
        assert_eq!(
            out.observations[0].rates,
            [Some(2.0), Some(0.2), Some(2.5), Some(10.0)]
        );
        assert!(out.observations.iter().all(|x| x.tier == "standard"));
    }
    #[test]
    fn quotation_boundaries_preserve_sign_and_allow_sentence_punctuation() {
        assert!(contains_quote("Rate is $2.50. Next", "$2.50"));
        assert!(!contains_quote("Rate is $2.500 per token", "$2.50"));
        for sign in ['+', '-', '−'] {
            assert!(!contains_quote(&format!("Rate is {sign}$2.50."), "$2.50"));
            assert!(!contains_quote(&format!("Rate is {sign}2.50."), "2.50"));
        }
    }
    #[test]
    fn normalization_excludes_complete_comments_and_decodes_entities_once() {
        assert_eq!(
            normalize("visible <!-- a > hidden $2 --> text"),
            "visible text"
        );
        assert_eq!(normalize("&amp;lt; &lt;"), "&lt; <");
        assert_eq!(normalize("a <!-- unfinished > $2"), "");
    }
    #[test]
    fn decoded_response_limit_and_failed_fetch_cache_are_bounded() {
        assert_eq!(read_decoded_limited(&b"abcd"[..], 4).unwrap(), b"abcd");
        assert!(read_decoded_limited(&b"abcde"[..], 4).is_err());
        let mut cache = FetchCache {
            requests: MAX_REQUESTS,
            ..FetchCache::default()
        };
        let url = "https://openai.com/news/test";
        let first = cache.get(url).unwrap_err();
        cache.requests = 0;
        assert_eq!(cache.get(url).unwrap_err(), first);
        assert_eq!(cache.requests, 0);
    }
    #[test]
    fn duplicate_rows_are_unresolved_without_removing_independent_models() {
        let input = "Flagship models\nPrices per 1M tokens.\n### Standard pricing data\n| Model | Short context input | Short context output | Short context cached input | Short context 30m write | Short context 12h write | Long context input | Long context output | Long context cached input | Long context 30m write | Long context 12h write |\n| gpt-6-sol | $2 | $10 | $0.2 | $2.5 | $5 | $4 | $15 | $0.4 | $5 | $10 |\n| gpt-6-sol | $3 | $10 | $0.2 | $2.5 | $5 | $4 | $15 | $0.4 | $5 | $10 |\n| gpt-6-luna | $1 | $2 | $0.1 | $1 | $2 | $2 | $3 | $0.2 | $2 | $4 |\nCyber models";
        let mut out = Collection {
            observations: vec![],
            evidence: vec![],
            unresolved: vec![],
            fetched: FetchCache::default(),
        };
        extract_pricing(input, "2026-01-01T00:00:00Z", &mut out);
        assert_eq!(out.observations.len(), 2);
        assert!(out.observations.iter().all(|o| o.model == "gpt-6-luna"));
        assert!(
            !out.evidence
                .iter()
                .any(|e| e.id.starts_with("pricing-gpt-6-sol-"))
        );
        assert!(
            out.unresolved
                .iter()
                .any(|u| u.contains("duplicate") && u.contains("gpt-6-sol"))
        );
    }
}
