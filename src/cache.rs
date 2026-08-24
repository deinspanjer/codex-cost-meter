use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::{Duration as StdDuration, UNIX_EPOCH},
};

use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde::{Deserialize, Serialize};
use time::Duration;

use crate::{
    pricing::Usage,
    rollout::{
        analysis::{AnalysisError, RolloutStats, UsageEvent, analyze, analyze_skipping},
        discovery::{ForkHistoryMode, RolloutKind, RolloutRecord},
    },
};

const CACHE_FILENAME: &str = "codex-cost-meter.sqlite";
const DISCOVERY_VERSION: i64 = 2;
const ANALYSIS_VERSION: i64 = 4;

pub(crate) struct RolloutCache {
    path: PathBuf,
    connection: RefCell<Option<Connection>>,
    open_attempted: RefCell<bool>,
    notices: RefCell<Vec<String>>,
    refresh: bool,
}

#[derive(Serialize, Deserialize)]
struct CachedDiscovery {
    id: String,
    parent_id: Option<String>,
    fork_history_mode: Option<ForkHistoryMode>,
    kind: RolloutKind,
    cwd: Option<String>,
    malformed_lines_skipped: usize,
    oversized_lines_skipped: usize,
}

pub(crate) struct DiscoveryHit {
    pub record: RolloutRecord,
    pub malformed_lines_skipped: usize,
    pub oversized_lines_skipped: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct FileRevision {
    modified_ns: i64,
    size: i64,
}

#[derive(Serialize, Deserialize)]
struct CachedStats {
    known_usage: Usage,
    reasoning_output: u64,
    events: Vec<UsageEvent>,
    unattributed_tokens: u64,
    turns: usize,
    ended_turns: usize,
    duration: Duration,
    turn_durations: HashMap<String, Duration>,
    turn_models: Vec<CachedTurnModel>,
    malformed_lines: usize,
    oversized_lines: usize,
    invalid_usage_records: usize,
    incomplete_usage: bool,
}

#[derive(Serialize, Deserialize)]
struct CachedTurnModel {
    model: String,
    effort: String,
    turns: usize,
}

impl RolloutCache {
    pub(crate) fn open(home: &Path, refresh: bool) -> Self {
        Self {
            path: home.join(CACHE_FILENAME),
            connection: RefCell::new(None),
            open_attempted: RefCell::new(false),
            notices: RefCell::new(Vec::new()),
            refresh,
        }
    }

    pub(crate) fn discovery(&self, path: &Path) -> Option<DiscoveryHit> {
        let key = path.to_string_lossy();
        let json = self.query(|connection| {
            connection
                .query_row(
                    "SELECT discovery_json
                     FROM rollout_cache
                     WHERE rollout_path = ?1 AND discovery_version = ?2",
                    params![key.as_ref(), DISCOVERY_VERSION],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(CacheError::from)
        })??;
        let cached = match serde_json::from_str::<CachedDiscovery>(&json) {
            Ok(cached) => cached,
            Err(error) => {
                self.disable(error);
                return None;
            }
        };
        Some(DiscoveryHit {
            record: RolloutRecord {
                id: cached.id,
                parent_id: cached.parent_id,
                fork_history_mode: cached.fork_history_mode,
                kind: cached.kind,
                cwd: cached.cwd,
                path: path.to_path_buf(),
            },
            malformed_lines_skipped: cached.malformed_lines_skipped,
            oversized_lines_skipped: cached.oversized_lines_skipped,
        })
    }

    pub(crate) fn store_discovery(
        &self,
        record: &RolloutRecord,
        malformed_lines_skipped: usize,
        oversized_lines_skipped: usize,
    ) {
        let cached = CachedDiscovery {
            id: record.id.clone(),
            parent_id: record.parent_id.clone(),
            fork_history_mode: record.fork_history_mode.clone(),
            kind: record.kind.clone(),
            cwd: record.cwd.clone(),
            malformed_lines_skipped,
            oversized_lines_skipped,
        };
        let json = match serde_json::to_string(&cached) {
            Ok(json) => json,
            Err(error) => {
                self.disable(error);
                return;
            }
        };
        let key = record.path.to_string_lossy();
        let _ = self.query(|connection| {
            connection
                .execute(
                    "INSERT INTO rollout_cache (
                        rollout_path, discovery_version, discovery_json
                     ) VALUES (?1, ?2, ?3)
                     ON CONFLICT(rollout_path) DO UPDATE SET
                        discovery_version = excluded.discovery_version,
                        discovery_json = excluded.discovery_json",
                    params![key.as_ref(), DISCOVERY_VERSION, json],
                )
                .map(|_| ())
                .map_err(CacheError::from)
        });
    }

    pub(crate) fn related_discovery_paths(
        &self,
        paths: &[PathBuf],
        selected_ids: &HashSet<String>,
    ) -> Option<Vec<PathBuf>> {
        if paths.is_empty() {
            return Some(Vec::new());
        }
        let originals = paths
            .iter()
            .map(|path| (path.to_string_lossy().into_owned(), path.clone()))
            .collect::<HashMap<_, _>>();
        let keys = originals.keys().collect::<Vec<_>>();
        let mut cached = HashMap::new();
        for keys in keys.chunks(500) {
            let values = std::iter::repeat_n("?", keys.len())
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "SELECT rollout_path, discovery_json
                 FROM rollout_cache
                 WHERE discovery_version = {DISCOVERY_VERSION}
                   AND rollout_path IN ({values})"
            );
            let rows = self.query(|connection| {
                let mut statement = connection.prepare(&sql)?;
                statement
                    .query_map(params_from_iter(keys), |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(CacheError::from)
            })?;
            for (path, json) in rows {
                match serde_json::from_str::<CachedDiscovery>(&json) {
                    Ok(discovery) => {
                        cached.insert(path, discovery);
                    }
                    Err(error) => {
                        self.disable(error);
                        return None;
                    }
                }
            }
        }

        let mut related = paths
            .iter()
            .filter(|path| !cached.contains_key(path.to_string_lossy().as_ref()))
            .cloned()
            .collect::<Vec<_>>();
        let mut selected_ids = selected_ids.clone();
        loop {
            let mut added = false;
            cached.retain(|path, discovery| {
                let include = match &discovery.parent_id {
                    Some(parent_id) => selected_ids.contains(parent_id),
                    None if matches!(discovery.kind, RolloutKind::Root) => {
                        selected_ids.contains(&discovery.id)
                    }
                    None => true,
                };
                if include {
                    selected_ids.insert(discovery.id.clone());
                    if let Some(path) = originals.get(path) {
                        related.push(path.clone());
                    }
                    added = true;
                }
                !include
            });
            if !added {
                break;
            }
        }
        Some(related)
    }

    pub(crate) fn analyze(&self, record: &RolloutRecord) -> Result<RolloutStats, AnalysisError> {
        let before = file_revision(&record.path);
        if !self.refresh
            && let Some(revision) = before
            && let Some(stats) = self.cached_analysis(&record.path, revision)
        {
            return Ok(stats);
        }

        let stats = analyze(record)?;
        let after = file_revision(&record.path);
        if let Some(revision) = before
            && Some(revision) == after
        {
            self.store_analysis(&record.path, revision, &stats);
        }
        Ok(stats)
    }

    pub(crate) fn analyze_with_replay(
        &self,
        record: &RolloutRecord,
        replay_prefix_len: usize,
    ) -> Result<RolloutStats, AnalysisError> {
        if replay_prefix_len == 0 {
            self.analyze(record)
        } else {
            analyze_skipping(record, replay_prefix_len)
        }
    }

    pub(crate) fn take_notices(&self) -> Vec<String> {
        self.notices.take()
    }

    fn cached_analysis(&self, path: &Path, revision: FileRevision) -> Option<RolloutStats> {
        let key = path.to_string_lossy();
        let json = self.query(|connection| {
            connection
                .query_row(
                    "SELECT analysis_json
                     FROM rollout_cache
                     WHERE rollout_path = ?1
                       AND modified_ns = ?2
                       AND file_size = ?3
                       AND analysis_version = ?4",
                    params![
                        key.as_ref(),
                        revision.modified_ns,
                        revision.size,
                        ANALYSIS_VERSION
                    ],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()
                .map(|value| value.flatten())
                .map_err(CacheError::from)
        })??;
        match serde_json::from_str::<CachedStats>(&json) {
            Ok(stats) => Some(stats.into()),
            Err(error) => {
                self.disable(error);
                None
            }
        }
    }

    fn store_analysis(&self, path: &Path, revision: FileRevision, stats: &RolloutStats) {
        let json = match serde_json::to_string(&CachedStats::from(stats)) {
            Ok(json) => json,
            Err(error) => {
                self.disable(error);
                return;
            }
        };
        let key = path.to_string_lossy();
        let _ = self.query(|connection| {
            connection
                .execute(
                    "UPDATE rollout_cache SET
                        modified_ns = ?2,
                        file_size = ?3,
                        analysis_version = ?4,
                        analysis_json = ?5
                     WHERE rollout_path = ?1",
                    params![
                        key.as_ref(),
                        revision.modified_ns,
                        revision.size,
                        ANALYSIS_VERSION,
                        json
                    ],
                )
                .map(|_| ())
                .map_err(CacheError::from)
        });
    }

    fn query<T>(&self, operation: impl FnOnce(&Connection) -> Result<T, CacheError>) -> Option<T> {
        self.ensure_open();
        let result = {
            let connection = self.connection.borrow();
            operation(connection.as_ref()?).map_err(|error| error.to_string())
        };
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.disable(error);
                None
            }
        }
    }

    fn ensure_open(&self) {
        if *self.open_attempted.borrow() {
            return;
        }
        *self.open_attempted.borrow_mut() = true;
        let existed = match fs::symlink_metadata(&self.path) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => true,
            Ok(_) => {
                self.open_error("cache path is not a regular file");
                return;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => {
                self.open_error(error);
                return;
            }
        };
        let connection = match Connection::open(&self.path) {
            Ok(connection) => connection,
            Err(error) => {
                self.open_error(error);
                return;
            }
        };
        if let Err(error) = make_private(&self.path) {
            self.open_error(error);
            return;
        }
        let connection: rusqlite::Result<Connection> = (|| {
            connection.busy_timeout(StdDuration::from_millis(100))?;
            connection.pragma_update(None, "journal_mode", "WAL")?;
            connection.pragma_update(None, "synchronous", "NORMAL")?;
            connection.execute_batch(
                "CREATE TABLE IF NOT EXISTS rollout_cache (
                    rollout_path TEXT PRIMARY KEY,
                    discovery_version INTEGER NOT NULL,
                    discovery_json TEXT NOT NULL,
                    modified_ns INTEGER,
                    file_size INTEGER,
                    analysis_version INTEGER,
                    analysis_json TEXT
                );",
            )?;
            Ok(connection)
        })();
        match connection {
            Ok(connection) => {
                *self.connection.borrow_mut() = Some(connection);
                if !existed {
                    self.notices
                        .borrow_mut()
                        .push(format!("created rollout cache at {}", self.path.display()));
                }
            }
            Err(error) => self.open_error(error),
        }
    }

    fn open_error(&self, error: impl std::fmt::Display) {
        self.notices.borrow_mut().push(format!(
            "rollout cache unavailable at {}: {error}; continuing without cache",
            self.path.display()
        ));
    }

    fn disable(&self, error: impl std::fmt::Display) {
        if self.connection.take().is_some() {
            self.notices.borrow_mut().push(format!(
                "rollout cache error at {}: {error}; continuing without cache",
                self.path.display()
            ));
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
struct CacheError(#[from] rusqlite::Error);

fn file_revision(path: &Path) -> Option<FileRevision> {
    let metadata = fs::metadata(path).ok()?;
    let modified_ns = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos()
        .try_into()
        .ok()?;
    let size = metadata.len().try_into().ok()?;
    Some(FileRevision { modified_ns, size })
}

#[cfg(unix)]
fn make_private(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn make_private(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

impl From<&RolloutStats> for CachedStats {
    fn from(stats: &RolloutStats) -> Self {
        Self {
            known_usage: stats.known_usage,
            reasoning_output: stats.reasoning_output,
            events: stats.events.clone(),
            unattributed_tokens: stats.unattributed_tokens,
            turns: stats.turns,
            ended_turns: stats.ended_turns,
            duration: stats.duration,
            turn_durations: stats.turn_durations.clone(),
            turn_models: stats
                .turn_models
                .iter()
                .map(|((model, effort), turns)| CachedTurnModel {
                    model: model.clone(),
                    effort: effort.clone(),
                    turns: *turns,
                })
                .collect(),
            malformed_lines: stats.malformed_lines,
            oversized_lines: stats.oversized_lines,
            invalid_usage_records: stats.invalid_usage_records,
            incomplete_usage: stats.incomplete_usage,
        }
    }
}

impl From<CachedStats> for RolloutStats {
    fn from(stats: CachedStats) -> Self {
        Self {
            known_usage: stats.known_usage,
            reasoning_output: stats.reasoning_output,
            events: stats.events,
            unattributed_tokens: stats.unattributed_tokens,
            turns: stats.turns,
            ended_turns: stats.ended_turns,
            duration: stats.duration,
            turn_durations: stats.turn_durations,
            turn_models: stats
                .turn_models
                .into_iter()
                .map(|entry| ((entry.model, entry.effort), entry.turns))
                .collect(),
            malformed_lines: stats.malformed_lines,
            oversized_lines: stats.oversized_lines,
            invalid_usage_records: stats.invalid_usage_records,
            incomplete_usage: stats.incomplete_usage,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::RolloutCache;
    use crate::rollout::discovery::{ForkHistoryMode, RolloutKind, RolloutRecord};

    #[test]
    fn unchanged_analysis_round_trips_through_sqlite() {
        let home = TempDir::new().unwrap();
        let path = home.path().join("rollout.jsonl");
        fs::write(
            &path,
            concat!(
                "{\"type\":\"session_meta\",\"timestamp\":\"2026-08-21T00:00:00Z\",\"payload\":{\"id\":\"root\"}}\n",
                "{\"type\":\"turn_context\",\"payload\":{\"model\":\"gpt-5.6-terra\"}}\n",
                "{\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"last_token_usage\":{\"input_tokens\":1,\"total_tokens\":1}}}}\n",
            ),
        )
        .unwrap();
        let record = RolloutRecord {
            id: "root".into(),
            parent_id: None,
            fork_history_mode: Some(ForkHistoryMode::Legacy),
            kind: RolloutKind::Root,
            cwd: None,
            path: path.clone(),
        };
        let cache = RolloutCache::open(home.path(), false);
        cache.store_discovery(&record, 0, 0);
        assert_eq!(
            cache.discovery(&path).unwrap().record.fork_history_mode,
            Some(ForkHistoryMode::Legacy)
        );
        let first = cache.analyze(&record).unwrap();
        let revision = super::file_revision(&path).unwrap();
        let mut sentinel = cache.cached_analysis(&path, revision).unwrap();
        sentinel.known_usage.input = 999;
        cache.store_analysis(&path, revision, &sentinel);

        let hit = cache.analyze(&record).unwrap();
        let replay_adjusted = cache.analyze_with_replay(&record, 1).unwrap();
        let hit_after_replay = cache.analyze(&record).unwrap();
        let refreshed = RolloutCache::open(home.path(), true)
            .analyze(&record)
            .unwrap();

        assert_eq!(hit.known_usage.input, 999);
        assert_eq!(replay_adjusted.known_usage.input, 0);
        assert_eq!(hit_after_replay.known_usage.input, 999);
        assert_eq!(refreshed.known_usage, first.known_usage);
        assert_eq!(refreshed.events.len(), first.events.len());
        assert_eq!(refreshed.turn_models, first.turn_models);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            assert_eq!(
                fs::metadata(home.path().join(super::CACHE_FILENAME))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn cache_open_failure_warns_once_and_analysis_continues() {
        let home = TempDir::new().unwrap();
        fs::create_dir(home.path().join(super::CACHE_FILENAME)).unwrap();
        let path = home.path().join("rollout.jsonl");
        fs::write(
            &path,
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"root\"}}\n",
        )
        .unwrap();
        let record = RolloutRecord {
            id: "root".into(),
            parent_id: None,
            fork_history_mode: None,
            kind: RolloutKind::Root,
            cwd: None,
            path,
        };
        let cache = RolloutCache::open(home.path(), false);

        cache.store_discovery(&record, 0, 0);
        cache.store_discovery(&record, 0, 0);
        assert_eq!(cache.analyze(&record).unwrap().turns, 0);
        let notices = cache.take_notices();
        assert_eq!(notices.len(), 1);
        assert!(notices[0].contains("continuing without cache"));
    }
}
