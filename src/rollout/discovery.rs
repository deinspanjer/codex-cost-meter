use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde_json::Value;
use thiserror::Error;

const MAX_JSONL_RECORD_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RolloutKind {
    Root,
    Subagent,
    CodeReview,
    Compaction,
    MemoryConsolidation,
    SecurityReview,
    Internal(String),
    OtherSubagent(String),
}

#[derive(Clone, Debug)]
pub(crate) struct RolloutRecord {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: RolloutKind,
    pub cwd: Option<String>,
    pub path: PathBuf,
}

#[derive(Debug)]
pub(crate) struct DiscoveryWarning {
    pub path: PathBuf,
    pub error: io::ErrorKind,
}

pub(crate) type DiscoveryError = std::convert::Infallible;

#[derive(Default)]
pub(crate) struct LineReadSummary {
    pub oversized_lines_skipped: usize,
}

#[derive(Debug, Error)]
pub(crate) enum JsonlReadError {
    #[error("could not read {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
}

impl JsonlReadError {
    pub(crate) fn kind(&self) -> io::ErrorKind {
        match self {
            Self::Read { source, .. } => source.kind(),
        }
    }
}

pub(crate) struct RolloutIndex {
    records: HashMap<String, RolloutRecord>,
    children: HashMap<String, Vec<String>>,
    warnings: Vec<DiscoveryWarning>,
    oversized_lines_skipped: usize,
    malformed_lines_skipped: usize,
}

impl RolloutIndex {
    pub(crate) fn build(home: &Path) -> Result<Self, DiscoveryError> {
        let mut candidates = Vec::new();
        let mut warnings = Vec::new();
        let mut oversized_lines_skipped = 0;
        let mut malformed_lines_skipped = 0;

        for root in [home.join("sessions"), home.join("archived_sessions")] {
            scan_root(
                &root,
                &mut candidates,
                &mut warnings,
                &mut oversized_lines_skipped,
                &mut malformed_lines_skipped,
            );
        }

        let records = resolve_duplicates(candidates);
        let mut children: HashMap<String, Vec<String>> = HashMap::new();
        for record in records.values() {
            if let Some(parent_id) = &record.parent_id {
                children
                    .entry(parent_id.clone())
                    .or_default()
                    .push(record.id.clone());
            }
        }
        for child_ids in children.values_mut() {
            child_ids.sort();
        }

        Ok(Self {
            records,
            children,
            warnings,
            oversized_lines_skipped,
            malformed_lines_skipped,
        })
    }

    pub(crate) fn record(&self, id: &str) -> Option<&RolloutRecord> {
        self.records.get(id)
    }

    pub(crate) fn descendants(&self, root_id: &str) -> Option<Vec<String>> {
        self.records.get(root_id)?;
        let mut seen = HashSet::from([root_id.to_owned()]);
        let mut pending = self.children.get(root_id).cloned().unwrap_or_default();
        let mut descendants = Vec::new();
        while let Some(id) = pending.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Some(children) = self.children.get(&id) {
                pending.extend(children.iter().cloned());
            }
            descendants.push(id);
        }
        descendants.sort();
        Some(descendants)
    }

    pub(crate) fn warnings(&self) -> &[DiscoveryWarning] {
        &self.warnings
    }

    pub(crate) fn oversized_lines_skipped(&self) -> usize {
        self.oversized_lines_skipped
    }

    pub(crate) fn malformed_lines_skipped(&self) -> usize {
        self.malformed_lines_skipped
    }
}

fn scan_root(
    root: &Path,
    candidates: &mut Vec<(RolloutRecord, SystemTime)>,
    warnings: &mut Vec<DiscoveryWarning>,
    oversized_lines_skipped: &mut usize,
    malformed_lines_skipped: &mut usize,
) {
    let metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return,
        Err(error) => {
            warnings.push(DiscoveryWarning {
                path: root.to_path_buf(),
                error: error.kind(),
            });
            return;
        }
    };
    if !metadata.is_dir() {
        return;
    }

    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                warnings.push(DiscoveryWarning {
                    path: directory,
                    error: error.kind(),
                });
                continue;
            }
        };
        let mut entries: Vec<_> = entries.collect();
        entries.sort_by_key(|entry| entry.as_ref().ok().map(|entry| entry.file_name()));
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    warnings.push(DiscoveryWarning {
                        path: directory.clone(),
                        error: error.kind(),
                    });
                    continue;
                }
            };
            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => {
                    warnings.push(DiscoveryWarning {
                        path,
                        error: error.kind(),
                    });
                    continue;
                }
            };
            if metadata.is_dir() {
                directories.push(path);
            } else if metadata.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension == "jsonl")
            {
                scan_file(
                    &path,
                    candidates,
                    warnings,
                    oversized_lines_skipped,
                    malformed_lines_skipped,
                );
            }
        }
    }
}

fn scan_file(
    path: &Path,
    candidates: &mut Vec<(RolloutRecord, SystemTime)>,
    warnings: &mut Vec<DiscoveryWarning>,
    oversized_lines_skipped: &mut usize,
    malformed_lines_skipped: &mut usize,
) {
    let modified = match fs::metadata(path).and_then(|metadata| metadata.modified()) {
        Ok(modified) => modified,
        Err(error) => {
            warnings.push(DiscoveryWarning {
                path: path.to_path_buf(),
                error: error.kind(),
            });
            return;
        }
    };
    let mut record = None;
    match read_jsonl(path, |line| match serde_json::from_slice::<Value>(line) {
        Ok(value) => {
            if record.is_none() {
                record = record_from_value(&value, path);
            }
        }
        Err(_) => *malformed_lines_skipped += 1,
    }) {
        Ok(summary) => *oversized_lines_skipped += summary.oversized_lines_skipped,
        Err(error) => warnings.push(DiscoveryWarning {
            path: path.to_path_buf(),
            error: error.kind(),
        }),
    }
    if let Some(record) = record {
        candidates.push((record, modified));
    }
}

pub(crate) fn read_jsonl(
    path: &Path,
    mut visitor: impl FnMut(&[u8]),
) -> Result<LineReadSummary, JsonlReadError> {
    let file = File::open(path).map_err(|source| JsonlReadError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let mut reader = BufReader::new(file);
    let mut line = Vec::with_capacity(MAX_JSONL_RECORD_BYTES + 1);
    let mut oversized = false;
    let mut summary = LineReadSummary::default();

    loop {
        let buffer = reader.fill_buf().map_err(|source| JsonlReadError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        if buffer.is_empty() {
            if !line.is_empty() && !oversized {
                visitor(&line);
            } else if oversized {
                summary.oversized_lines_skipped += 1;
            }
            return Ok(summary);
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let bytes = newline.map_or(buffer.len(), |position| position);
        if !oversized {
            if line.len() + bytes > MAX_JSONL_RECORD_BYTES {
                line.clear();
                oversized = true;
            } else {
                line.extend_from_slice(&buffer[..bytes]);
            }
        }
        let consumed = newline.map_or(buffer.len(), |position| position + 1);
        reader.consume(consumed);
        if newline.is_some() {
            if oversized {
                summary.oversized_lines_skipped += 1;
            } else {
                visitor(&line);
            }
            line.clear();
            oversized = false;
        }
    }
}

fn record_from_value(value: &Value, path: &Path) -> Option<RolloutRecord> {
    if value.get("type")?.as_str()? != "session_meta" {
        return None;
    }
    let payload = value.get("payload")?.as_object()?;
    let id = payload
        .get("id")
        .or_else(|| payload.get("session_id"))?
        .as_str()?
        .to_owned();
    let source = payload.get("source");
    let parent_id = payload
        .get("parent_thread_id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            source?
                .get("subagent")?
                .get("thread_spawn")?
                .get("parent_thread_id")?
                .as_str()
                .map(str::to_owned)
        });
    Some(RolloutRecord {
        id,
        parent_id,
        kind: rollout_kind(source),
        cwd: payload
            .get("cwd")
            .and_then(Value::as_str)
            .map(str::to_owned),
        path: path.to_path_buf(),
    })
}

fn rollout_kind(source: Option<&Value>) -> RolloutKind {
    let Some(source) = source.and_then(Value::as_object) else {
        return RolloutKind::Root;
    };
    if let Some(internal) = source.get("internal").and_then(Value::as_str) {
        return if internal == "memory_consolidation" {
            RolloutKind::MemoryConsolidation
        } else {
            RolloutKind::Internal(internal.to_owned())
        };
    }
    let Some(subagent) = source.get("subagent") else {
        return RolloutKind::Root;
    };
    let Some(subagent) = subagent.as_object() else {
        return RolloutKind::Subagent;
    };
    let Some((kind, label)) = subagent.iter().next() else {
        return RolloutKind::Subagent;
    };
    match kind.as_str() {
        "thread_spawn" => RolloutKind::Subagent,
        "review" => RolloutKind::CodeReview,
        "compact" => RolloutKind::Compaction,
        "memory_consolidation" => RolloutKind::MemoryConsolidation,
        "other" if label.as_str() == Some("guardian") => RolloutKind::SecurityReview,
        _ => RolloutKind::OtherSubagent(label.as_str().unwrap_or(kind).to_owned()),
    }
}

fn resolve_duplicates(
    candidates: Vec<(RolloutRecord, SystemTime)>,
) -> HashMap<String, RolloutRecord> {
    let mut selected: HashMap<String, (RolloutRecord, SystemTime)> = HashMap::new();
    for (record, modified) in candidates {
        let replace = selected
            .get(&record.id)
            .is_none_or(|(previous, previous_modified)| {
                modified > *previous_modified
                    || (modified == *previous_modified && record.path > previous.path)
            });
        if replace {
            selected.insert(record.id.clone(), (record, modified));
        }
    }
    selected
        .into_iter()
        .map(|(id, (record, _))| (id, record))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{Duration, UNIX_EPOCH},
    };

    use serde_json::json;
    use tempfile::TempDir;

    use super::{RolloutIndex, RolloutKind, RolloutRecord, resolve_duplicates};

    fn write_jsonl(home: &TempDir, relative: &str, rows: &[serde_json::Value]) -> PathBuf {
        let path = home.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            rows.iter()
                .map(|row| format!("{row}\n"))
                .collect::<String>(),
        )
        .unwrap();
        path
    }

    fn meta(id: &str, parent: Option<&str>, source: serde_json::Value) -> serde_json::Value {
        let mut payload = json!({"id": id, "source": source, "cwd": "/project"});
        if let Some(parent) = parent {
            payload["parent_thread_id"] = json!(parent);
        }
        json!({"type": "session_meta", "payload": payload})
    }

    #[test]
    fn indexes_active_and_archived_rollouts_with_sorted_descendants() {
        let home = TempDir::new().unwrap();
        write_jsonl(
            &home,
            "sessions/2026/root.jsonl",
            &[meta("root", None, json!("cli"))],
        );
        write_jsonl(
            &home,
            "archived_sessions/child.jsonl",
            &[meta(
                "child",
                Some("root"),
                json!({"subagent": {"other": "guardian"}}),
            )],
        );
        write_jsonl(
            &home,
            "sessions/2026/grandchild.jsonl",
            &[meta(
                "grandchild",
                None,
                json!({"subagent": {"thread_spawn": {"parent_thread_id": "child"}}}),
            )],
        );

        let index = RolloutIndex::build(home.path()).unwrap();
        assert_eq!(
            index.descendants("root").unwrap(),
            vec!["child", "grandchild"]
        );
        assert_eq!(
            index.record("child").unwrap().kind,
            RolloutKind::SecurityReview
        );
    }

    #[test]
    fn accepts_session_id_and_classifies_known_kinds() {
        let home = TempDir::new().unwrap();
        write_jsonl(
            &home,
            "sessions/root.jsonl",
            &[json!({"type": "session_meta", "payload": {"session_id": "root", "source": "cli"}})],
        );
        write_jsonl(
            &home,
            "sessions/review.jsonl",
            &[meta(
                "review",
                None,
                json!({"subagent": {"review": "reviewer"}}),
            )],
        );
        write_jsonl(
            &home,
            "sessions/compact.jsonl",
            &[meta(
                "compact",
                None,
                json!({"subagent": {"compact": "compact"}}),
            )],
        );
        write_jsonl(
            &home,
            "sessions/internal.jsonl",
            &[meta(
                "internal",
                None,
                json!({"internal": "memory_consolidation"}),
            )],
        );

        let index = RolloutIndex::build(home.path()).unwrap();
        assert_eq!(index.record("root").unwrap().kind, RolloutKind::Root);
        assert_eq!(
            index.record("review").unwrap().kind,
            RolloutKind::CodeReview
        );
        assert_eq!(
            index.record("compact").unwrap().kind,
            RolloutKind::Compaction
        );
        assert_eq!(
            index.record("internal").unwrap().kind,
            RolloutKind::MemoryConsolidation
        );
    }

    #[test]
    fn ignores_malformed_and_oversized_records() {
        let home = TempDir::new().unwrap();
        let path = home.path().join("sessions/records.jsonl");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            path,
            format!(
                "not json\n{}\n{}\n",
                meta("valid", None, json!("cli")),
                "x".repeat(16 * 1024 * 1024 + 1)
            ),
        )
        .unwrap();

        let index = RolloutIndex::build(home.path()).unwrap();
        assert!(index.record("valid").is_some());
        assert_eq!(index.oversized_lines_skipped(), 1);
        assert_eq!(index.malformed_lines_skipped(), 1);
    }

    #[test]
    fn refuses_directory_symlinks() {
        let home = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        write_jsonl(
            &outside,
            "hidden.jsonl",
            &[meta("hidden", None, json!("cli"))],
        );
        fs::create_dir_all(home.path().join("sessions")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path(), home.path().join("sessions/link")).unwrap();

        let index = RolloutIndex::build(home.path()).unwrap();
        assert!(index.record("hidden").is_none());
    }

    #[test]
    fn traversal_stops_at_cyclic_parents() {
        let home = TempDir::new().unwrap();
        write_jsonl(
            &home,
            "sessions/a.jsonl",
            &[meta("a", Some("b"), json!("cli"))],
        );
        write_jsonl(
            &home,
            "sessions/b.jsonl",
            &[meta("b", Some("a"), json!("cli"))],
        );

        let index = RolloutIndex::build(home.path()).unwrap();
        assert_eq!(index.descendants("a").unwrap(), vec!["b"]);
    }

    #[test]
    fn duplicate_resolution_uses_synthetic_time_then_path() {
        let record = |path: &str, cwd: &str| RolloutRecord {
            id: "duplicate".into(),
            parent_id: None,
            kind: RolloutKind::Root,
            cwd: Some(cwd.into()),
            path: PathBuf::from(path),
        };

        let selected = resolve_duplicates(vec![
            (record("a.jsonl", "old"), UNIX_EPOCH),
            (
                record("z.jsonl", "new"),
                UNIX_EPOCH + Duration::from_secs(1),
            ),
        ]);
        assert_eq!(selected["duplicate"].cwd.as_deref(), Some("new"));

        let selected = resolve_duplicates(vec![
            (record("a.jsonl", "first"), UNIX_EPOCH),
            (record("z.jsonl", "later"), UNIX_EPOCH),
        ]);
        assert_eq!(selected["duplicate"].cwd.as_deref(), Some("later"));
    }

    #[test]
    fn missing_roots_produce_an_empty_index() {
        let home = TempDir::new().unwrap();
        let index = RolloutIndex::build(home.path()).unwrap();
        assert!(index.record("missing").is_none());
        assert_eq!(index.descendants("missing"), None);
    }
}
