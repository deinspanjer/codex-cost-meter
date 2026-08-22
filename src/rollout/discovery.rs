use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OpenFlags, params_from_iter};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::cache::RolloutCache;

const MAX_JSONL_RECORD_BYTES: usize = 16 * 1024 * 1024;
const INITIAL_JSONL_RECORD_BYTES: usize = 8 * 1024;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
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
    #[cfg(test)]
    pub(crate) fn build(home: &Path) -> Self {
        Self::build_with_progress(home, || {})
    }

    pub(crate) fn build_cached(home: &Path, cache: &RolloutCache) -> Self {
        Self::build_with_cache_progress(home, Some(cache), || {})
    }

    #[cfg(test)]
    pub(crate) fn build_with_progress(home: &Path, mut indexed_file: impl FnMut()) -> Self {
        Self::build_with_cache_progress(home, None, &mut indexed_file)
    }

    pub(crate) fn build_with_cache_progress(
        home: &Path,
        cache: Option<&RolloutCache>,
        mut indexed_file: impl FnMut(),
    ) -> Self {
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
                cache,
                &mut indexed_file,
            );
        }

        Self::from_records(
            resolve_duplicates(candidates),
            warnings,
            oversized_lines_skipped,
            malformed_lines_skipped,
        )
    }

    pub(crate) fn build_for_cached(
        home: &Path,
        ids: &[String],
        cache: Option<&RolloutCache>,
        mut indexed_file: impl FnMut(),
    ) -> Self {
        targeted_paths(home, ids, cache)
            .and_then(|paths| {
                let mut candidates = Vec::new();
                let mut warnings = Vec::new();
                let mut oversized_lines_skipped = 0;
                let mut malformed_lines_skipped = 0;
                for path in &paths {
                    scan_file(
                        path,
                        &mut candidates,
                        &mut warnings,
                        &mut oversized_lines_skipped,
                        &mut malformed_lines_skipped,
                        cache,
                    );
                    indexed_file();
                }
                let records = resolve_duplicates(candidates);
                (warnings.is_empty()
                    && records.len() == paths.len()
                    && ids.iter().all(|id| records.contains_key(id)))
                .then(|| {
                    Self::from_records(
                        records,
                        warnings,
                        oversized_lines_skipped,
                        malformed_lines_skipped,
                    )
                })
            })
            .unwrap_or_else(|| Self::build_with_cache_progress(home, cache, indexed_file))
    }

    fn from_records(
        records: HashMap<String, RolloutRecord>,
        warnings: Vec<DiscoveryWarning>,
        oversized_lines_skipped: usize,
        malformed_lines_skipped: usize,
    ) -> Self {
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
        Self {
            records,
            children,
            warnings,
            oversized_lines_skipped,
            malformed_lines_skipped,
        }
    }

    pub(crate) fn record(&self, id: &str) -> Option<&RolloutRecord> {
        self.records.get(id)
    }

    pub(crate) fn is_root(&self, id: &str) -> bool {
        self.records
            .get(id)
            .is_some_and(|record| matches!(&record.kind, RolloutKind::Root))
    }

    pub(crate) fn roots(&self) -> impl Iterator<Item = &RolloutRecord> {
        self.records
            .values()
            .filter(|record| matches!(record.kind, RolloutKind::Root))
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

fn targeted_paths(
    home: &Path,
    ids: &[String],
    cache: Option<&RolloutCache>,
) -> Option<Vec<PathBuf>> {
    if ids.is_empty() {
        return Some(Vec::new());
    }
    let database = [
        home.join("state_5.sqlite"),
        home.join("sqlite/state_5.sqlite"),
    ]
    .into_iter()
    .find(|path| path.is_file())?;
    let connection =
        Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    let values = std::iter::repeat_n("(?)", ids.len())
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "WITH RECURSIVE selected(id) AS (
             VALUES {values}
             UNION
             SELECT e.child_thread_id
             FROM thread_spawn_edges e
             JOIN selected s ON s.id = e.parent_thread_id
         )
         SELECT DISTINCT t.id, t.rollout_path
         FROM threads t
         JOIN selected s ON s.id = t.id"
    );
    let mut statement = connection.prepare(&sql).ok()?;
    let rows = statement
        .query_map(params_from_iter(ids), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .ok()?;
    let selected = rows.collect::<Result<Vec<_>, _>>().ok()?;
    let selected_ids = ids
        .iter()
        .cloned()
        .chain(selected.iter().map(|(id, _)| id.clone()))
        .collect::<HashSet<_>>();
    let mut paths = selected
        .into_iter()
        .map(|(_, path)| PathBuf::from(path))
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                home.join(path)
            }
        })
        .collect::<Vec<_>>();
    let orphan_paths = connection
        .prepare(
            "SELECT t.rollout_path
             FROM threads t
             LEFT JOIN thread_spawn_edges e ON e.child_thread_id = t.id
             WHERE e.child_thread_id IS NULL
               AND (t.source LIKE '%\"subagent\"%' OR t.source LIKE '%\"internal\"%')",
        )
        .ok()?
        .query_map([], |row| row.get::<_, String>(0))
        .ok()?
        .collect::<Result<Vec<_>, _>>()
        .ok()?
        .into_iter()
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                home.join(path)
            }
        })
        .collect::<Vec<_>>();
    let all_paths = connection
        .prepare("SELECT rollout_path FROM threads")
        .ok()?
        .query_map([], |row| row.get::<_, String>(0))
        .ok()?
        .collect::<Result<Vec<_>, _>>()
        .ok()?
        .into_iter()
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                home.join(path)
            }
        })
        .collect::<HashSet<_>>();
    let mut reconciliation_paths = orphan_paths;
    reconciliation_paths.extend(unindexed_rollout_paths(home, &all_paths)?);
    paths.extend(match cache {
        Some(cache) => cache.related_discovery_paths(&reconciliation_paths, &selected_ids)?,
        None => reconciliation_paths,
    });
    paths.sort();
    paths.dedup();
    Some(paths)
}

pub(crate) fn state_roots(home: &Path, cache: &RolloutCache) -> Option<Vec<RolloutRecord>> {
    let database = [
        home.join("state_5.sqlite"),
        home.join("sqlite/state_5.sqlite"),
    ]
    .into_iter()
    .find(|path| path.is_file())?;
    let connection =
        Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    let mut statement = connection
        .prepare("SELECT id, rollout_path, source, cwd FROM threads")
        .ok()?;
    let rows = statement
        .query_map([], |row| {
            let path = PathBuf::from(row.get::<_, String>(1)?);
            Ok((
                row.get::<_, String>(0)?,
                path,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .ok()?;
    let rows = rows.collect::<Result<Vec<_>, _>>().ok()?;
    let indexed_paths = rows
        .iter()
        .map(|(_, path, _, _)| {
            if path.is_absolute() {
                path.clone()
            } else {
                home.join(path)
            }
        })
        .collect::<HashSet<_>>();
    let mut candidates = rows
        .into_iter()
        .filter(|(_, _, source, _)| source_is_root(source))
        .map(|(id, path, _, cwd)| {
            let path = if path.is_absolute() {
                path
            } else {
                home.join(path)
            };
            let modified = fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH);
            (
                RolloutRecord {
                    id,
                    parent_id: None,
                    kind: RolloutKind::Root,
                    cwd: cwd.filter(|cwd| !cwd.is_empty()),
                    path,
                },
                modified,
            )
        })
        .collect::<Vec<_>>();
    let mut warnings = Vec::new();
    let mut oversized = 0;
    let mut malformed = 0;
    for path in unindexed_rollout_paths(home, &indexed_paths)? {
        scan_file(
            &path,
            &mut candidates,
            &mut warnings,
            &mut oversized,
            &mut malformed,
            Some(cache),
        );
    }
    if !warnings.is_empty() {
        return None;
    }
    let mut roots = resolve_duplicates(candidates)
        .into_values()
        .filter(|record| matches!(record.kind, RolloutKind::Root))
        .collect::<Vec<_>>();
    roots.sort_by(|left, right| left.id.cmp(&right.id));
    Some(roots)
}

fn unindexed_rollout_paths(home: &Path, indexed: &HashSet<PathBuf>) -> Option<Vec<PathBuf>> {
    let mut unindexed = Vec::new();
    for root in [home.join("sessions"), home.join("archived_sessions")] {
        let metadata = match fs::symlink_metadata(&root) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => return None,
        };
        if !metadata.is_dir() {
            continue;
        }
        let mut directories = vec![root];
        while let Some(directory) = directories.pop() {
            for entry in fs::read_dir(directory).ok()? {
                let entry = entry.ok()?;
                let file_type = entry.file_type().ok()?;
                let path = entry.path();
                if file_type.is_dir() {
                    directories.push(path);
                } else if file_type.is_file()
                    && path
                        .extension()
                        .is_some_and(|extension| extension == "jsonl")
                    && !indexed.contains(&path)
                {
                    unindexed.push(path);
                }
            }
        }
    }
    Some(unindexed)
}

fn scan_root(
    root: &Path,
    candidates: &mut Vec<(RolloutRecord, SystemTime)>,
    warnings: &mut Vec<DiscoveryWarning>,
    oversized_lines_skipped: &mut usize,
    malformed_lines_skipped: &mut usize,
    cache: Option<&RolloutCache>,
    indexed_file: &mut impl FnMut(),
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
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => {
                    warnings.push(DiscoveryWarning {
                        path,
                        error: error.kind(),
                    });
                    continue;
                }
            };
            if file_type.is_dir() {
                directories.push(path);
            } else if file_type.is_file()
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
                    cache,
                );
                indexed_file();
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
    cache: Option<&RolloutCache>,
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
    if let Some(hit) = cache.and_then(|cache| cache.discovery(path)) {
        *malformed_lines_skipped += hit.malformed_lines_skipped;
        *oversized_lines_skipped += hit.oversized_lines_skipped;
        candidates.push((hit.record, modified));
        return;
    }
    let malformed_before = *malformed_lines_skipped;
    let oversized_before = *oversized_lines_skipped;
    let mut record = None;
    match read_jsonl_until(path, |line| match serde_json::from_slice::<Value>(line) {
        Ok(value) => {
            record = record_from_value(&value, path);
            record.is_some()
        }
        Err(_) => {
            *malformed_lines_skipped += 1;
            false
        }
    }) {
        Ok(summary) => *oversized_lines_skipped += summary.oversized_lines_skipped,
        Err(error) => warnings.push(DiscoveryWarning {
            path: path.to_path_buf(),
            error: error.kind(),
        }),
    }
    if let Some(record) = record {
        if let Some(cache) = cache {
            cache.store_discovery(
                &record,
                *malformed_lines_skipped - malformed_before,
                *oversized_lines_skipped - oversized_before,
            );
        }
        candidates.push((record, modified));
    }
}

pub(crate) fn read_jsonl(
    path: &Path,
    mut visitor: impl FnMut(&[u8]),
) -> Result<LineReadSummary, JsonlReadError> {
    read_jsonl_until(path, |line| {
        visitor(line);
        false
    })
}

fn read_jsonl_until(
    path: &Path,
    mut visitor: impl FnMut(&[u8]) -> bool,
) -> Result<LineReadSummary, JsonlReadError> {
    let file = File::open(path).map_err(|source| JsonlReadError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let mut reader = BufReader::new(file);
    let mut line = Vec::with_capacity(INITIAL_JSONL_RECORD_BYTES);
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
        let bytes = newline.unwrap_or(buffer.len());
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
            } else if visitor(&line) {
                return Ok(summary);
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
    let (kind, label) = match subagent {
        Value::String(kind) => (kind.as_str(), None),
        Value::Object(subagent) => {
            let Some((kind, label)) = subagent.iter().next() else {
                return RolloutKind::Subagent;
            };
            (kind.as_str(), label.as_str())
        }
        _ => return RolloutKind::Subagent,
    };
    match kind {
        "thread_spawn" => RolloutKind::Subagent,
        "review" => RolloutKind::CodeReview,
        "compact" => RolloutKind::Compaction,
        "memory_consolidation" => RolloutKind::MemoryConsolidation,
        "other" if label == Some("guardian") => RolloutKind::SecurityReview,
        _ => RolloutKind::OtherSubagent(label.unwrap_or(kind).to_owned()),
    }
}

pub(crate) fn source_is_root(source: &str) -> bool {
    serde_json::from_str(source)
        .ok()
        .as_ref()
        .is_none_or(|source| matches!(rollout_kind(Some(source)), RolloutKind::Root))
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

    use rusqlite::{Connection, params};
    use serde_json::json;
    use tempfile::TempDir;

    use super::{
        RolloutIndex, RolloutKind, RolloutRecord, resolve_duplicates, rollout_kind, state_roots,
    };
    use crate::cache::RolloutCache;

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

        let index = RolloutIndex::build(home.path());
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
    fn targeted_index_skips_unrelated_rollouts_and_keeps_all_descendant_sources() {
        let home = TempDir::new().unwrap();
        let root = write_jsonl(
            &home,
            "sessions/root.jsonl",
            &[meta("root", None, json!("cli"))],
        );
        let child = write_jsonl(
            &home,
            "sessions/child.jsonl",
            &[meta(
                "child",
                None,
                json!({"subagent": {"thread_spawn": {"parent_thread_id": "root"}}}),
            )],
        );
        let guardian = write_jsonl(
            &home,
            "archived_sessions/guardian.jsonl",
            &[meta(
                "guardian",
                Some("root"),
                json!({"subagent": {"other": "guardian"}}),
            )],
        );
        let other_guardian = write_jsonl(
            &home,
            "archived_sessions/other-guardian.jsonl",
            &[meta(
                "other-guardian",
                Some("other-root"),
                json!({"subagent": {"other": "guardian"}}),
            )],
        );
        write_jsonl(
            &home,
            "sessions/lagged.jsonl",
            &[meta(
                "lagged",
                Some("root"),
                json!({"subagent": {"other": "security-review"}}),
            )],
        );
        write_jsonl(
            &home,
            "sessions/unindexed-unrelated.jsonl",
            &[meta("unindexed-unrelated", None, json!("cli"))],
        );
        let unrelated = home.path().join("sessions/unrelated.jsonl");
        fs::write(
            &unrelated,
            format!("not json\n{}\n", meta("unrelated", None, json!("cli"))),
        )
        .unwrap();
        let database = Connection::open(home.path().join("state_5.sqlite")).unwrap();
        database
            .execute_batch(
                "CREATE TABLE threads (id TEXT PRIMARY KEY, rollout_path TEXT, source TEXT);
                 CREATE TABLE thread_spawn_edges (
                     parent_thread_id TEXT, child_thread_id TEXT PRIMARY KEY
                 );",
            )
            .unwrap();
        for (id, path, source) in [
            ("root", root, "cli".into()),
            (
                "child",
                child,
                json!({"subagent": {"thread_spawn": {"parent_thread_id": "root"}}}).to_string(),
            ),
            (
                "guardian",
                guardian,
                json!({"subagent": {"other": "guardian"}}).to_string(),
            ),
            (
                "other-guardian",
                other_guardian,
                json!({"subagent": {"other": "guardian"}}).to_string(),
            ),
            ("unrelated", unrelated, "cli".into()),
        ] {
            database
                .execute(
                    "INSERT INTO threads VALUES (?1, ?2, ?3)",
                    params![id, path.to_string_lossy(), source],
                )
                .unwrap();
        }
        database
            .execute(
                "INSERT INTO thread_spawn_edges VALUES ('root', 'child')",
                [],
            )
            .unwrap();

        let cache = RolloutCache::open(home.path(), false);
        let mut cold_indexed = 0;
        let index =
            RolloutIndex::build_for_cached(home.path(), &["root".into()], Some(&cache), || {
                cold_indexed += 1
            });
        let mut warm_indexed = 0;
        let warm =
            RolloutIndex::build_for_cached(home.path(), &["root".into()], Some(&cache), || {
                warm_indexed += 1
            });

        assert_eq!(
            index.descendants("root").unwrap(),
            vec!["child", "guardian", "lagged"]
        );
        assert_eq!(
            index.record("guardian").unwrap().kind,
            RolloutKind::SecurityReview
        );
        assert!(index.record("unrelated").is_none());
        assert!(warm.record("other-guardian").is_none());
        assert!(warm.record("unindexed-unrelated").is_none());
        assert_eq!(index.malformed_lines_skipped(), 0);
        assert_eq!(cold_indexed, 6);
        assert_eq!(warm_indexed, 4);
    }

    #[test]
    fn project_root_hints_include_jsonl_missing_from_sqlite() {
        let home = TempDir::new().unwrap();
        let indexed = write_jsonl(
            &home,
            "sessions/indexed.jsonl",
            &[meta("indexed", None, json!("cli"))],
        );
        write_jsonl(
            &home,
            "sessions/lagged.jsonl",
            &[meta("lagged", None, json!("cli"))],
        );
        let child = write_jsonl(
            &home,
            "sessions/child.jsonl",
            &[meta(
                "child",
                Some("indexed"),
                json!({"subagent": {"thread_spawn": {"parent_thread_id": "indexed"}}}),
            )],
        );
        let database = Connection::open(home.path().join("state_5.sqlite")).unwrap();
        database
            .execute_batch(
                "CREATE TABLE threads (
                    id TEXT PRIMARY KEY, rollout_path TEXT, source TEXT, cwd TEXT
                 );",
            )
            .unwrap();
        database
            .execute(
                "INSERT INTO threads VALUES (?1, ?2, ?3, '/project')",
                params!["indexed", indexed.to_string_lossy(), "cli"],
            )
            .unwrap();
        database
            .execute(
                "INSERT INTO threads VALUES (?1, ?2, ?3, '/project')",
                params![
                    "child",
                    child.to_string_lossy(),
                    json!({"subagent": {"thread_spawn": {"parent_thread_id": "indexed"}}})
                        .to_string()
                ],
            )
            .unwrap();
        let cache = RolloutCache::open(home.path(), false);

        let roots = state_roots(home.path(), &cache).unwrap();
        let ids = roots.into_iter().map(|root| root.id).collect::<Vec<_>>();

        assert_eq!(ids, ["indexed", "lagged"]);
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

        let index = RolloutIndex::build(home.path());
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
    fn classifies_known_string_subagent_kinds() {
        let cases = [
            ("review", RolloutKind::CodeReview),
            ("memory_consolidation", RolloutKind::MemoryConsolidation),
        ];

        for (kind, expected) in cases {
            assert_eq!(
                rollout_kind(Some(&json!({"subagent": kind}))),
                expected,
                "{kind}"
            );
        }
    }

    #[test]
    fn discovery_stops_after_finding_session_metadata() {
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

        let index = RolloutIndex::build(home.path());
        assert!(index.record("valid").is_some());
        assert_eq!(index.oversized_lines_skipped(), 0);
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

        let index = RolloutIndex::build(home.path());
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

        let index = RolloutIndex::build(home.path());
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
        let index = RolloutIndex::build(home.path());
        assert!(index.record("missing").is_none());
        assert_eq!(index.descendants("missing"), None);
    }
}
