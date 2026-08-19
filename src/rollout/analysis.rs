use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};
use thiserror::Error;
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    pricing::Usage,
    rollout::discovery::{JsonlReadError, RolloutRecord, read_jsonl},
};

const TOKEN_FIELDS: [&str; 6] = [
    "input_tokens",
    "cached_input_tokens",
    "cache_write_input_tokens",
    "output_tokens",
    "reasoning_output_tokens",
    "total_tokens",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UsageEvent {
    pub model: String,
    pub at: Option<OffsetDateTime>,
    pub usage: Usage,
    pub reasoning_output: u64,
}

#[derive(Debug, Default)]
pub(crate) struct RolloutStats {
    pub known_usage: Usage,
    pub reasoning_output: u64,
    pub events: Vec<UsageEvent>,
    pub unattributed_tokens: u64,
    pub turns: usize,
    pub ended_turns: usize,
    pub duration: Duration,
    pub turn_durations: HashMap<String, Duration>,
    pub turn_models: HashMap<(String, String), usize>,
    pub malformed_lines: usize,
    pub oversized_lines: usize,
    pub invalid_usage_records: usize,
    pub incomplete_usage: bool,
}

#[derive(Debug, Error)]
pub(crate) enum AnalysisError {
    #[error(transparent)]
    Read(#[from] JsonlReadError),
}

#[derive(Clone, Copy)]
struct TokenUsage {
    usage: Usage,
    reasoning_output: u64,
    total_tokens: u64,
}

impl TokenUsage {
    fn from_json(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let mut values = [0; TOKEN_FIELDS.len()];
        for (index, field) in TOKEN_FIELDS.iter().enumerate() {
            if let Some(value) = object.get(*field) {
                values[index] = value.as_u64()?;
            }
        }
        Some(Self {
            usage: Usage {
                input: values[0],
                cached_input: values[1],
                cache_write_input: values[2],
                output: values[3],
            },
            reasoning_output: values[4],
            total_tokens: values[5],
        })
    }

    fn delta(self, previous: Self) -> Self {
        Self {
            usage: Usage {
                input: self.usage.input.saturating_sub(previous.usage.input),
                cached_input: self
                    .usage
                    .cached_input
                    .saturating_sub(previous.usage.cached_input),
                cache_write_input: self
                    .usage
                    .cache_write_input
                    .saturating_sub(previous.usage.cache_write_input),
                output: self.usage.output.saturating_sub(previous.usage.output),
            },
            reasoning_output: self
                .reasoning_output
                .saturating_sub(previous.reasoning_output),
            total_tokens: self.total_tokens.saturating_sub(previous.total_tokens),
        }
    }

    fn reset_from(self, previous: Self) -> bool {
        self.usage.input < previous.usage.input
            || self.usage.cached_input < previous.usage.cached_input
            || self.usage.cache_write_input < previous.usage.cache_write_input
            || self.usage.output < previous.usage.output
            || self.reasoning_output < previous.reasoning_output
            || self.total_tokens < previous.total_tokens
    }

    fn has_tokens(self) -> bool {
        self.usage.input > 0
            || self.usage.cached_input > 0
            || self.usage.cache_write_input > 0
            || self.usage.output > 0
            || self.reasoning_output > 0
    }
}

pub(crate) fn analyze(record: &RolloutRecord) -> Result<RolloutStats, AnalysisError> {
    let mut session_meta_count = 0;
    let mut session_at = None;
    let mut malformed_timestamp = false;
    let mut valid_turns = HashSet::new();
    let mut turn_contexts = HashMap::new();

    read_jsonl(&record.path, |line| {
        let Ok(item) = serde_json::from_slice::<Value>(line) else {
            return;
        };
        match item.get("type").and_then(Value::as_str) {
            Some("session_meta") => {
                session_meta_count += 1;
                match timestamp(&item) {
                    Ok(Some(at)) => session_at = Some(at),
                    Ok(None) => {}
                    Err(()) => malformed_timestamp = true,
                }
            }
            Some("turn_context") => {
                let Some(payload) = item.get("payload").and_then(Value::as_object) else {
                    return;
                };
                let Some(turn_id) = payload.get("turn_id").and_then(Value::as_str) else {
                    return;
                };
                valid_turns.insert(turn_id.to_owned());
                turn_contexts.insert(turn_id.to_owned(), model_context(payload));
            }
            _ => {}
        }
    })?;

    let mut stats = RolloutStats {
        incomplete_usage: malformed_timestamp,
        ..RolloutStats::default()
    };
    let mut starts = HashMap::new();
    let mut started_turns = Vec::new();
    let mut active_turn = None::<String>;
    let mut legacy_model = None::<String>;
    let mut previous_total = None;
    let summary = read_jsonl(&record.path, |line| {
        let Ok(item) = serde_json::from_slice::<Value>(line) else {
            stats.malformed_lines += 1;
            stats.incomplete_usage = true;
            return;
        };
        let Some(payload) = item.get("payload").and_then(Value::as_object) else {
            return;
        };

        if item.get("type").and_then(Value::as_str) == Some("turn_context") {
            if payload.get("turn_id").and_then(Value::as_str).is_some() {
                legacy_model = None;
            } else if session_meta_count == 1 {
                legacy_model = Some(model_context(payload).0);
            }
            return;
        }

        if item.get("type").and_then(Value::as_str) != Some("event_msg") {
            return;
        }
        let Some(event_type) = payload.get("type").and_then(Value::as_str) else {
            return;
        };
        if matches!(event_type, "task_started" | "turn_started") {
            let turn_id = event_turn_id(payload);
            active_turn = turn_id.filter(|id| valid_turns.contains(id));
            if let Some(turn_id) = &active_turn {
                legacy_model = None;
                started_turns.push(turn_id.clone());
                if let Some(at) = timestamp_or_incomplete(&item, &mut stats) {
                    starts.insert(turn_id.clone(), at);
                }
            }
            return;
        }
        if matches!(
            event_type,
            "task_complete" | "turn_complete" | "task_aborted" | "turn_aborted"
        ) {
            let turn_id = event_turn_id(payload).or_else(|| active_turn.clone());
            let ended_at = timestamp_or_incomplete(&item, &mut stats);
            if let Some(turn_id) = &turn_id
                && let Some(started_at) = starts.remove(turn_id)
            {
                stats.ended_turns += 1;
                if let Some(ended_at) = ended_at {
                    let duration = ended_at - started_at;
                    stats.duration += duration;
                    if let Some((model, _)) = turn_contexts.get(turn_id) {
                        *stats.turn_durations.entry(model.clone()).or_default() += duration;
                    }
                }
            }
            if turn_id.as_deref() == active_turn.as_deref() {
                active_turn = None;
            }
            if turn_id.is_none() {
                legacy_model = None;
            }
            return;
        }
        if event_type != "token_count" {
            return;
        }

        let Some(info) = payload.get("info").and_then(Value::as_object) else {
            return;
        };
        let Some(last) = info.get("last_token_usage") else {
            return;
        };
        let Some(last) = TokenUsage::from_json(last) else {
            stats.invalid_usage_records += 1;
            stats.incomplete_usage = true;
            return;
        };
        let total = match info.get("total_token_usage") {
            Some(total) => match TokenUsage::from_json(total) {
                Some(total) => Some(total),
                None => {
                    stats.invalid_usage_records += 1;
                    stats.incomplete_usage = true;
                    return;
                }
            },
            None => None,
        };
        let normalized = match (total, previous_total) {
            (Some(total), Some(previous)) if !total.reset_from(previous) => total.delta(previous),
            _ => last,
        };
        if let Some(total) = total {
            previous_total = Some(total);
        }
        if !normalized.has_tokens() {
            return;
        }

        let Some(model) = active_turn
            .as_ref()
            .and_then(|turn_id| turn_contexts.get(turn_id).map(|(model, _)| model.clone()))
            .or_else(|| legacy_model.clone())
        else {
            if session_meta_count > 1 {
                add_unattributed(&mut stats, normalized.total_tokens);
            }
            stats.incomplete_usage = true;
            return;
        };
        if !add_known_usage(&mut stats, normalized) {
            stats.invalid_usage_records += 1;
            stats.incomplete_usage = true;
            return;
        }
        let at = timestamp_or_incomplete(&item, &mut stats).or(session_at);
        stats.events.push(UsageEvent {
            model,
            at,
            usage: normalized.usage,
            reasoning_output: normalized.reasoning_output,
        });
    })?;

    stats.oversized_lines = summary.oversized_lines_skipped;
    if stats.oversized_lines > 0 {
        stats.incomplete_usage = true;
    }
    stats.turns = started_turns.len();
    for turn_id in started_turns {
        let context = turn_contexts
            .get(&turn_id)
            .cloned()
            .unwrap_or_else(|| ("unknown".into(), "unknown".into()));
        *stats.turn_models.entry(context).or_default() += 1;
    }
    Ok(stats)
}

fn model_context(payload: &Map<String, Value>) -> (String, String) {
    let model = payload
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let effort = payload
        .get("effort")
        .or_else(|| payload.get("reasoning_effort"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    (model, effort)
}

fn event_turn_id(payload: &Map<String, Value>) -> Option<String> {
    payload
        .get("turn_id")
        .or_else(|| payload.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn timestamp(item: &Value) -> Result<Option<OffsetDateTime>, ()> {
    let Some(value) = item.get("timestamp") else {
        return Ok(None);
    };
    let Some(value) = value.as_str() else {
        return Err(());
    };
    OffsetDateTime::parse(value, &Rfc3339)
        .map(Some)
        .map_err(|_| ())
}

fn timestamp_or_incomplete(item: &Value, stats: &mut RolloutStats) -> Option<OffsetDateTime> {
    match timestamp(item) {
        Ok(at) => at,
        Err(()) => {
            stats.incomplete_usage = true;
            None
        }
    }
}

fn add_unattributed(stats: &mut RolloutStats, tokens: u64) {
    if let Some(total) = stats.unattributed_tokens.checked_add(tokens) {
        stats.unattributed_tokens = total;
    } else {
        stats.incomplete_usage = true;
    }
}

fn add_known_usage(stats: &mut RolloutStats, usage: TokenUsage) -> bool {
    let Some(input) = stats.known_usage.input.checked_add(usage.usage.input) else {
        return false;
    };
    let Some(cached_input) = stats
        .known_usage
        .cached_input
        .checked_add(usage.usage.cached_input)
    else {
        return false;
    };
    let Some(cache_write_input) = stats
        .known_usage
        .cache_write_input
        .checked_add(usage.usage.cache_write_input)
    else {
        return false;
    };
    let Some(output) = stats.known_usage.output.checked_add(usage.usage.output) else {
        return false;
    };
    let Some(reasoning_output) = stats.reasoning_output.checked_add(usage.reasoning_output) else {
        return false;
    };
    stats.known_usage = Usage {
        input,
        cached_input,
        cache_write_input,
        output,
    };
    stats.reasoning_output = reasoning_output;
    true
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fs, path::PathBuf};

    use serde_json::{Value, json};
    use tempfile::TempDir;
    use time::macros::datetime;

    use super::{RolloutStats, analyze};
    use crate::pricing::Usage;
    use crate::rollout::discovery::{RolloutKind, RolloutRecord};

    #[derive(Clone, Copy, Default)]
    struct FixtureUsage {
        input: u64,
        cached_input: u64,
        cache_write_input: u64,
        output: u64,
        reasoning_output: u64,
    }

    impl FixtureUsage {
        fn json(self) -> Value {
            json!({
                "input_tokens": self.input,
                "cached_input_tokens": self.cached_input,
                "cache_write_input_tokens": self.cache_write_input,
                "output_tokens": self.output,
                "reasoning_output_tokens": self.reasoning_output,
                "total_tokens": self.input + self.output,
            })
        }
    }

    fn token_event(last: FixtureUsage, total: FixtureUsage) -> Value {
        json!({
            "type": "event_msg",
            "timestamp": "2026-08-13T12:00:00Z",
            "payload": {
                "type": "token_count",
                "info": {
                    "last_token_usage": last.json(),
                    "total_token_usage": total.json(),
                },
            },
        })
    }

    fn analyze_fixture(rows: &[Value]) -> RolloutStats {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("rollout.jsonl");
        fs::write(
            &path,
            rows.iter()
                .map(|row| format!("{row}\n"))
                .collect::<String>(),
        )
        .unwrap();
        analyze(&record(path, RolloutKind::Root)).unwrap()
    }

    fn analyze_lines(lines: &[&str]) -> RolloutStats {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("rollout.jsonl");
        fs::write(&path, lines.join("\n")).unwrap();
        analyze(&record(path, RolloutKind::Root)).unwrap()
    }

    fn record(path: PathBuf, kind: RolloutKind) -> RolloutRecord {
        RolloutRecord {
            id: "rollout".into(),
            parent_id: None,
            kind,
            cwd: None,
            path,
        }
    }

    fn metadata(timestamp: &str) -> Value {
        json!({"type": "session_meta", "timestamp": timestamp, "payload": {"id": "rollout"}})
    }

    fn context(turn_id: Option<&str>, model: &str, effort: &str) -> Value {
        let mut payload = json!({"model": model, "effort": effort});
        if let Some(turn_id) = turn_id {
            payload["turn_id"] = json!(turn_id);
        }
        json!({"type": "turn_context", "payload": payload})
    }

    fn lifecycle(name: &str, turn_id: &str, timestamp: &str) -> Value {
        json!({
            "type": "event_msg",
            "timestamp": timestamp,
            "payload": {"type": name, "turn_id": turn_id},
        })
    }

    fn usage(input: u64, cached_input: u64, output: u64) -> FixtureUsage {
        FixtureUsage {
            input,
            cached_input,
            output,
            ..FixtureUsage::default()
        }
    }

    #[test]
    fn cumulative_usage_becomes_deltas_and_resets_use_last_usage() {
        let stats = analyze_fixture(&[
            metadata("2026-08-13T10:00:00Z"),
            context(None, "gpt-5.6-terra", "high"),
            token_event(usage(100, 20, 5), usage(100, 20, 5)),
            token_event(usage(40, 10, 2), usage(140, 30, 7)),
            token_event(usage(7, 1, 1), usage(7, 1, 1)),
        ]);

        assert_eq!(stats.known_usage.input, 147);
        assert_eq!(stats.known_usage.cached_input, 31);
        assert_eq!(stats.known_usage.output, 8);
    }

    #[test]
    fn attributes_modern_and_legacy_usage_but_marks_ambiguous_history_unattributed() {
        let cases = [
            (
                "modern root",
                vec![
                    metadata("2026-08-13T10:00:00Z"),
                    context(Some("turn-1"), "gpt-5.6-terra", "high"),
                    lifecycle("task_started", "turn-1", "2026-08-13T10:00:01Z"),
                    token_event(usage(3, 1, 4), usage(3, 1, 4)),
                    lifecycle("task_complete", "turn-1", "2026-08-13T10:00:03Z"),
                ],
                7,
                0,
                1,
                1,
            ),
            (
                "guardian child",
                vec![
                    metadata("2026-08-13T10:00:00Z"),
                    context(Some("turn-1"), "gpt-5.6-sol", "medium"),
                    lifecycle("turn_started", "turn-1", "2026-08-13T10:00:01Z"),
                    token_event(usage(8, 2, 1), usage(8, 2, 1)),
                    lifecycle("turn_aborted", "turn-1", "2026-08-13T10:00:02Z"),
                ],
                9,
                0,
                1,
                1,
            ),
            (
                "unambiguous legacy",
                vec![
                    metadata("2026-08-13T10:00:00Z"),
                    context(None, "gpt-5.6-terra", "low"),
                    token_event(usage(6, 2, 3), usage(6, 2, 3)),
                ],
                9,
                0,
                0,
                0,
            ),
            (
                "ambiguous embedded history",
                vec![
                    metadata("2026-08-13T10:00:00Z"),
                    metadata("2026-08-13T10:00:01Z"),
                    context(None, "gpt-5.6-terra", "low"),
                    token_event(usage(6, 2, 3), usage(6, 2, 3)),
                ],
                0,
                9,
                0,
                0,
            ),
        ];

        for (name, rows, expected_tokens, expected_unattributed, expected_turns, expected_ended) in
            cases
        {
            let stats = analyze_fixture(&rows);
            assert_eq!(
                stats.known_usage.input + stats.known_usage.output,
                expected_tokens,
                "{name}"
            );
            assert_eq!(stats.unattributed_tokens, expected_unattributed, "{name}");
            assert_eq!(stats.turns, expected_turns, "{name}");
            assert_eq!(stats.ended_turns, expected_ended, "{name}");
        }
    }

    #[test]
    fn marks_unbound_usage_unattributed_when_embedded_history_also_has_valid_turns() {
        let stats = analyze_fixture(&[
            metadata("2026-08-13T10:00:00Z"),
            metadata("2026-08-13T10:00:01Z"),
            context(Some("turn-1"), "gpt-5.6-terra", "high"),
            token_event(usage(6, 2, 3), usage(6, 2, 3)),
        ]);

        assert_eq!(stats.known_usage, Usage::default());
        assert_eq!(stats.unattributed_tokens, 9);
        assert!(stats.incomplete_usage);
    }

    #[test]
    fn records_turn_durations_and_model_effort_contexts() {
        let stats = analyze_fixture(&[
            metadata("2026-08-13T10:00:00Z"),
            context(Some("a"), "gpt-5.6-terra", "high"),
            context(Some("b"), "gpt-5.6-sol", "low"),
            lifecycle("turn_started", "a", "2026-08-13T10:00:01Z"),
            lifecycle("turn_complete", "a", "2026-08-13T10:00:04Z"),
            lifecycle("task_started", "b", "2026-08-13T10:00:05Z"),
            lifecycle("task_aborted", "b", "2026-08-13T10:00:07Z"),
        ]);

        assert_eq!(stats.turns, 2);
        assert_eq!(stats.ended_turns, 2);
        assert_eq!(stats.duration.whole_seconds(), 5);
        assert_eq!(stats.turn_models.len(), 2);
        assert_eq!(
            stats.turn_models.get(&("gpt-5.6-sol".into(), "low".into())),
            Some(&1)
        );
    }

    #[test]
    fn falls_back_to_session_timestamp_and_keeps_reasoning_inside_output_usage() {
        let stats = analyze_fixture(&[
            metadata("2026-08-13T10:00:00Z"),
            context(None, "gpt-5.6-terra", "high"),
            json!({
                "type": "event_msg",
                "timestamp": "not-a-timestamp",
                "payload": {
                    "type": "token_count",
                    "info": {
                        "last_token_usage": {
                            "input_tokens": 1,
                            "output_tokens": 9,
                            "reasoning_output_tokens": 4,
                            "total_tokens": 10,
                        },
                    },
                },
            }),
        ]);

        assert_eq!(stats.known_usage.output, 9);
        assert_eq!(stats.reasoning_output, 4);
        assert_eq!(stats.events[0].at, Some(datetime!(2026-08-13 10:00 UTC)));
        assert!(stats.incomplete_usage);
    }

    #[test]
    fn absent_event_timestamps_still_fall_back_without_making_usage_incomplete() {
        let stats = analyze_fixture(&[
            metadata("2026-08-13T10:00:00Z"),
            context(None, "gpt-5.6-terra", "high"),
            json!({
                "type": "event_msg",
                "payload": {
                    "type": "token_count",
                    "info": {"last_token_usage": {"input_tokens": 1, "total_tokens": 1}},
                },
            }),
        ]);

        assert_eq!(stats.events[0].at, Some(datetime!(2026-08-13 10:00 UTC)));
        assert!(!stats.incomplete_usage);
    }

    #[test]
    fn malformed_usage_and_json_lines_make_results_incomplete_without_panicking() {
        let stats = analyze_lines(&[
            "not json",
            &metadata("malformed").to_string(),
            &context(None, "gpt-5.6-terra", "high").to_string(),
            &json!({
                "type": "event_msg",
                "payload": {
                    "type": "token_count",
                    "info": {
                        "last_token_usage": {
                            "input_tokens": -1,
                            "output_tokens": "wrong",
                            "total_tokens": 5,
                        },
                    },
                },
            })
            .to_string(),
        ]);

        assert_eq!(stats.known_usage, Usage::default());
        assert_eq!(stats.malformed_lines, 1);
        assert_eq!(stats.invalid_usage_records, 1);
        assert!(stats.incomplete_usage);
    }

    #[test]
    fn turn_models_count_each_started_turn() {
        let stats = analyze_fixture(&[
            metadata("2026-08-13T10:00:00Z"),
            context(Some("turn-1"), "gpt-5.6-terra", "medium"),
            lifecycle("task_started", "turn-1", "2026-08-13T10:00:01Z"),
        ]);

        assert_eq!(
            stats.turn_models,
            HashMap::from([(("gpt-5.6-terra".into(), "medium".into()), 1)])
        );
    }
}
