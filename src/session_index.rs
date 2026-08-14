use std::{collections::HashMap, io, path::Path};

use serde_json::Value;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::rollout::discovery::read_jsonl;

pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) updated_at: OffsetDateTime,
}

pub(crate) struct Snapshot {
    entries: HashMap<String, Entry>,
    malformed_records: usize,
    oversized_records: usize,
    read_error: Option<io::ErrorKind>,
}

impl Snapshot {
    pub(crate) fn load(home: &Path) -> Self {
        let mut snapshot = Self {
            entries: HashMap::new(),
            malformed_records: 0,
            oversized_records: 0,
            read_error: None,
        };
        let result = read_jsonl(&home.join("session_index.jsonl"), |line| {
            let Ok(value) = serde_json::from_slice::<Value>(line) else {
                snapshot.malformed_records += 1;
                return;
            };
            let Some(object) = value.as_object() else {
                snapshot.malformed_records += 1;
                return;
            };
            let Some(id) = object.get("id").and_then(Value::as_str) else {
                return;
            };
            let Some(name) = object.get("thread_name").and_then(Value::as_str) else {
                return;
            };
            if name.is_empty() {
                return;
            }
            let Some(updated_at) = object.get("updated_at").and_then(Value::as_str) else {
                snapshot.malformed_records += 1;
                return;
            };
            let Ok(updated_at) = OffsetDateTime::parse(updated_at, &Rfc3339) else {
                snapshot.malformed_records += 1;
                return;
            };
            if snapshot
                .entries
                .get(id)
                .is_none_or(|entry| updated_at >= entry.updated_at)
            {
                snapshot.entries.insert(
                    id.into(),
                    Entry {
                        name: name.into(),
                        updated_at,
                    },
                );
            }
        });
        match result {
            Ok(summary) => snapshot.oversized_records = summary.oversized_lines_skipped,
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                snapshot.read_error = Some(error.kind());
            }
            Err(_) => {}
        }
        snapshot
    }

    pub(crate) fn entry(&self, id: &str) -> Option<&Entry> {
        self.entries.get(id)
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.malformed_records == 0 && self.oversized_records == 0 && self.read_error.is_none()
    }

    pub(crate) fn malformed_records(&self) -> usize {
        self.malformed_records
    }

    pub(crate) fn oversized_records(&self) -> usize {
        self.oversized_records
    }

    pub(crate) fn read_error(&self) -> Option<io::ErrorKind> {
        self.read_error
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;
    use tempfile::TempDir;

    use super::Snapshot;

    #[test]
    fn retains_the_latest_valid_entry_and_counts_malformed_records() {
        let home = TempDir::new().unwrap();
        assert!(Snapshot::load(home.path()).is_complete());

        let rows = [
            json!({
                "id": "root",
                "thread_name": "oldest",
                "updated_at": "2026-08-13T12:00:00Z",
            })
            .to_string(),
            "not json".into(),
            json!("wrong type").to_string(),
            json!({
                "id": "root",
                "thread_name": "ignored",
                "updated_at": "2026-08-13T12:00:30Z",
                "unknown_field": "ignored",
            })
            .to_string(),
            json!({
                "id": "root",
                "thread_name": "newest",
                "updated_at": "2026-08-13T12:01:00Z",
            })
            .to_string(),
        ];
        fs::write(home.path().join("session_index.jsonl"), rows.join("\n")).unwrap();

        let snapshot = Snapshot::load(home.path());

        assert_eq!(snapshot.entry("root").unwrap().name, "newest");
        assert_eq!(snapshot.malformed_records(), 2);
        assert!(!snapshot.is_complete());
    }
}
