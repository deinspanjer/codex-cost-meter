use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, BufRead, BufReader, Read},
    path::{Path, PathBuf},
    time::SystemTime,
};

use rusqlite::{Connection, OpenFlags, params_from_iter};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    cache::RolloutCache,
    date_filter::Filter,
    homes::{HomeId, HomeScan},
};

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

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum ForkHistoryMode {
    Legacy,
    Paginated,
    Other,
}

#[derive(Clone, Debug)]
pub(crate) struct RolloutRecord {
    pub id: String,
    pub parent_id: Option<String>,
    pub fork_history_mode: Option<ForkHistoryMode>,
    pub kind: RolloutKind,
    pub cwd: Option<String>,
    pub path: PathBuf,
    pub page_start: Option<(u64, OffsetDateTime)>,
    pub pages: Vec<RolloutPage>,
    /// The home this copy was read from; set by discovery, never cached.
    pub home: HomeId,
}

#[derive(Clone, Debug)]
pub(crate) struct RolloutPage {
    pub path: PathBuf,
    pub ordinal: u64,
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
    holders: HashMap<String, Vec<HomeId>>,
    warnings: Vec<DiscoveryWarning>,
    oversized_lines_skipped: usize,
    malformed_lines_skipped: usize,
}

#[derive(Default)]
struct Scan {
    candidates: Vec<(RolloutRecord, SystemTime)>,
    warnings: Vec<DiscoveryWarning>,
    oversized_lines_skipped: usize,
    malformed_lines_skipped: usize,
}

impl Scan {
    fn file(&mut self, path: &Path, cache: Option<&RolloutCache>) {
        scan_file(
            path,
            &mut self.candidates,
            &mut self.warnings,
            &mut self.oversized_lines_skipped,
            &mut self.malformed_lines_skipped,
            cache,
        );
    }

    fn home(&mut self, home: HomeScan<'_>, indexed_file: &mut impl FnMut()) {
        for root in [
            home.path.join("sessions"),
            home.path.join("archived_sessions"),
        ] {
            scan_root(
                &root,
                &mut self.candidates,
                &mut self.warnings,
                &mut self.oversized_lines_skipped,
                &mut self.malformed_lines_skipped,
                home.cache,
                indexed_file,
            );
        }
    }

    fn tag(&mut self, start: usize, home: HomeId) {
        for (record, _) in &mut self.candidates[start..] {
            record.home = home;
        }
    }

    fn holders(&self) -> HashMap<String, Vec<HomeId>> {
        let mut holders = HashMap::<String, Vec<HomeId>>::new();
        for (record, _) in &self.candidates {
            let homes = holders.entry(record.id.clone()).or_default();
            if !homes.contains(&record.home) {
                homes.push(record.home);
            }
        }
        for homes in holders.values_mut() {
            homes.sort_unstable();
        }
        holders
    }

    fn index(self) -> RolloutIndex {
        let holders = self.holders();
        RolloutIndex::from_records(
            resolve_duplicates(self.candidates),
            holders,
            self.warnings,
            self.oversized_lines_skipped,
            self.malformed_lines_skipped,
        )
    }
}

impl RolloutIndex {
    #[cfg(test)]
    pub(crate) fn build(home: &Path) -> Self {
        Self::build_homes(
            &[HomeScan {
                id: 0,
                path: home,
                cache: None,
            }],
            || {},
        )
    }

    #[cfg(test)]
    pub(crate) fn build_cached(home: &Path, cache: &RolloutCache) -> Self {
        Self::build_homes(
            &[HomeScan {
                id: 0,
                path: home,
                cache: Some(cache),
            }],
            || {},
        )
    }

    /// Scan every rollout in every home, then select one file per rollout ID.
    pub(crate) fn build_homes(homes: &[HomeScan<'_>], mut indexed_file: impl FnMut()) -> Self {
        let mut scan = Scan::default();
        for home in homes {
            let start = scan.candidates.len();
            scan.home(*home, &mut indexed_file);
            scan.tag(start, home.id);
        }
        scan.index()
    }

    /// Like [`Self::build_homes`], but skip metadata-backed rollouts outside the date filter.
    pub(crate) fn build_corpus(
        homes: &[HomeScan<'_>],
        filter: &Filter,
        mut indexed_file: impl FnMut(),
    ) -> Self {
        let mut scan = Scan::default();
        for home in homes {
            let start = scan.candidates.len();
            match corpus_paths(home.path, filter) {
                Some(paths) => {
                    for path in paths {
                        scan.file(&path, home.cache);
                        indexed_file();
                    }
                }
                None => scan.home(*home, &mut indexed_file),
            }
            scan.tag(start, home.id);
        }
        scan.index()
    }

    /// Index the requested rollouts and their descendants. One home uses its
    /// SQLite projection when that is reliable; several homes always scan.
    pub(crate) fn build_for(
        homes: &[HomeScan<'_>],
        ids: &[String],
        mut indexed_file: impl FnMut(),
    ) -> Self {
        let [home] = homes else {
            return Self::build_homes(homes, indexed_file);
        };
        targeted_paths(home.path, ids, home.cache)
            .and_then(|paths| {
                let mut scan = Scan::default();
                for path in &paths {
                    scan.file(path, home.cache);
                    indexed_file();
                }
                scan.tag(0, home.id);
                let all_paths_resolved = scan.candidates.len() == paths.len();
                let holders = scan.holders();
                let mut records = resolve_duplicates(std::mem::take(&mut scan.candidates));
                retain_workset(&mut records, ids);
                (scan.warnings.is_empty()
                    && all_paths_resolved
                    && ids.iter().all(|id| records.contains_key(id))
                    && replay_parents_present(&records, ids))
                .then(|| {
                    Self::from_records(
                        records,
                        holders,
                        scan.warnings,
                        scan.oversized_lines_skipped,
                        scan.malformed_lines_skipped,
                    )
                })
            })
            .unwrap_or_else(|| Self::build_homes(homes, indexed_file))
    }

    fn from_records(
        records: HashMap<String, RolloutRecord>,
        holders: HashMap<String, Vec<HomeId>>,
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
            holders,
            warnings,
            oversized_lines_skipped,
            malformed_lines_skipped,
        }
    }

    /// Every home that holds a copy of this rollout, in home order.
    pub(crate) fn holders(&self, id: &str) -> &[HomeId] {
        self.holders.get(id).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn record(&self, id: &str) -> Option<&RolloutRecord> {
        self.records.get(id)
    }

    pub(crate) fn is_root(&self, id: &str) -> bool {
        self.records.get(id).is_some_and(|record| {
            record.parent_id.is_none() && matches!(&record.kind, RolloutKind::Root)
        })
    }

    pub(crate) fn roots(&self) -> impl Iterator<Item = &RolloutRecord> {
        self.records
            .values()
            .filter(|record| record.parent_id.is_none() && matches!(record.kind, RolloutKind::Root))
    }

    pub(crate) fn records(&self) -> impl Iterator<Item = &RolloutRecord> {
        self.records.values()
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

/// The rollout directories of one home, under its given and canonical spellings.
struct HomeRoots {
    home: PathBuf,
    directories: Vec<PathBuf>,
}

impl HomeRoots {
    fn new(home: &Path) -> Self {
        let mut bases = vec![home.to_path_buf()];
        bases.extend(fs::canonicalize(home).ok().filter(|path| path != home));
        Self {
            home: home.into(),
            directories: bases
                .iter()
                .flat_map(|base| [base.join("sessions"), base.join("archived_sessions")])
                .collect(),
        }
    }

    /// Resolve a SQLite rollout path, or `None` when it lies outside this home,
    /// as a copied home's metadata can still point into the original home.
    fn resolve(&self, stored: &str) -> Option<PathBuf> {
        let path = PathBuf::from(stored);
        let path = if path.is_absolute() {
            path
        } else {
            self.home.join(path)
        };
        self.directories
            .iter()
            .any(|directory| path.starts_with(directory))
            .then_some(path)
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
    let roots = HomeRoots::new(home);
    // A requested rollout recorded outside this home cannot be read from it.
    let mut paths = selected
        .into_iter()
        .map(|(_, path)| roots.resolve(&path))
        .collect::<Option<Vec<_>>>()?;
    let all_paths = connection
        .prepare("SELECT rollout_path FROM threads")
        .ok()?
        .query_map([], |row| row.get::<_, String>(0))
        .ok()?
        .collect::<Result<Vec<_>, _>>()
        .ok()?
        .into_iter()
        .filter_map(|path| roots.resolve(&path))
        .collect::<HashSet<_>>();
    let mut reconciliation_paths = all_paths.iter().cloned().collect::<Vec<_>>();
    reconciliation_paths.extend(unindexed_rollout_paths(home, &all_paths)?);
    paths.extend(match cache {
        Some(cache) => cache.related_discovery_paths(&reconciliation_paths, &selected_ids)?,
        None => reconciliation_paths,
    });
    paths.sort();
    paths.dedup();
    Some(paths)
}

fn corpus_paths(home: &Path, filter: &Filter) -> Option<Vec<PathBuf>> {
    if !filter.is_filtered() {
        return None;
    }
    let database = [
        home.join("state_5.sqlite"),
        home.join("sqlite/state_5.sqlite"),
    ]
    .into_iter()
    .find(|path| path.is_file())?;
    let connection =
        Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    let mut statement = connection
        .prepare("SELECT rollout_path, created_at, updated_at FROM threads")
        .ok()?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        })
        .ok()?
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let roots = HomeRoots::new(home);
    let mut indexed = HashSet::new();
    let mut paths = Vec::new();
    for (path, created_at, updated_at) in rows {
        // A path outside this home stays unindexed, so the walk below keeps
        // this home's own copy without date pruning.
        let Some(path) = roots.resolve(&path) else {
            continue;
        };
        indexed.insert(path.clone());
        let created_at =
            created_at.and_then(|value| OffsetDateTime::from_unix_timestamp(value).ok());
        let updated_at =
            updated_at.and_then(|value| OffsetDateTime::from_unix_timestamp(value).ok());
        if filter.retains_rollout(created_at, updated_at) {
            paths.push(path);
        }
    }
    paths.extend(unindexed_rollout_paths(home, &indexed)?);
    paths.sort();
    paths.dedup();
    Some(paths)
}

/// Root rollouts sorted by ID. A root without a recorded starting directory
/// takes one from the SQLite projection of a home that holds it.
pub(crate) fn project_roots(index: &RolloutIndex, homes: &[&Path]) -> Vec<RolloutRecord> {
    let mut cwds = vec![None; homes.len()];
    let mut roots = index.roots().cloned().collect::<Vec<_>>();
    for root in &mut roots {
        if root.cwd.is_some() {
            continue;
        }
        root.cwd = index.holders(&root.id).iter().find_map(|home| {
            cwds[*home]
                .get_or_insert_with(|| recorded_cwds(homes[*home]))
                .get(&root.id)
                .cloned()
        });
    }
    roots.sort_by(|left, right| left.id.cmp(&right.id));
    roots
}

/// Nonempty thread starting directories from a home's SQLite projection.
fn recorded_cwds(home: &Path) -> HashMap<String, String> {
    let Some(database) = [
        home.join("state_5.sqlite"),
        home.join("sqlite/state_5.sqlite"),
    ]
    .into_iter()
    .find(|path| path.is_file()) else {
        return HashMap::new();
    };
    let rows = || -> Option<HashMap<String, String>> {
        let connection =
            Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
        connection
            .prepare("SELECT id, cwd FROM threads")
            .ok()?
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })
            .ok()?
            .filter_map(|row| match row {
                Ok((id, Some(cwd))) if !cwd.is_empty() => Some(Ok((id, cwd))),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<HashMap<_, _>, _>>()
            .ok()
    };
    rows().unwrap_or_default()
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
                } else if file_type.is_file() && is_rollout_path(&path) && !indexed.contains(&path)
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
            } else if file_type.is_file() && is_rollout_path(&path) {
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
    // SQLite can retain the original name after Codex compresses an idle rollout.
    let compressed = path.with_extension("jsonl.zst");
    let path = if !path.exists()
        && path.extension().is_some_and(|ext| ext == "jsonl")
        && compressed.is_file()
    {
        compressed.as_path()
    } else {
        path
    };
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

fn is_rollout_path(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "jsonl")
        || path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().ends_with(".jsonl.zst"))
}

/// Read one logical history, retaining normalization and turn state across pages.
pub(crate) fn read_rollout(
    record: &RolloutRecord,
    mut visitor: impl FnMut(&[u8]),
) -> Result<LineReadSummary, JsonlReadError> {
    read_rollout_located(record, |_, _, line| visitor(line))
}

pub(crate) fn read_rollout_located(
    record: &RolloutRecord,
    mut visitor: impl FnMut(&Path, u64, &[u8]),
) -> Result<LineReadSummary, JsonlReadError> {
    if record.pages.is_empty() {
        return read_jsonl_located_until(&record.path, |line, bytes| {
            visitor(&record.path, line, bytes);
            false
        });
    }
    let mut summary = LineReadSummary::default();
    let mut overlap = HashSet::<Value>::new();
    for (index, page) in record.pages.iter().enumerate() {
        let next_start = record.pages[index + 1..]
            .iter()
            .map(|page| page.ordinal)
            .min();
        let mut next_overlap = HashSet::new();
        if let Some(start) = next_start {
            next_overlap.extend(
                overlap
                    .iter()
                    .filter(|row| {
                        row.get("ordinal")
                            .and_then(Value::as_u64)
                            .is_some_and(|n| n >= start)
                    })
                    .cloned(),
            );
        }
        let result = read_jsonl_located_until(&page.path, |line_number, line| {
            let Ok(row) = serde_json::from_slice::<Value>(line) else {
                visitor(&page.path, line_number, line);
                return false;
            };
            // Each page repeats the session header; it is not embedded fork history.
            if index > 0
                && row.get("type").and_then(Value::as_str) == Some("session_meta")
                && row
                    .get("payload")
                    .and_then(|payload| payload.get("id").or_else(|| payload.get("session_id")))
                    .and_then(Value::as_str)
                    == Some(record.id.as_str())
            {
                return false;
            }
            // Ordinals can be reused at page boundaries. Only identical records
            // are replay; distinct records sharing an ordinal must both survive.
            let duplicate = overlap.contains(&row);
            if let Some(start) = next_start
                && row
                    .get("ordinal")
                    .and_then(Value::as_u64)
                    .is_some_and(|n| n >= start)
            {
                next_overlap.insert(row);
            }
            if !duplicate {
                visitor(&page.path, line_number, line);
            }
            false
        })?;
        summary.oversized_lines_skipped += result.oversized_lines_skipped;
        overlap = next_overlap;
    }
    Ok(summary)
}

fn read_jsonl_until(
    path: &Path,
    mut visitor: impl FnMut(&[u8]) -> bool,
) -> Result<LineReadSummary, JsonlReadError> {
    read_jsonl_located_until(path, |_, line| visitor(line))
}

fn read_jsonl_located_until(
    path: &Path,
    mut visitor: impl FnMut(u64, &[u8]) -> bool,
) -> Result<LineReadSummary, JsonlReadError> {
    let file = File::open(path).map_err(|source| JsonlReadError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let source: Box<dyn Read> = if path.extension().is_some_and(|ext| ext == "zst") {
        Box::new(
            zstd::stream::read::Decoder::new(file).map_err(|source| JsonlReadError::Read {
                path: path.to_path_buf(),
                source,
            })?,
        )
    } else {
        Box::new(file)
    };
    let mut reader = BufReader::new(source);
    let mut line = Vec::with_capacity(INITIAL_JSONL_RECORD_BYTES);
    let mut oversized = false;
    let mut line_number = 1;
    let mut summary = LineReadSummary::default();

    loop {
        let buffer = reader.fill_buf().map_err(|source| JsonlReadError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        if buffer.is_empty() {
            if !line.is_empty() && !oversized {
                visitor(line_number, &line);
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
            } else if visitor(line_number, &line) {
                return Ok(summary);
            }
            line.clear();
            oversized = false;
            line_number += 1;
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
    let forked_from_id = payload
        .get("forked_from_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_owned);
    let parent_id = forked_from_id.clone().or_else(|| {
        payload
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
            })
    });
    let fork_history_mode = forked_from_id.map(|_| match payload.get("history_mode") {
        None => ForkHistoryMode::Legacy,
        Some(Value::String(mode)) if mode == "legacy" => ForkHistoryMode::Legacy,
        Some(Value::String(mode)) if mode == "paginated" => ForkHistoryMode::Paginated,
        Some(_) => ForkHistoryMode::Other,
    });
    Some(RolloutRecord {
        id,
        parent_id,
        fork_history_mode,
        kind: rollout_kind(source),
        cwd: payload
            .get("cwd")
            .and_then(Value::as_str)
            .map(str::to_owned),
        path: path.to_path_buf(),
        page_start: value.get("ordinal").and_then(Value::as_u64).zip(
            value
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(|at| OffsetDateTime::parse(at, &Rfc3339).ok()),
        ),
        pages: Vec::new(),
        home: 0,
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
    type PageKey = (String, Option<(u64, OffsetDateTime)>);
    let mut selected: HashMap<PageKey, (RolloutRecord, SystemTime)> = HashMap::new();
    for (record, modified) in candidates {
        let key = (record.id.clone(), record.page_start);
        let replace = selected
            .get(&key)
            .is_none_or(|(previous, previous_modified)| {
                modified > *previous_modified
                    || (modified == *previous_modified && record.path > previous.path)
            });
        if replace {
            selected.insert(key, (record, modified));
        }
    }
    let mut groups: HashMap<String, Vec<(RolloutRecord, SystemTime)>> = HashMap::new();
    for (record, modified) in selected.into_values() {
        groups
            .entry(record.id.clone())
            .or_default()
            .push((record, modified));
    }
    groups
        .into_iter()
        .map(|(id, mut records)| {
            records.sort_by(|(left, _), (right, _)| {
                left.page_start
                    .as_ref()
                    .map(|(_, at)| at)
                    .cmp(&right.page_start.as_ref().map(|(_, at)| at))
                    .then_with(|| left.page_start.cmp(&right.page_start))
            });
            let mut record = records
                .iter()
                .max_by(|(left, lm), (right, rm)| {
                    lm.cmp(rm).then_with(|| left.path.cmp(&right.path))
                })
                .unwrap()
                .0
                .clone();
            if records.len() > 1 {
                record.pages = records
                    .into_iter()
                    .map(|(page, _)| RolloutPage {
                        path: page.path,
                        ordinal: page.page_start.map_or(0, |(ordinal, _)| ordinal),
                    })
                    .collect();
            }
            (id, record)
        })
        .collect()
}

fn retain_workset(records: &mut HashMap<String, RolloutRecord>, root_ids: &[String]) {
    let mut selected = root_ids.iter().cloned().collect::<HashSet<_>>();
    loop {
        let additions = records
            .values()
            .filter(|record| {
                !selected.contains(&record.id)
                    && record
                        .parent_id
                        .as_ref()
                        .is_some_and(|parent_id| selected.contains(parent_id))
            })
            .map(|record| record.id.clone())
            .collect::<Vec<_>>();
        if additions.is_empty() {
            break;
        }
        selected.extend(additions);
    }
    // Keep each requested rollout's parent for fork-replay analysis only;
    // its own descendants are not added to the workset.
    selected.extend(
        root_ids
            .iter()
            .filter_map(|id| records.get(id)?.parent_id.clone()),
    );
    records.retain(|id, _| selected.contains(id));
}

/// A requested legacy fork needs its parent to suppress copied history.
fn replay_parents_present(records: &HashMap<String, RolloutRecord>, ids: &[String]) -> bool {
    ids.iter().filter_map(|id| records.get(id)).all(|record| {
        record.fork_history_mode != Some(ForkHistoryMode::Legacy)
            || record
                .parent_id
                .as_ref()
                .is_none_or(|parent| records.contains_key(parent))
    })
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
        ForkHistoryMode, RolloutIndex, RolloutKind, RolloutRecord, project_roots,
        resolve_duplicates, rollout_kind,
    };
    use crate::{cache::RolloutCache, homes::HomeScan};

    fn scan<'a>(home: &'a TempDir, cache: &'a RolloutCache) -> HomeScan<'a> {
        HomeScan {
            id: 0,
            path: home.path(),
            cache: Some(cache),
        }
    }

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
        let fork = write_jsonl(
            &home,
            "sessions/fork.jsonl",
            &[json!({
                "type": "session_meta",
                "payload": {
                    "id": "fork",
                    "forked_from_id": "root",
                    "history_mode": "legacy",
                    "source": "cli"
                }
            })],
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
            ("fork", fork, "cli".into()),
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
        let index = RolloutIndex::build_for(&[scan(&home, &cache)], &["root".into()], || {
            cold_indexed += 1
        });
        let mut warm_indexed = 0;
        let warm = RolloutIndex::build_for(&[scan(&home, &cache)], &["root".into()], || {
            warm_indexed += 1
        });

        assert_eq!(
            index.descendants("root").unwrap(),
            vec!["child", "fork", "guardian", "lagged"]
        );
        assert_eq!(
            warm.descendants("root").unwrap(),
            vec!["child", "fork", "guardian", "lagged"]
        );
        assert_eq!(
            index.record("guardian").unwrap().kind,
            RolloutKind::SecurityReview
        );
        assert!(index.record("unrelated").is_none());
        assert!(warm.record("other-guardian").is_none());
        assert!(warm.record("unindexed-unrelated").is_none());
        assert_eq!(index.malformed_lines_skipped(), 1);
        assert_eq!(warm.malformed_lines_skipped(), 0);
        assert!(cold_indexed >= warm_indexed);
    }

    fn homes<'a>(homes: &[&'a TempDir]) -> Vec<HomeScan<'a>> {
        homes
            .iter()
            .enumerate()
            .map(|(id, home)| HomeScan {
                id,
                path: home.path(),
                cache: None,
            })
            .collect()
    }

    fn set_modified(path: &std::path::Path, seconds: u64) {
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(seconds))
            .unwrap();
    }

    fn index_threads(home: &TempDir, rows: &[(&str, &std::path::Path)], edges: &[(&str, &str)]) {
        let database = Connection::open(home.path().join("state_5.sqlite")).unwrap();
        database
            .execute_batch(
                "CREATE TABLE threads (id TEXT PRIMARY KEY, rollout_path TEXT);
                 CREATE TABLE thread_spawn_edges (
                     parent_thread_id TEXT, child_thread_id TEXT PRIMARY KEY
                 );",
            )
            .unwrap();
        for (id, path) in rows {
            database
                .execute(
                    "INSERT INTO threads VALUES (?1, ?2)",
                    params![id, path.to_string_lossy()],
                )
                .unwrap();
        }
        for (parent, child) in edges {
            database
                .execute(
                    "INSERT INTO thread_spawn_edges VALUES (?1, ?2)",
                    params![parent, child],
                )
                .unwrap();
        }
    }

    #[test]
    fn a_rollout_copied_into_two_homes_resolves_to_the_newest_copy_in_any_order() {
        let first = TempDir::new().unwrap();
        let second = TempDir::new().unwrap();
        let old = write_jsonl(
            &first,
            "sessions/shared.jsonl",
            &[meta("shared", None, json!("cli"))],
        );
        let new = write_jsonl(
            &second,
            "archived_sessions/shared.jsonl",
            &[meta("shared", None, json!("cli"))],
        );
        set_modified(&old, 1_000);
        set_modified(&new, 2_000);

        for order in [[&first, &second], [&second, &first]] {
            let index = RolloutIndex::build_homes(&homes(&order), || {});
            assert_eq!(index.records().count(), 1);
            assert_eq!(index.record("shared").unwrap().path, new);
            assert_eq!(index.holders("shared"), [0, 1]);
        }
    }

    #[test]
    fn descendants_resolve_across_homes() {
        let first = TempDir::new().unwrap();
        let second = TempDir::new().unwrap();
        let root = write_jsonl(
            &first,
            "sessions/root.jsonl",
            &[meta("root", None, json!("cli"))],
        );
        let child = write_jsonl(
            &first,
            "sessions/child.jsonl",
            &[meta(
                "child",
                Some("root"),
                json!({"subagent": {"other": "worker"}}),
            )],
        );
        write_jsonl(
            &second,
            "sessions/grandchild.jsonl",
            &[meta(
                "grandchild",
                Some("child"),
                json!({"subagent": {"other": "worker"}}),
            )],
        );
        index_threads(
            &first,
            &[("root", &root), ("child", &child)],
            &[("root", "child")],
        );

        let index = RolloutIndex::build_for(&homes(&[&first, &second]), &["root".into()], || {});

        assert_eq!(index.descendants("root").unwrap(), ["child", "grandchild"]);
    }

    #[test]
    fn copied_home_metadata_pointing_elsewhere_reads_only_its_own_files() {
        let original = TempDir::new().unwrap();
        let copy = TempDir::new().unwrap();
        let original_path = write_jsonl(
            &original,
            "sessions/root.jsonl",
            &[
                json!({"type": "session_meta", "payload": {"id": "root", "source": "cli", "cwd": "/original"}}),
            ],
        );
        let copy_path = write_jsonl(
            &copy,
            "sessions/root.jsonl",
            &[meta("root", None, json!("cli"))],
        );
        // The original kept running after the copy was made.
        set_modified(&copy_path, 1_000);
        set_modified(&original_path, 2_000);
        index_threads(&copy, &[("root", &original_path)], &[]);
        let filter = crate::date_filter::Filter::try_from(crate::date_filter::Options {
            since: Some(jiff::civil::date(2000, 1, 1)),
            through: None,
            group_by: Vec::new(),
            include_empty: false,
        })
        .unwrap();

        for index in [
            RolloutIndex::build_for(&homes(&[&copy]), &["root".into()], || {}),
            RolloutIndex::build_corpus(&homes(&[&copy]), &filter, || {}),
        ] {
            let record = index.record("root").unwrap();
            assert!(record.path.starts_with(copy.path()), "{:?}", record.path);
        }
    }

    #[test]
    fn targeted_fork_child_keeps_its_parent_for_replay_only() {
        let home = TempDir::new().unwrap();
        let root = write_jsonl(
            &home,
            "sessions/root.jsonl",
            &[meta("root", None, json!("cli"))],
        );
        let fork = write_jsonl(
            &home,
            "sessions/fork.jsonl",
            &[json!({
                "type": "session_meta",
                "payload": {"id": "fork", "forked_from_id": "root", "source": "cli"}
            })],
        );
        index_threads(&home, &[("root", &root), ("fork", &fork)], &[]);
        let cache = RolloutCache::open(home.path(), false);
        let scan = [scan(&home, &cache)];
        // Warm the discovery cache, so the targeted pass skips unrelated files.
        RolloutIndex::build_for(&scan, &["fork".into()], || {});

        let index = RolloutIndex::build_for(&scan, &["fork".into()], || {});

        assert!(index.record("root").is_some());
        assert_eq!(index.descendants("fork").unwrap(), Vec::<String>::new());
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
        let fork = write_jsonl(
            &home,
            "sessions/fork.jsonl",
            &[json!({
                "type": "session_meta",
                "payload": {
                    "id": "fork",
                    "forked_from_id": "indexed",
                    "history_mode": "legacy",
                    "source": "cli"
                }
            })],
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
                params!["fork", fork.to_string_lossy(), "cli"],
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

        let index = RolloutIndex::build_cached(home.path(), &cache);
        let roots = project_roots(&index, &[home.path()]);
        assert_eq!(roots[0].cwd.as_deref(), Some("/project"));
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
    fn preserves_explicit_fork_history_mode_without_reclassifying_subagents() {
        let home = TempDir::new().unwrap();
        write_jsonl(
            &home,
            "sessions/root.jsonl",
            &[meta("root", None, json!("cli"))],
        );
        for (id, history_mode) in [
            ("default-legacy", None),
            ("legacy", Some("legacy")),
            ("paginated", Some("paginated")),
            ("future", Some("future")),
        ] {
            let mut payload = json!({
                "id": id,
                "forked_from_id": "root",
                "source": "cli",
            });
            if let Some(history_mode) = history_mode {
                payload["history_mode"] = json!(history_mode);
            }
            write_jsonl(
                &home,
                &format!("sessions/{id}.jsonl"),
                &[json!({"type": "session_meta", "payload": payload})],
            );
        }
        write_jsonl(
            &home,
            "sessions/subagent.jsonl",
            &[meta(
                "subagent",
                None,
                json!({"subagent": {"thread_spawn": {"parent_thread_id": "root"}}}),
            )],
        );
        write_jsonl(
            &home,
            "sessions/orphan.jsonl",
            &[json!({
                "type": "session_meta",
                "payload": {"id": "orphan", "forked_from_id": "missing", "source": "cli"},
            })],
        );

        let index = RolloutIndex::build(home.path());

        assert_eq!(
            index.record("default-legacy").unwrap().fork_history_mode,
            Some(ForkHistoryMode::Legacy)
        );
        assert_eq!(
            index.record("legacy").unwrap().fork_history_mode,
            Some(ForkHistoryMode::Legacy)
        );
        assert_eq!(
            index.record("paginated").unwrap().fork_history_mode,
            Some(ForkHistoryMode::Paginated)
        );
        assert_eq!(
            index.record("future").unwrap().fork_history_mode,
            Some(ForkHistoryMode::Other)
        );
        assert_eq!(index.record("subagent").unwrap().fork_history_mode, None);
        assert_eq!(
            index.record("orphan").unwrap().parent_id.as_deref(),
            Some("missing")
        );
        assert_eq!(
            index.descendants("root").unwrap(),
            vec![
                "default-legacy",
                "future",
                "legacy",
                "paginated",
                "subagent"
            ]
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
            fork_history_mode: None,
            kind: RolloutKind::Root,
            cwd: Some(cwd.into()),
            path: PathBuf::from(path),
            page_start: None,
            pages: Vec::new(),
            home: 0,
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
