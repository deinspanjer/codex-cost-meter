//! Typed proposal validation and application to an in-memory overlay.
use super::catalog::{
    Band, Basis, Overlay, Point, PointKey, ProxyEntry, ProxyOverride, Removal, ThresholdOverride,
    Tier,
};
use super::{Catalog, Rates};
use crate::pricing_evidence::{Collection, Evidence};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Proposal {
    pub proposal_schema_version: u32,
    pub catalog_schema_version: u32,
    pub operations: Vec<Operation>,
    pub evidence: Vec<Evidence>,
    pub coverage: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Operation {
    pub id: String,
    pub group: String,
    pub kind: Kind,
    pub target: Target,
    pub expected: Value,
    pub replacement: Option<Replacement>,
    pub evidence_ids: Vec<String>,
    pub date_basis: String,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind {
    Add,
    ReplaceValue,
    Remove,
    Move,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Target {
    Point { key: PointKey },
    OverlayPoint { key: PointKey },
    Tombstone { key: PointKey },
    Threshold { model: String },
    Proxy { proxy: String },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub(crate) enum Value {
    Absent,
    Rates(Rates),
    Threshold(u64),
    Proxy(Vec<ProxyEntry>),
    Tombstone,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Replacement {
    Point { key: PointKey, rates: Rates },
    Threshold { threshold: u64 },
    Proxy { points: Vec<ProxyEntry> },
}
#[derive(Debug, Serialize)]
pub(crate) struct Change {
    pub id: String,
    pub before: Value,
    pub after: Value,
    pub description: String,
    pub target: Option<Target>,
    pub replacement: Option<Replacement>,
    pub date_basis: Option<String>,
    pub evidence_ids: Vec<String>,
}
#[derive(Debug)]
pub(crate) struct Prepared {
    pub candidate: Overlay,
    pub changes: Vec<Change>,
    pub unresolved: Vec<String>,
}

fn identity(target: &Target) -> String {
    match target {
        Target::Point { key } | Target::OverlayPoint { key } | Target::Tombstone { key } => {
            format!(
                "point:{}:{:?}:{:?}:{}",
                key.model, key.tier, key.context, key.effective_from
            )
        }
        Target::Threshold { model } => format!("threshold:{model}"),
        Target::Proxy { proxy } => format!("proxy:{proxy}"),
    }
}
fn current(c: &Catalog, o: &Overlay, target: &Target) -> Value {
    match target {
        Target::Point { key } | Target::OverlayPoint { key } => c
            .effective_rates(key)
            .map(Value::Rates)
            .unwrap_or(Value::Absent),
        Target::Tombstone { key } => {
            if o.removals.iter().any(|x| &x.key == key) {
                Value::Tombstone
            } else {
                Value::Absent
            }
        }
        Target::Threshold { model } => c
            .long_context_thresholds
            .get(model)
            .copied()
            .map(Value::Threshold)
            .unwrap_or(Value::Absent),
        Target::Proxy { proxy } => c
            .proxies
            .get(proxy)
            .map(|p| {
                Value::Proxy(
                    p.iter()
                        .map(|v| ProxyEntry {
                            effective_from: v.effective_from.map(|d| d.to_string()),
                            target: v.target.clone(),
                        })
                        .collect(),
                )
            })
            .unwrap_or(Value::Absent),
    }
}
fn valid_key(key: &PointKey) -> bool {
    !key.model.trim().is_empty()
        && time::Date::parse(
            &key.effective_from,
            time::macros::format_description!("[year]-[month]-[day]"),
        )
        .is_ok_and(|d| d.to_string() == key.effective_from)
}
fn valid_rates(rates: &Rates) -> bool {
    rates.values().all(|r| r.is_finite() && r >= 0.0)
}
pub(crate) fn validate_structure(p: &Proposal) -> Result<(), String> {
    if p.proposal_schema_version != 1 || p.catalog_schema_version != 1 {
        return Err("unsupported proposal or catalog schema".into());
    }
    if p.operations.len() > 4096 || p.evidence.len() > 256 {
        return Err("proposal exceeds operation or evidence limit".into());
    }
    let mut ids = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    for e in &p.evidence {
        if e.id.is_empty() || !evidence.insert(&e.id) {
            return Err("invalid or duplicate evidence ID".into());
        }
    }
    for op in &p.operations {
        if op.id.is_empty()
            || op.group.is_empty()
            || !ids.insert(&op.id)
            || op.evidence_ids.is_empty()
            || op.date_basis.trim().is_empty()
        {
            return Err("invalid operation ID, group, evidence, or date basis".into());
        }
        if op.evidence_ids.iter().any(|id| !evidence.contains(id)) {
            return Err(format!("{} references missing evidence", op.id));
        }
        let target_valid = match &op.target {
            Target::Point { key } | Target::OverlayPoint { key } | Target::Tombstone { key } => {
                valid_key(key)
            }
            Target::Threshold { model } => !model.trim().is_empty(),
            Target::Proxy { proxy } => !proxy.trim().is_empty(),
        };
        let replacement_valid = match &op.replacement {
            Some(Replacement::Point { key, rates }) => valid_key(key) && valid_rates(rates),
            Some(Replacement::Threshold { threshold }) => *threshold > 0 && *threshold < u64::MAX,
            Some(Replacement::Proxy { points }) => {
                !points.is_empty()
                    && points.iter().all(|p| {
                        !p.target.trim().is_empty()
                            && p.effective_from.as_ref().is_none_or(|d| {
                                time::Date::parse(
                                    d,
                                    time::macros::format_description!("[year]-[month]-[day]"),
                                )
                                .is_ok_and(|v| v.to_string() == *d)
                            })
                    })
            }
            None => true,
        };
        if !target_valid
            || !replacement_valid
            || matches!(&op.expected, Value::Rates(r) if !valid_rates(r))
        {
            return Err(format!("{} has invalid target or value", op.id));
        }
        let valid = match (&op.kind, &op.target, &op.expected, &op.replacement) {
            (
                Kind::Add,
                Target::Point { key },
                Value::Absent | Value::Rates(_),
                Some(Replacement::Point { key: k, .. }),
            ) => key == k,
            (
                Kind::ReplaceValue,
                Target::Point { key },
                Value::Rates(_),
                Some(Replacement::Point { key: k, .. }),
            ) => key == k,
            (
                Kind::Move,
                Target::Point { key },
                Value::Rates(_),
                Some(Replacement::Point { key: k, .. }),
            ) => key != k && key.model == k.model && key.tier == k.tier && key.context == k.context,
            (
                Kind::Remove,
                Target::Point { .. } | Target::OverlayPoint { .. },
                Value::Rates(_),
                None,
            ) => true,
            (Kind::Remove, Target::Tombstone { .. }, Value::Tombstone, None) => true,
            (
                Kind::Add,
                Target::Threshold { .. },
                Value::Absent,
                Some(Replacement::Threshold { .. }),
            )
            | (
                Kind::ReplaceValue,
                Target::Threshold { .. },
                Value::Threshold(_),
                Some(Replacement::Threshold { .. }),
            )
            | (Kind::Remove, Target::Threshold { .. }, Value::Threshold(_), None) => true,
            (Kind::Add, Target::Proxy { .. }, Value::Absent, Some(Replacement::Proxy { .. }))
            | (
                Kind::ReplaceValue,
                Target::Proxy { .. },
                Value::Proxy(_),
                Some(Replacement::Proxy { .. }),
            )
            | (Kind::Remove, Target::Proxy { .. }, Value::Proxy(_), None) => true,
            _ => false,
        };
        if !valid {
            return Err(format!(
                "{} has incompatible kind, target, expected, or replacement",
                op.id
            ));
        }
    }
    Ok(())
}
fn remove_key(o: &mut Overlay, embedded: &Catalog, key: &PointKey, source: &str) {
    o.points.retain(|x| &x.key != key);
    o.removals.retain(|x| &x.key != key);
    if o.base.as_deref() != Some("none") && embedded.points.contains_key(key) {
        o.removals.push(Removal {
            key: key.clone(),
            basis: Basis::Researched,
            source: Some(source.into()),
        });
    }
}
fn apply_op(o: &mut Overlay, embedded: &Catalog, op: &Operation, source: &str) {
    match &op.target {
        Target::Point { key } => {
            if matches!(op.kind, Kind::Remove | Kind::Move) {
                remove_key(o, embedded, key, source);
            }
            if let Some(Replacement::Point { key, rates }) = &op.replacement {
                o.removals.retain(|x| &x.key != key);
                o.points.retain(|x| &x.key != key);
                o.points.push(Point {
                    key: key.clone(),
                    rates: *rates,
                    basis: match op.date_basis.as_str() {
                        "observation" => Basis::Observed,
                        "official" => Basis::Official,
                        _ => Basis::Researched,
                    },
                    source: Some(source.into()),
                });
            }
        }
        Target::OverlayPoint { key } => o.points.retain(|x| &x.key != key),
        Target::Tombstone { key } => o.removals.retain(|x| &x.key != key),
        Target::Threshold { model } => {
            o.thresholds.retain(|x| &x.model != model);
            o.thresholds.push(ThresholdOverride {
                model: model.clone(),
                threshold: match &op.replacement {
                    Some(Replacement::Threshold { threshold }) => Some(*threshold),
                    _ => None,
                },
                basis: if op.date_basis == "observation" {
                    Basis::Observed
                } else {
                    Basis::Researched
                },
                source: Some(source.into()),
            });
        }
        Target::Proxy { proxy } => {
            o.proxies.retain(|x| &x.proxy != proxy);
            o.proxies.push(ProxyOverride {
                proxy: proxy.clone(),
                points: match &op.replacement {
                    Some(Replacement::Proxy { points }) => points.clone(),
                    _ => vec![],
                },
                basis: Basis::Researched,
                source: Some(source.into()),
            });
        }
    }
}

// Imported quotes must at least identify the target and every claimed numeric
// replacement. This remains a conservative support check, not an interpretation proof.
fn custom_at(catalog: &Catalog, overlay: &Overlay, key: &PointKey) -> bool {
    let effective = catalog.points.keys().rfind(|k| {
        k.model == key.model
            && k.tier == key.tier
            && k.context == key.context
            && k.effective_from <= key.effective_from
    });
    overlay
        .points
        .iter()
        .any(|p| p.basis == Basis::Custom && (&p.key == key || Some(&p.key) == effective))
        || overlay
            .removals
            .iter()
            .any(|r| &r.key == key && r.basis == Basis::Custom)
}
fn protects_custom(catalog: &Catalog, overlay: &Overlay, op: &Operation) -> bool {
    if overlay.base.as_deref() == Some("none") {
        return true;
    }
    let target = match &op.target {
        Target::Point { key } | Target::OverlayPoint { key } | Target::Tombstone { key } => {
            custom_at(catalog, overlay, key)
        }
        Target::Threshold { model } => overlay
            .thresholds
            .iter()
            .any(|t| &t.model == model && t.basis == Basis::Custom),
        Target::Proxy { proxy } => overlay
            .proxies
            .iter()
            .any(|p| &p.proxy == proxy && p.basis == Basis::Custom),
    };
    target
        || matches!(&op.replacement,Some(Replacement::Point{key,..}) if custom_at(catalog,overlay,key))
}
fn paired_point_replacement(op: &Operation, group: &[&Operation], proposal: &Proposal) -> bool {
    let key = match &op.target {
        Target::Point { key } | Target::OverlayPoint { key } | Target::Tombstone { key } => key,
        _ => return true,
    };
    group.iter().any(|other| {
        other.id != op.id && matches!(other.kind,Kind::Add | Kind::ReplaceValue | Kind::Move)
            && matches!(&other.replacement,Some(Replacement::Point{key:replacement,rates}) if replacement.model == key.model && replacement.tier == key.tier && replacement.context == key.context && rates.values().next().is_some())
            && super::proposal_support::supports_operation(other,proposal)
    })
}

#[cfg(test)]
fn prepare(
    embedded: &Catalog,
    overlay: &Overlay,
    proposal: &Proposal,
    verified: &BTreeMap<String, Result<(), String>>,
    deterministic: bool,
) -> Result<Prepared, String> {
    prepare_scoped(
        embedded,
        overlay,
        proposal,
        verified,
        deterministic,
        &BTreeMap::new(),
    )
}

fn new_model(catalog: &Catalog, overlay: &Overlay, model: &str) -> bool {
    overlay.base.as_deref() != Some("none")
        && !catalog.points.keys().any(|k| k.model == model)
        && !catalog.proxies.contains_key(model)
        && !catalog.long_context_thresholds.contains_key(model)
        && !overlay.removals.iter().any(|r| r.key.model == model)
        && !overlay.thresholds.iter().any(|t| t.model == model)
        && !overlay.proxies.iter().any(|p| p.proxy == model)
}

fn scoped_threshold(op: &Operation, p: &Proposal, docs: &BTreeMap<String, Collection>) -> bool {
    let (Target::Threshold { model }, Some(Replacement::Threshold { threshold }), Kind::Add) =
        (&op.target, &op.replacement, op.kind)
    else {
        return false;
    };
    let cited: Vec<_> = op
        .evidence_ids
        .iter()
        .filter_map(|id| p.evidence.iter().find(|e| e.id == *id))
        .collect();
    let Some(primary) = cited.first() else {
        return false;
    };
    let Some(facts) = docs.get(&primary.url) else {
        return false;
    };
    let matches_quote = |id: &str| {
        facts
            .evidence
            .iter()
            .find(|e| e.id == id)
            .is_some_and(|actual| {
                cited
                    .iter()
                    .any(|e| e.url == primary.url && e.excerpt == actual.excerpt)
            })
    };
    matches_quote(crate::pricing_evidence::LEGEND)
        && facts.evidence.iter().any(|e| {
            e.id == crate::pricing_evidence::LEGEND
                && crate::pricing_evidence::cutoff(&e.excerpt) == Some(*threshold)
        })
        && facts
            .observations
            .iter()
            .any(|row| row.model == *model && matches_quote(&row.evidence_id))
}

pub(crate) fn prepare_scoped(
    embedded: &Catalog,
    overlay: &Overlay,
    proposal: &Proposal,
    verified: &BTreeMap<String, Result<(), String>>,
    deterministic: bool,
    docs: &BTreeMap<String, Collection>,
) -> Result<Prepared, String> {
    validate_structure(proposal)?;
    let baseline = embedded.merge_overlay(overlay).map_err(|e| e.to_string())?;
    let mut unresolved = proposal.coverage.clone();
    let mut candidate = overlay.clone();
    let mut changes = vec![];
    let mut targets = BTreeMap::<String, usize>::new();
    for op in &proposal.operations {
        *targets.entry(identity(&op.target)).or_default() += 1;
        if let Some(Replacement::Point { key, .. }) = &op.replacement
            && matches!(op.kind, Kind::Move)
        {
            *targets
                .entry(identity(&Target::Point { key: key.clone() }))
                .or_default() += 1;
        }
    }
    let mut groups = BTreeMap::<String, Vec<&Operation>>::new();
    for op in &proposal.operations {
        groups.entry(op.group.clone()).or_default().push(op);
    }
    for (group, ops) in groups {
        let mut error = None;
        for op in &ops {
            if !deterministic && protects_custom(&baseline, overlay, op) {
                error = Some(format!(
                    "{} targets user-controlled custom or standalone data; edit manually",
                    op.id
                ));
                break;
            }
            if !deterministic
                && op.kind == Kind::Remove
                && !paired_point_replacement(op, &ops, proposal)
            {
                error = Some(format!(
                    "{} removal needs a supported replacement in the same pricing history and atomic group",
                    op.id
                ));
                break;
            }
            if targets[&identity(&op.target)] > 1 {
                error = Some("overlapping targets".to_string());
                break;
            }
            if let Target::Point { key } = &op.target {
                let exists = baseline.points.contains_key(key);
                if op.kind == Kind::Add && overlay.removals.iter().any(|r| &r.key == key) {
                    error = Some(format!("{} target has an explicit tombstone", op.id));
                    break;
                }
                if (op.kind == Kind::Add && exists)
                    || (matches!(op.kind, Kind::ReplaceValue | Kind::Remove | Kind::Move)
                        && !exists)
                {
                    error = Some(format!("{} point existence changed", op.id));
                    break;
                }
            }
            if current(&baseline, overlay, &op.target) != op.expected {
                error = Some(format!("{} precondition changed", op.id));
                break;
            }
            if matches!(&op.target,Target::OverlayPoint{key} if !overlay.points.iter().any(|p| &p.key == key))
            {
                error = Some(format!("{} overlay point missing", op.id));
                break;
            }
            if let Some(Replacement::Point { key, .. }) = &op.replacement
                && matches!(op.kind, Kind::Move)
                && (baseline.points.contains_key(key)
                    || overlay.removals.iter().any(|r| &r.key == key)
                    || targets[&identity(&Target::Point { key: key.clone() })] > 1)
            {
                error = Some(format!("{} destination is occupied or overlapping", op.id));
                break;
            }
            if !deterministic
                && !super::proposal_support::supports_operation(op, proposal)
                && !(scoped_threshold(op, proposal, docs)
                    && matches!(&op.target,
                    Target::Threshold { model } if new_model(&baseline, overlay, model) && ops.iter().any(|member| matches!((&member.kind, &member.replacement),
                        (Kind::Add, Some(Replacement::Point { key, .. })) if key.model == *model))))
            {
                error = Some(format!(
                    "{} cited excerpts do not identify all affected facts",
                    op.id
                ));
                break;
            }
            if op
                .evidence_ids
                .iter()
                .any(|id| !verified.get(id).is_some_and(Result::is_ok))
            {
                error = Some(format!("{} evidence could not be verified", op.id));
                break;
            }
        }
        if let Some(e) = error {
            unresolved.push(format!("group {group}: {e}"));
            continue;
        }
        let mut trial = candidate.clone();
        for op in &ops {
            let source = &proposal
                .evidence
                .iter()
                .find(|e| e.id == op.evidence_ids[0])
                .unwrap()
                .url;
            let mut trusted_op = (*op).clone();
            if !deterministic {
                trusted_op.date_basis = "researched".into();
            }
            apply_op(&mut trial, embedded, &trusted_op, source);
        }
        let after = match embedded.merge_overlay(&trial) {
            Ok(v) => v,
            Err(e) => {
                unresolved.push(format!("group {group}: {e}"));
                continue;
            }
        };
        for op in &ops {
            changes.push(Change {
                id: op.id.clone(),
                before: current(&baseline, overlay, &op.target),
                after: current(&after, &trial, &op.target),
                description: if op.date_basis == "observation" && deterministic { format!("{:?}; provisional observation boundary: earlier usage retains the prior price and may be mispriced", op.kind) } else { format!("{:?}", op.kind) },
                target: Some(op.target.clone()),
                replacement: op.replacement.clone(),
                date_basis: Some(op.date_basis.clone()),
                evidence_ids: op.evidence_ids.clone(),
            });
        }
        candidate = trial;
    }
    for point in candidate
        .prune_generated(embedded)
        .map_err(|e| e.to_string())?
    {
        changes.push(Change {
            id: "prune".into(),
            before: Value::Rates(point.rates),
            after: Value::Rates(point.rates),
            description: format!(
                "removed redundant external point; effective price unchanged: {} {}",
                point.key.model, point.key.effective_from
            ),
            target: Some(Target::Point {
                key: point.key.clone(),
            }),
            replacement: None,
            date_basis: None,
            evidence_ids: vec![],
        });
    }
    if candidate.base.as_deref() != Some("none") {
        for p in &candidate.points {
            if matches!(p.basis, Basis::Official | Basis::Observed)
                && p.key.effective_from.as_str() <= embedded.as_of()
                && embedded.effective_rates(&p.key) != Some(p.rates)
            {
                unresolved.push(format!(
                    "generated point {} {} differs from covered embedded history",
                    p.key.model, p.key.effective_from
                ));
            }
        }
    }
    Ok(Prepared {
        candidate,
        changes,
        unresolved,
    })
}

pub(crate) fn from_collection(
    embedded: &Catalog,
    overlay: &Overlay,
    collection: &Collection,
) -> Result<Proposal, String> {
    let current = embedded.merge_overlay(overlay).map_err(|e| e.to_string())?;
    let mut operations = vec![];
    let mut coverage = collection.unresolved.clone();
    let legend = collection
        .evidence
        .iter()
        .find(|e| e.id == crate::pricing_evidence::LEGEND)
        .and_then(|e| crate::pricing_evidence::cutoff(&e.excerpt));
    let mut onboarding = BTreeSet::new();
    for observed in &collection.observations {
        let Some(evidence) = collection
            .evidence
            .iter()
            .find(|e| e.id == observed.evidence_id)
        else {
            return Err("missing collected evidence".into());
        };
        let Ok(retrieved) = time::OffsetDateTime::parse(
            &evidence.retrieved_at,
            &time::format_description::well_known::Rfc3339,
        ) else {
            coverage.push(format!("invalid retrieval date for {}", observed.model));
            continue;
        };
        let day = retrieved.to_offset(time::UtcOffset::UTC).date().to_string();
        let tier = match observed.tier.as_str() {
            "standard" => Tier::Standard,
            "fast" => Tier::Fast,
            "ultrafast" => Tier::Ultrafast,
            _ => {
                coverage.push(format!("unsupported tier for {}", observed.model));
                continue;
            }
        };
        let context = match observed.context.as_str() {
            "short" => Band::Short,
            "long" => Band::Long,
            _ => {
                coverage.push(format!("unsupported context for {}", observed.model));
                continue;
            }
        };
        let existing_short = current
            .points
            .keys()
            .any(|k| k.model == observed.model && k.context == Band::Short);
        let onboard = legend.is_some() && new_model(&current, overlay, &observed.model);
        if !onboard
            && !current
                .long_context_thresholds
                .contains_key(&observed.model)
            && (context == Band::Long || !existing_short)
        {
            coverage.push(format!(
                "{} context threshold unavailable for this band or new model: {}",
                observed.model,
                if legend.is_none() {
                    "no unambiguous scoped legend"
                } else {
                    "existing history or user-controlled catalog"
                }
            ));
            continue;
        }
        let key = PointKey {
            model: observed.model.clone(),
            tier,
            context,
            effective_from: observed
                .effective_from
                .clone()
                .unwrap_or_else(|| day.clone()),
        };
        let effective_key = current.points.keys().rfind(|k| {
            k.model == key.model
                && k.tier == key.tier
                && k.context == key.context
                && k.effective_from <= key.effective_from
        });
        let custom = effective_key.is_some_and(|k| {
            overlay
                .points
                .iter()
                .any(|p| &p.key == k && p.basis == Basis::Custom)
        });
        let latest_embedded = embedded.points.keys().rfind(|k| {
            k.model == key.model
                && k.tier == key.tier
                && k.context == key.context
                && k.effective_from <= key.effective_from
        });
        let tombstoned =
            latest_embedded.is_some_and(|k| overlay.removals.iter().any(|r| &r.key == k));
        if overlay.base.as_deref() == Some("none") || custom || tombstoned {
            coverage.push(format!(
                "{} / {} / {}: retained user-controlled standalone, custom, or tombstoned pricing",
                observed.model, observed.tier, observed.context
            ));
            continue;
        }
        let rates = Rates {
            input: observed.rates[0],
            cached_input: observed.rates[1],
            cache_write_input: observed.rates[2],
            output: observed.rates[3],
        };
        if current.effective_rates(&key) == Some(rates) {
            continue;
        }
        if current.points.contains_key(&key) {
            coverage.push(format!(
                "{} conflicts with an existing same-day point",
                observed.model
            ));
            continue;
        }
        let id = format!("observed-{}", observed.evidence_id);
        let group = if onboard {
            format!("onboard-{}", observed.model)
        } else {
            id.clone()
        };
        if onboard && onboarding.insert(observed.model.clone()) {
            operations.push(Operation {
                id: format!("threshold-{}", observed.model),
                group: group.clone(),
                kind: Kind::Add,
                target: Target::Threshold {
                    model: observed.model.clone(),
                },
                expected: Value::Absent,
                replacement: Some(Replacement::Threshold {
                    threshold: legend.expect("checked above"),
                }),
                evidence_ids: vec![
                    crate::pricing_evidence::LEGEND.into(),
                    observed.evidence_id.clone(),
                ],
                date_basis: "observation".into(),
            });
        }
        operations.push(Operation {
            id: id.clone(),
            group,
            kind: Kind::Add,
            target: Target::Point { key: key.clone() },
            expected: current
                .effective_rates(&key)
                .map(Value::Rates)
                .unwrap_or(Value::Absent),
            replacement: Some(Replacement::Point { key, rates }),
            evidence_ids: [
                observed.evidence_id.clone(),
                "pricing-units".into(),
                format!("pricing-section-{}", observed.tier),
                format!("pricing-tier-{}", observed.tier),
                format!("pricing-header-{}", observed.tier),
            ]
            .into_iter()
            .filter(|id| collection.evidence.iter().any(|e| &e.id == id))
            .collect(),
            date_basis: if observed.effective_from.is_some() {
                "official".into()
            } else {
                "observation".into()
            },
        });
    }
    let observed_cells: BTreeSet<_> = collection
        .observations
        .iter()
        .map(|o| (o.model.clone(), o.tier.clone(), o.context.clone()))
        .collect();
    let existing_cells: BTreeSet<_> = current
        .points
        .iter()
        .filter(|(_, r)| r.values().next().is_some())
        .map(|(k, _)| {
            (
                k.model.clone(),
                format!("{:?}", k.tier).to_ascii_lowercase(),
                format!("{:?}", k.context).to_ascii_lowercase(),
            )
        })
        .collect();
    for (model, tier, context) in existing_cells.difference(&observed_cells) {
        coverage.push(format!("{model} / {tier} / {context}: current pricing not verified by supported Markdown extraction"));
    }
    if !current.long_context_thresholds.is_empty() || !current.proxies.is_empty() {
        coverage.push("Existing context thresholds and proxy histories were retained without fresh verification".into());
    }
    coverage.sort();
    coverage.dedup();
    Ok(Proposal {
        proposal_schema_version: 1,
        catalog_schema_version: 1,
        operations,
        evidence: collection.evidence.clone(),
        coverage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn october_catalog() -> Catalog {
        Catalog::parse(r#"{"as_of":"2026-10-27","histories":{"gpt-6-sol":[{"effective_from":"2026-09-22","input":2}]},"proxies":{}}"#).unwrap()
    }
    fn key(date: &str) -> PointKey {
        PointKey {
            model: "gpt-6-sol".into(),
            tier: Tier::Standard,
            context: Band::Short,
            effective_from: date.into(),
        }
    }
    fn citation() -> Evidence {
        Evidence {
            id: "e".into(),
            url: "https://developers.openai.com/test".into(),
            retrieved_at: "2026-11-06T00:00:00Z".into(),
            excerpt: "Prices per 1M tokens.\n### Standard pricing data\n| Model | Short context input | Short context cached input | Short context 30m write | Short context output |\n| gpt-6-sol | $2 | $0.20 | $2.50 | $10 |".into(),
        }
    }
    fn add(id: &str, date: &str, expected: Value, amount: f64) -> Operation {
        let k = key(date);
        Operation {
            id: id.into(),
            group: id.into(),
            kind: Kind::Add,
            target: Target::Point { key: k.clone() },
            expected,
            replacement: Some(Replacement::Point {
                key: k,
                rates: Rates {
                    input: Some(amount),
                    ..Rates::default()
                },
            }),
            evidence_ids: vec!["e".into()],
            date_basis: "observation".into(),
        }
    }
    fn observed_collection(model: &str) -> Collection {
        Collection {
            observations: vec![crate::pricing_evidence::ObservedRate {
                model: model.into(),
                tier: "standard".into(),
                context: "short".into(),
                rates: [Some(2.0), None, None, Some(10.0)],
                effective_from: None,
                evidence_id: "e".into(),
            }],
            evidence: vec![citation()],
            unresolved: vec![],
            fetched: crate::pricing_evidence::FetchCache::default(),
        }
    }
    #[test]
    fn refresh_preserves_custom_standalone_and_tombstoned_cells() {
        let embedded = Catalog::embedded().unwrap();
        let k = key("2026-09-22");
        let custom = Point {
            key: k.clone(),
            rates: Rates {
                input: Some(0.01),
                ..Rates::default()
            },
            basis: Basis::Custom,
            source: None,
        };
        let cases = [
            Overlay {
                points: vec![custom.clone()],
                ..Overlay::default()
            },
            Overlay {
                base: Some("none".into()),
                points: vec![custom],
                ..Overlay::default()
            },
            Overlay {
                removals: vec![Removal {
                    key: k,
                    basis: Basis::Custom,
                    source: None,
                }],
                ..Overlay::default()
            },
        ];
        for overlay in cases {
            let proposal =
                from_collection(&embedded, &overlay, &observed_collection("gpt-6-sol")).unwrap();
            assert!(proposal.operations.is_empty());
            assert!(!proposal.coverage.is_empty());
            let result = prepare(&embedded, &overlay, &proposal, &BTreeMap::new(), true).unwrap();
            assert_eq!(result.candidate, overlay);
        }
    }
    #[test]
    fn researched_resolution_survives_subsequent_runs_and_new_models() {
        let embedded = Catalog::embedded().unwrap();
        let overlay = Overlay {
            points: vec![
                Point {
                    key: key("2026-10-01"),
                    rates: Rates {
                        input: Some(999.0),
                        ..Rates::default()
                    },
                    basis: Basis::Researched,
                    source: Some("https://openai.com/evidence".into()),
                },
                Point {
                    key: PointKey {
                        model: "researched-new".into(),
                        ..key("2026-10-01")
                    },
                    rates: Rates {
                        input: Some(1.0),
                        ..Rates::default()
                    },
                    basis: Basis::Researched,
                    source: Some("https://openai.com/evidence".into()),
                },
            ],
            ..Overlay::default()
        };
        let proposal = Proposal {
            proposal_schema_version: 1,
            catalog_schema_version: 1,
            operations: vec![],
            evidence: vec![],
            coverage: vec![],
        };
        let first = prepare(&embedded, &overlay, &proposal, &BTreeMap::new(), false).unwrap();
        let next = prepare(
            &embedded,
            &first.candidate,
            &proposal,
            &BTreeMap::new(),
            true,
        )
        .unwrap();
        assert!(first.unresolved.is_empty() && next.unresolved.is_empty());
        assert_eq!(next.candidate, overlay);
    }
    #[test]
    fn existing_short_only_model_refreshes_without_context_threshold() {
        let embedded = Catalog::embedded().unwrap();
        let mut collection = observed_collection("gpt-5.1");
        let mut legend = citation();
        legend.id = crate::pricing_evidence::LEGEND.into();
        legend.excerpt =
            "Short context: ≤272K input tokens. Long context: >272K input tokens.".into();
        collection.evidence.push(legend);
        let mut long = collection.observations[0].clone();
        long.context = "long".into();
        collection.observations.push(long);
        collection.observations[0].rates[0] = Some(999.0);
        let proposal = from_collection(&embedded, &Overlay::default(), &collection).unwrap();
        assert_eq!(proposal.operations.len(), 1);
        assert!(
            proposal
                .coverage
                .iter()
                .any(|s| s.contains("gpt-5.1 context threshold unavailable"))
        );
        let result = prepare(
            &embedded,
            &Overlay::default(),
            &proposal,
            &BTreeMap::from([("e".into(), Ok(()))]),
            true,
        )
        .unwrap();
        assert_eq!(result.candidate.points[0].rates.input, Some(999.0));
    }
    #[test]
    fn imports_preserve_custom_targets_inherited_custom_values_and_standalone() {
        let embedded = Catalog::embedded().unwrap();
        let old = Rates {
            input: Some(0.01),
            ..Rates::default()
        };
        let source = key("2026-09-22");
        let overlay = Overlay {
            points: vec![Point {
                key: source.clone(),
                rates: old,
                basis: Basis::Custom,
                source: None,
            }],
            ..Overlay::default()
        };
        let mut replace = add("edit", "2026-09-22", Value::Rates(old), 2.0);
        replace.kind = Kind::ReplaceValue;
        let mut remove = replace.clone();
        remove.kind = Kind::Remove;
        remove.target = Target::OverlayPoint {
            key: source.clone(),
        };
        remove.replacement = None;
        let mut moved = replace.clone();
        moved.kind = Kind::Move;
        moved.replacement = Some(Replacement::Point {
            key: key("2026-10-01"),
            rates: old,
        });
        let added = add("later", "2026-10-01", Value::Rates(old), 2.0);
        for op in [replace, remove, moved, added] {
            let proposal = Proposal {
                proposal_schema_version: 1,
                catalog_schema_version: 1,
                operations: vec![op],
                evidence: vec![citation()],
                coverage: vec![],
            };
            let result = prepare(
                &embedded,
                &overlay,
                &proposal,
                &BTreeMap::from([("e".into(), Ok(()))]),
                false,
            )
            .unwrap();
            assert_eq!(result.candidate, overlay);
            assert!(
                result
                    .unresolved
                    .iter()
                    .any(|u| u.contains("user-controlled"))
            );
        }
        let standalone = Overlay {
            base: Some("none".into()),
            points: vec![Point {
                key: source,
                rates: old,
                basis: Basis::Researched,
                source: None,
            }],
            ..Overlay::default()
        };
        let proposal = Proposal {
            proposal_schema_version: 1,
            catalog_schema_version: 1,
            operations: vec![add("new", "2026-10-01", Value::Rates(old), 2.0)],
            evidence: vec![citation()],
            coverage: vec![],
        };
        let result = prepare(
            &embedded,
            &standalone,
            &proposal,
            &BTreeMap::from([("e".into(), Ok(()))]),
            false,
        )
        .unwrap();
        assert_eq!(result.candidate, standalone);
        assert!(
            result
                .unresolved
                .iter()
                .any(|u| u.contains("user-controlled"))
        );
    }
    #[test]
    fn imported_removal_needs_verified_replacement_in_same_cell_and_group() {
        let embedded = Catalog::embedded().unwrap();
        let source = key("2026-09-22");
        let old = *embedded.points.get(&source).unwrap();
        let remove = Operation {
            id: "remove".into(),
            group: "repair".into(),
            kind: Kind::Remove,
            target: Target::Point {
                key: source.clone(),
            },
            expected: Value::Rates(old),
            replacement: None,
            evidence_ids: vec!["e".into()],
            date_basis: "researched".into(),
        };
        let mut replacement = add("replace", "2026-10-01", Value::Rates(old), 2.0);
        let mut proposal = Proposal {
            proposal_schema_version: 1,
            catalog_schema_version: 1,
            operations: vec![remove.clone()],
            evidence: vec![citation()],
            coverage: vec![],
        };
        let verified = BTreeMap::from([("e".into(), Ok(()))]);
        let standalone =
            prepare(&embedded, &Overlay::default(), &proposal, &verified, false).unwrap();
        assert!(standalone.candidate.removals.is_empty());
        assert!(!standalone.unresolved.is_empty());
        proposal.operations.push(replacement.clone());
        let separate =
            prepare(&embedded, &Overlay::default(), &proposal, &verified, false).unwrap();
        assert!(separate.candidate.removals.is_empty());
        replacement.group = "repair".into();
        proposal.operations[1] = replacement;
        let supported =
            prepare(&embedded, &Overlay::default(), &proposal, &verified, false).unwrap();
        assert!(supported.candidate.removals.iter().any(|r| r.key == source));
        let failed = prepare(
            &embedded,
            &Overlay::default(),
            &proposal,
            &BTreeMap::new(),
            false,
        )
        .unwrap();
        assert!(failed.candidate.removals.is_empty() && failed.candidate.points.is_empty());
    }
    #[test]
    fn stale_group_does_not_block_independent_observation_and_preserves_history() {
        let embedded = october_catalog();
        let overlay = Overlay::default();
        let old = embedded.effective_rates(&key("2026-11-06")).unwrap();
        let proposal = Proposal {
            proposal_schema_version: 1,
            catalog_schema_version: 1,
            evidence: vec![citation()],
            coverage: vec![],
            operations: vec![
                add("valid", "2026-11-06", Value::Rates(old), 99.0),
                add("stale", "2026-11-07", Value::Absent, 100.0),
            ],
        };
        let verified = BTreeMap::from([("e".into(), Ok(()))]);
        let result = prepare(&embedded, &overlay, &proposal, &verified, true).unwrap();
        assert_eq!(result.unresolved.len(), 1);
        assert_eq!(result.changes.len(), 1);
        let effective = embedded.merge_overlay(&result.candidate).unwrap();
        assert_eq!(effective.effective_rates(&key("2026-11-05")), Some(old));
        assert_eq!(
            effective.effective_rates(&key("2026-11-06")).unwrap().input,
            Some(99.0)
        );
    }
    #[test]
    fn move_removes_embedded_key_and_never_overwrites_destination() {
        let embedded = Catalog::embedded().unwrap();
        let overlay = Overlay::default();
        let source = key("2026-09-22");
        let old = *embedded.points.get(&source).unwrap();
        let dest = key("2026-10-01");
        let op = Operation {
            id: "move".into(),
            group: "move".into(),
            kind: Kind::Move,
            target: Target::Point {
                key: source.clone(),
            },
            expected: Value::Rates(old),
            replacement: Some(Replacement::Point {
                key: dest.clone(),
                rates: old,
            }),
            evidence_ids: vec!["e".into()],
            date_basis: "researched".into(),
        };
        let proposal = Proposal {
            proposal_schema_version: 1,
            catalog_schema_version: 1,
            operations: vec![op],
            evidence: vec![citation()],
            coverage: vec![],
        };
        let verified = BTreeMap::from([("e".into(), Ok(()))]);
        let result = prepare(&embedded, &overlay, &proposal, &verified, false).unwrap();
        assert!(result.candidate.removals.iter().any(|p| p.key == source));
        assert!(result.candidate.points.iter().any(|p| p.key == dest));
        let mut occupied = overlay;
        occupied.points.push(Point {
            key: dest,
            rates: old,
            basis: Basis::Custom,
            source: None,
        });
        let result = prepare(&embedded, &occupied, &proposal, &verified, false).unwrap();
        assert!(!result.unresolved.is_empty());
        assert!(result.changes.is_empty());
    }
    #[test]
    fn missing_evidence_blocks_entire_group_and_malformed_references_fail() {
        let embedded = Catalog::embedded().unwrap();
        let overlay = Overlay::default();
        let old = embedded.effective_rates(&key("2026-11-06")).unwrap();
        let mut first = add("first", "2026-11-06", Value::Rates(old), 33.0);
        let mut second = add("second", "2026-11-07", Value::Rates(old), 44.0);
        first.group = "together".into();
        second.group = "together".into();
        let mut proposal = Proposal {
            proposal_schema_version: 1,
            catalog_schema_version: 1,
            operations: vec![first, second],
            evidence: vec![citation()],
            coverage: vec![],
        };
        let verified = BTreeMap::from([("e".into(), Err("unavailable".into()))]);
        let result = prepare(&embedded, &overlay, &proposal, &verified, false).unwrap();
        assert!(result.candidate.points.is_empty());
        assert_eq!(result.unresolved.len(), 1);
        proposal.operations[0].evidence_ids = vec!["missing".into()];
        assert!(validate_structure(&proposal).is_err());
    }
    #[test]
    fn unrelated_verified_text_cannot_authorize_arbitrary_rates() {
        let embedded = Catalog::embedded().unwrap();
        let overlay = Overlay::default();
        let old = embedded.effective_rates(&key("2026-11-06")).unwrap();
        let op = add("fake", "2026-11-06", Value::Rates(old), 99.0);
        let proposal = Proposal {
            proposal_schema_version: 1,
            catalog_schema_version: 1,
            operations: vec![op],
            evidence: vec![citation()],
            coverage: vec![],
        };
        let result = prepare(
            &embedded,
            &overlay,
            &proposal,
            &BTreeMap::from([("e".into(), Ok(()))]),
            false,
        )
        .unwrap();
        assert!(result.candidate.points.is_empty());
        assert_eq!(result.unresolved.len(), 1);
    }
    #[test]
    fn unknown_context_threshold_defers_both_bands() {
        use crate::pricing_evidence::{Collection, FetchCache, ObservedRate};
        let embedded = Catalog::embedded().unwrap();
        let observations = ["short", "long"]
            .into_iter()
            .map(|context| ObservedRate {
                model: "gpt-new".into(),
                tier: "standard".into(),
                context: context.into(),
                rates: [Some(1.0), Some(0.1), None, Some(5.0)],
                effective_from: None,
                evidence_id: "e".into(),
            })
            .collect();
        let collection = Collection {
            observations,
            evidence: vec![citation()],
            unresolved: vec![],
            fetched: FetchCache::default(),
        };
        let result = from_collection(&embedded, &Overlay::default(), &collection).unwrap();
        assert!(result.operations.is_empty());
        assert!(
            result
                .coverage
                .iter()
                .any(|x| x.contains("gpt-new") && x.contains("threshold"))
        );
    }
    #[test]
    fn covered_generated_conflicts_respect_review_date_and_custom_and_standalone() {
        let embedded = october_catalog();
        let point = |date: &str, basis| Point {
            key: key(date),
            rates: Rates {
                input: Some(999.0),
                ..Rates::default()
            },
            basis,
            source: Some("https://openai.com/x".into()),
        };
        let empty = Proposal {
            proposal_schema_version: 1,
            catalog_schema_version: 1,
            operations: vec![],
            evidence: vec![],
            coverage: vec![],
        };
        let mut overlay = Overlay {
            points: vec![point("2026-10-20", Basis::Observed)],
            ..Overlay::default()
        };
        assert_eq!(
            prepare(&embedded, &overlay, &empty, &BTreeMap::new(), true)
                .unwrap()
                .unresolved
                .len(),
            1
        );
        overlay.points[0].key.effective_from = "2026-11-01".into();
        assert!(
            prepare(&embedded, &overlay, &empty, &BTreeMap::new(), true)
                .unwrap()
                .unresolved
                .is_empty()
        );
        overlay.points[0] = point("2026-10-20", Basis::Custom);
        assert!(
            prepare(&embedded, &overlay, &empty, &BTreeMap::new(), true)
                .unwrap()
                .unresolved
                .is_empty()
        );
        overlay.base = Some("none".into());
        overlay.points[0].basis = Basis::Observed;
        assert!(
            prepare(&embedded, &overlay, &empty, &BTreeMap::new(), true)
                .unwrap()
                .unresolved
                .is_empty()
        );
    }
    #[test]
    fn removing_overlay_point_or_tombstone_reveals_embedded_value() {
        let embedded = Catalog::embedded().unwrap();
        let k = key("2026-09-22");
        let original = *embedded.points.get(&k).unwrap();
        let overridden = Rates {
            input: Some(99.0),
            ..Rates::default()
        };
        let mut overlay = Overlay {
            points: vec![Point {
                key: k.clone(),
                rates: overridden,
                basis: Basis::Researched,
                source: None,
            }],
            ..Overlay::default()
        };
        let op = Operation {
            id: "remove".into(),
            group: "remove".into(),
            kind: Kind::Remove,
            target: Target::OverlayPoint { key: k.clone() },
            expected: Value::Rates(overridden),
            replacement: None,
            evidence_ids: vec!["e".into()],
            date_basis: "researched".into(),
        };
        let mut proposal = Proposal {
            proposal_schema_version: 1,
            catalog_schema_version: 1,
            operations: vec![
                op,
                add("replacement", "2026-10-01", Value::Rates(overridden), 2.0),
            ],
            evidence: vec![citation()],
            coverage: vec![],
        };
        proposal.operations[1].group = "remove".into();
        let verified = BTreeMap::from([("e".into(), Ok(()))]);
        let result = prepare(&embedded, &overlay, &proposal, &verified, false).unwrap();
        assert_eq!(
            embedded
                .merge_overlay(&result.candidate)
                .unwrap()
                .points
                .get(&k),
            Some(&original)
        );
        overlay.points.clear();
        overlay.removals.push(Removal {
            key: k.clone(),
            basis: Basis::Researched,
            source: None,
        });
        proposal.operations[0].target = Target::Tombstone { key: k.clone() };
        proposal.operations[0].expected = Value::Tombstone;
        proposal.operations[1].expected = Value::Absent;
        let result = prepare(&embedded, &overlay, &proposal, &verified, false).unwrap();
        assert_eq!(
            embedded
                .merge_overlay(&result.candidate)
                .unwrap()
                .points
                .get(&k),
            Some(&original)
        );
    }
    #[test]
    fn observations_skip_unchanged_and_report_occupied_day_conflict() {
        use crate::pricing_evidence::{Collection, FetchCache, ObservedRate};
        let embedded = Catalog::embedded().unwrap();
        let overlay = Overlay::default();
        let old = embedded.effective_rates(&key("2026-11-06")).unwrap();
        let mut obs = ObservedRate {
            model: "gpt-6-sol".into(),
            tier: "standard".into(),
            context: "short".into(),
            rates: [
                old.input,
                old.cached_input,
                old.cache_write_input,
                old.output,
            ],
            effective_from: None,
            evidence_id: "e".into(),
        };
        let mut collection = Collection {
            observations: vec![obs.clone()],
            evidence: vec![citation()],
            unresolved: vec![],
            fetched: FetchCache::default(),
        };
        assert!(
            from_collection(&embedded, &overlay, &collection)
                .unwrap()
                .operations
                .is_empty()
        );
        let coverage = from_collection(&embedded, &overlay, &collection)
            .unwrap()
            .coverage;
        assert!(
            coverage
                .iter()
                .any(|item| item.starts_with("gpt-6-sol / fast / short:"))
        );
        assert!(
            !coverage
                .iter()
                .any(|item| item.starts_with("gpt-6-sol / standard / short:"))
        );
        obs.effective_from = Some("2026-09-22".into());
        obs.rates[0] = Some(999.0);
        collection.observations = vec![obs];
        let proposal = from_collection(&embedded, &overlay, &collection).unwrap();
        assert!(proposal.operations.is_empty());
        assert!(proposal.coverage.iter().any(|x| x.contains("same-day")));
    }
}
