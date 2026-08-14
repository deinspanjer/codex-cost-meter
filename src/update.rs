#![allow(dead_code)] // The v0.2 CLI task consumes this dry-run interface.

use std::{
    collections::{HashMap, HashSet},
    fs::{File, OpenOptions, TryLockError},
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use rusqlite::{Connection, OpenFlags};
use thiserror::Error;
use time::OffsetDateTime;

use crate::{
    pricing::PricingError,
    report::{ReportContext, ReportError},
    session_index::Snapshot,
    title::{TitleError, TitleFormat},
};

const REQUIRED_COLUMNS: [&str; 6] = [
    "id",
    "title",
    "name",
    "history_mode",
    "updated_at",
    "first_user_message",
];

pub(crate) struct UpdateOptions {
    pub(crate) thread_ids: Vec<String>,
    pub(crate) title_matches: Vec<String>,
    pub(crate) idle_minutes: Option<u64>,
    pub(crate) limit: usize,
    pub(crate) max_runtime: Option<Duration>,
    pub(crate) reprice_before: Option<OffsetDateTime>,
    pub(crate) apply: bool,
    pub(crate) title_format: TitleFormat,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ProposedUpdate {
    pub(crate) id: String,
    pub(crate) old_title: String,
    pub(crate) new_title: String,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct UpdateResult {
    pub(crate) proposals: Vec<ProposedUpdate>,
}

#[derive(Debug, Error)]
pub(crate) enum UpdateError {
    #[error("another title updater is already running")]
    LockBusy,
    #[error("could not open updater lock {path}: {source}")]
    Lock {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("state database not found under {home}")]
    DatabaseNotFound { home: String },
    #[error("could not open or read state database: {source}")]
    Database {
        #[source]
        source: rusqlite::Error,
    },
    #[error("threads schema is missing required column: {column}")]
    Schema { column: String },
    #[error("session index could not be read")]
    SessionIndexUnreadable,
    #[error("root thread id not found: {id}")]
    RootNotFound { id: String },
    #[error("title substring {query:?} resolved to {matches}")]
    TitleMatch { query: String, matches: String },
    #[error("SQLite writes are not implemented yet")]
    ApplyUnsupported,
    #[error(transparent)]
    Pricing(#[from] PricingError),
    #[error(transparent)]
    Report(#[from] ReportError),
    #[error(transparent)]
    Title(#[from] TitleError),
}

struct ThreadRow {
    id: String,
    title: Option<String>,
    name: Option<String>,
    history_mode: Option<String>,
    updated_at: i64,
    first_user_message: Option<String>,
}

pub(crate) fn run(home: &Path, options: &UpdateOptions) -> Result<UpdateResult, UpdateError> {
    let _lock = acquire_lock(home)?;
    let connection = open_database(home, options.apply)?;
    validate_schema(&connection)?;
    let rows = read_rows(&connection)?;
    let context = ReportContext::new(home)?;
    let snapshot = context.session_index();
    if snapshot.read_error().is_some() {
        return Err(UpdateError::SessionIndexUnreadable);
    }
    let selected = select_rows(&rows, &context, snapshot, options)?;
    let deadline = options
        .max_runtime
        .map(|duration| Instant::now() + duration);
    let mut proposals = Vec::with_capacity(selected.len());
    for row in selected {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            break;
        }
        let old_title = base_name(row, snapshot);
        let report = context.build(&row.id)?;
        let new_title = options.title_format.compose(&old_title, &report.tree)?;
        proposals.push(ProposedUpdate {
            id: row.id.clone(),
            old_title,
            new_title,
        });
    }
    if options.apply {
        return Err(UpdateError::ApplyUnsupported);
    }
    Ok(UpdateResult { proposals })
}

fn acquire_lock(home: &Path) -> Result<File, UpdateError> {
    let path = home.join("thread-cost-title-updater.lock");
    let lock = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|source| UpdateError::Lock {
            path: path.clone(),
            source,
        })?;
    lock.try_lock().map_err(|source| match source {
        TryLockError::WouldBlock => UpdateError::LockBusy,
        TryLockError::Error(source) => UpdateError::Lock { path, source },
    })?;
    Ok(lock)
}

fn open_database(home: &Path, apply: bool) -> Result<Connection, UpdateError> {
    let path = [
        home.join("state_5.sqlite"),
        home.join("sqlite/state_5.sqlite"),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .ok_or_else(|| UpdateError::DatabaseNotFound {
        home: home.display().to_string(),
    })?;
    let flags = if apply {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    } else {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    };
    let connection = Connection::open_with_flags(path, flags)
        .map_err(|source| UpdateError::Database { source })?;
    connection
        .busy_timeout(Duration::from_secs(1))
        .map_err(|source| UpdateError::Database { source })?;
    Ok(connection)
}

fn validate_schema(connection: &Connection) -> Result<(), UpdateError> {
    let mut statement = connection
        .prepare("PRAGMA table_info(threads)")
        .map_err(|source| UpdateError::Database { source })?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|source| UpdateError::Database { source })?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|source| UpdateError::Database { source })?;
    for column in REQUIRED_COLUMNS {
        if !columns.contains(column) {
            return Err(UpdateError::Schema {
                column: column.into(),
            });
        }
    }
    Ok(())
}

fn read_rows(connection: &Connection) -> Result<Vec<ThreadRow>, UpdateError> {
    let mut statement = connection
        .prepare(
            "SELECT id, title, name, history_mode, updated_at, first_user_message FROM threads",
        )
        .map_err(|source| UpdateError::Database { source })?;
    statement
        .query_map([], |row| {
            Ok(ThreadRow {
                id: row.get(0)?,
                title: row.get(1)?,
                name: row.get(2)?,
                history_mode: row.get(3)?,
                updated_at: row.get(4)?,
                first_user_message: row.get(5)?,
            })
        })
        .map_err(|source| UpdateError::Database { source })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| UpdateError::Database { source })
}

fn select_rows<'a>(
    rows: &'a [ThreadRow],
    context: &ReportContext,
    snapshot: &Snapshot,
    options: &UpdateOptions,
) -> Result<Vec<&'a ThreadRow>, UpdateError> {
    let roots = rows
        .iter()
        .filter(|row| context.is_root(&row.id))
        .collect::<Vec<_>>();
    if options.idle_minutes.is_some() {
        return Ok(select_idle(roots, snapshot, options));
    }
    let by_id = roots
        .iter()
        .map(|row| (row.id.as_str(), *row))
        .collect::<HashMap<_, _>>();
    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    for id in &options.thread_ids {
        let row = by_id
            .get(id.as_str())
            .copied()
            .ok_or_else(|| UpdateError::RootNotFound { id: id.clone() })?;
        if seen.insert(row.id.as_str()) {
            selected.push(row);
        }
    }
    for query in &options.title_matches {
        let matches = roots
            .iter()
            .copied()
            .filter(|row| matches_title(row, snapshot, query))
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(UpdateError::TitleMatch {
                query: query.clone(),
                matches: if matches.is_empty() {
                    "none".into()
                } else {
                    matches
                        .iter()
                        .map(|row| row.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                },
            });
        }
        let row = matches[0];
        if seen.insert(row.id.as_str()) {
            selected.push(row);
        }
    }
    Ok(selected)
}

fn select_idle<'a>(
    mut roots: Vec<&'a ThreadRow>,
    snapshot: &Snapshot,
    options: &UpdateOptions,
) -> Vec<&'a ThreadRow> {
    let cutoff = OffsetDateTime::now_utc().unix_timestamp()
        - i64::try_from(options.idle_minutes.unwrap_or_default().saturating_mul(60))
            .unwrap_or(i64::MAX);
    roots.sort_by_key(|row| std::cmp::Reverse(row.updated_at));
    roots
        .into_iter()
        .filter(|row| row.updated_at <= cutoff)
        .filter(|row| needs_update(row, snapshot, options))
        .take(options.limit)
        .collect()
}

fn needs_update(row: &ThreadRow, snapshot: &Snapshot, options: &UpdateOptions) -> bool {
    let Some(entry) = snapshot.entry(&row.id) else {
        return true;
    };
    let Some(task_updated_at) = OffsetDateTime::from_unix_timestamp(row.updated_at).ok() else {
        return true;
    };
    !(options.title_format.matches_suffix(&entry.name)
        && entry.name.chars().count() <= options.title_format.width()
        && entry.updated_at >= task_updated_at
        && options
            .reprice_before
            .is_none_or(|cutoff| entry.updated_at >= cutoff))
}

fn matches_title(row: &ThreadRow, snapshot: &Snapshot, query: &str) -> bool {
    let query = query.to_lowercase();
    [
        row.title.as_deref(),
        row.name.as_deref(),
        snapshot.entry(&row.id).map(|entry| entry.name.as_str()),
    ]
    .into_iter()
    .flatten()
    .any(|value| value.to_lowercase().contains(&query))
}

fn base_name(row: &ThreadRow, snapshot: &Snapshot) -> String {
    let title = nonblank(row.title.as_deref());
    let prompt = nonblank(row.first_user_message.as_deref());
    if row.history_mode.as_deref() == Some("paginated") {
        if let Some(name) = nonblank(row.name.as_deref()) {
            return name.into();
        }
        return normalize_whitespace(prompt.or(title).unwrap_or("Untitled"));
    }
    if let Some(title) = title.filter(|title| Some(*title) != prompt) {
        return title.into();
    }
    if let Some(name) = snapshot
        .entry(&row.id)
        .and_then(|entry| nonblank(Some(&entry.name)))
    {
        return name.into();
    }
    normalize_whitespace(prompt.or(title).unwrap_or("Untitled"))
}

fn nonblank(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use std::{fs, time::Duration};

    use rusqlite::{Connection, params};
    use serde_json::json;
    use tempfile::TempDir;
    use time::{Duration as TimeDuration, OffsetDateTime, format_description::well_known::Rfc3339};

    use super::{UpdateError, UpdateOptions, run};
    use crate::title::{MetricList, TitleFormat};

    const REQUIRED: [&str; 6] = [
        "id",
        "title",
        "name",
        "history_mode",
        "updated_at",
        "first_user_message",
    ];

    fn format(width: usize) -> TitleFormat {
        TitleFormat::new(width, "total-tokens".parse::<MetricList>().unwrap())
    }

    fn options() -> UpdateOptions {
        UpdateOptions {
            thread_ids: Vec::new(),
            title_matches: Vec::new(),
            idle_minutes: None,
            limit: 20,
            max_runtime: None,
            reprice_before: None,
            apply: false,
            title_format: format(65),
        }
    }

    fn database(home: &TempDir, nested: bool, columns: &[&str]) -> std::path::PathBuf {
        let path = if nested {
            home.path().join("sqlite/state_5.sqlite")
        } else {
            home.path().join("state_5.sqlite")
        };
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let connection = Connection::open(&path).unwrap();
        let definitions = columns
            .iter()
            .map(|column| match *column {
                "id" => "id TEXT PRIMARY KEY".to_owned(),
                "title" | "name" | "history_mode" | "first_user_message" => {
                    format!("{column} TEXT")
                }
                "updated_at" => "updated_at INTEGER".to_owned(),
                _ => format!("{column} TEXT"),
            })
            .collect::<Vec<_>>()
            .join(", ");
        connection
            .execute_batch(&format!("CREATE TABLE threads ({definitions})"))
            .unwrap();
        path
    }

    fn insert(
        path: &std::path::Path,
        id: &str,
        title: Option<&str>,
        name: Option<&str>,
        history_mode: &str,
        updated_at: i64,
        first_user_message: Option<&str>,
    ) {
        Connection::open(path)
            .unwrap()
            .execute(
                "INSERT INTO threads (id, title, name, history_mode, updated_at, first_user_message) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, title, name, history_mode, updated_at, first_user_message],
            )
            .unwrap();
    }

    fn rollout(home: &TempDir, id: &str, parent: Option<&str>) {
        let path = home.path().join("sessions").join(format!("{id}.jsonl"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut payload = json!({"id": id, "source": "cli", "cwd": "/project"});
        if let Some(parent) = parent {
            payload["parent_thread_id"] = json!(parent);
            payload["source"] = json!({"subagent": {"thread_spawn": {"parent_thread_id": parent}}});
        }
        fs::write(
            path,
            format!(
                "{}\n{}\n{}\n",
                json!({"type": "session_meta", "timestamp": "2026-08-13T12:00:00Z", "payload": payload}),
                json!({"type": "turn_context", "payload": {"model": "gpt-5.6-terra", "effort": "high"}}),
                json!({"type": "event_msg", "timestamp": "2026-08-13T12:00:00Z", "payload": {"type": "token_count", "info": {"last_token_usage": {"input_tokens": 100, "total_tokens": 100}}}}),
            ),
        )
        .unwrap();
    }

    fn index(home: &TempDir, rows: &[(&str, &str, OffsetDateTime)]) {
        fs::write(
            home.path().join("session_index.jsonl"),
            rows.iter()
                .map(|(id, name, updated_at)| {
                    format!(
                        "{}\n",
                        json!({"id": id, "thread_name": name, "updated_at": updated_at.format(&Rfc3339).unwrap()})
                    )
                })
                .collect::<String>(),
        )
        .unwrap();
    }

    fn proposal_ids(result: &super::UpdateResult) -> Vec<&str> {
        result
            .proposals
            .iter()
            .map(|proposal| proposal.id.as_str())
            .collect()
    }

    #[test]
    fn accepts_required_schema_regardless_of_unrelated_changes_and_database_location() {
        for nested in [false, true] {
            let home = TempDir::new().unwrap();
            let mut columns = REQUIRED.to_vec();
            columns.push("extra");
            let database = database(&home, nested, &columns);
            let connection = Connection::open(&database).unwrap();
            connection
                .execute_batch("CREATE TABLE unrelated (value TEXT); PRAGMA user_version = 999")
                .unwrap();
            insert(
                &database,
                "root",
                Some("Stored"),
                Some("Sidebar"),
                "paginated",
                0,
                Some("Prompt"),
            );
            rollout(&home, "root", None);
            let mut options = options();
            options.thread_ids = vec!["root".into()];

            let result = run(home.path(), &options).unwrap();

            assert_eq!(proposal_ids(&result), ["root"]);
            assert!(!options.apply);
        }
    }

    #[test]
    fn rejects_each_missing_required_column_and_a_non_sqlite_file() {
        for missing in REQUIRED {
            let home = TempDir::new().unwrap();
            let columns = REQUIRED
                .iter()
                .copied()
                .filter(|column| *column != missing)
                .collect::<Vec<_>>();
            let database = database(&home, false, &columns);
            rollout(&home, "root", None);
            let mut options = options();
            options.thread_ids = vec!["root".into()];

            let error = run(home.path(), &options).unwrap_err();

            assert!(
                matches!(error, UpdateError::Schema { .. }),
                "{missing}: {error}"
            );
            drop(database);
        }
        let home = TempDir::new().unwrap();
        fs::write(home.path().join("state_5.sqlite"), b"not sqlite").unwrap();
        let error = run(home.path(), &options()).unwrap_err();
        assert!(matches!(error, UpdateError::Database { .. }));
    }

    #[test]
    fn chooses_paginated_and_legacy_base_names_in_adr_order() {
        let home = TempDir::new().unwrap();
        let database = database(&home, false, &REQUIRED);
        let now = OffsetDateTime::now_utc().unix_timestamp() - 600;
        for (id, title, name, mode, prompt) in [
            (
                "paginated",
                "SQLite title",
                Some(" SQLite name "),
                "paginated",
                "Prompt",
            ),
            ("legacy-title", " SQLite title ", None, "legacy", "Prompt"),
            ("legacy-index", "Prompt", None, "legacy", "Prompt"),
            (
                "paginated-blank",
                "SQLite title",
                None,
                "paginated",
                "  Prompt   words ",
            ),
            (
                "fallback",
                "  Prompt   words ",
                None,
                "legacy",
                "  Prompt   words ",
            ),
        ] {
            insert(&database, id, Some(title), name, mode, now, Some(prompt));
            rollout(&home, id, None);
        }
        index(
            &home,
            &[
                ("legacy-index", "Index name", OffsetDateTime::now_utc()),
                (
                    "paginated-blank",
                    "Stale index name",
                    OffsetDateTime::now_utc(),
                ),
            ],
        );
        let mut options = options();
        options.thread_ids = vec![
            "paginated".into(),
            "legacy-title".into(),
            "legacy-index".into(),
            "paginated-blank".into(),
            "fallback".into(),
        ];

        let result = run(home.path(), &options).unwrap();

        assert_eq!(
            result
                .proposals
                .iter()
                .map(|proposal| proposal.old_title.as_str())
                .collect::<Vec<_>>(),
            [
                "SQLite name",
                "SQLite title",
                "Index name",
                "Prompt words",
                "Prompt words",
            ]
        );
    }

    #[test]
    fn explicit_selection_matches_all_name_sources_deduplicates_and_rejects_non_unique_or_non_root_ids()
     {
        let home = TempDir::new().unwrap();
        let database = database(&home, false, &REQUIRED);
        let now = OffsetDateTime::now_utc().unix_timestamp() - 600;
        for (id, title, name) in [
            ("title", "Title only", None),
            ("name", "Other", Some("Name match")),
            ("index", "Other", None),
            ("first", "Shared", None),
            ("second", "Shared", None),
            ("child", "Child", None),
        ] {
            insert(
                &database,
                id,
                Some(title),
                name,
                "legacy",
                now,
                Some("Prompt"),
            );
            rollout(&home, id, (id == "child").then_some("title"));
        }
        index(
            &home,
            &[("index", "Index match", OffsetDateTime::now_utc())],
        );
        let mut selected_options = options();
        selected_options.thread_ids = vec!["name".into(), "name".into()];
        selected_options.title_matches = vec![
            "title only".into(),
            "name match".into(),
            "index match".into(),
        ];
        assert_eq!(
            proposal_ids(&run(home.path(), &selected_options).unwrap()),
            ["name", "title", "index"]
        );

        let mut ambiguous_options = options();
        ambiguous_options.title_matches = vec!["shared".into()];
        assert!(matches!(
            run(home.path(), &ambiguous_options),
            Err(UpdateError::TitleMatch { .. })
        ));
        let mut absent_options = options();
        absent_options.title_matches = vec!["absent".into()];
        assert!(matches!(
            run(home.path(), &absent_options),
            Err(UpdateError::TitleMatch { .. })
        ));
        let mut child_options = options();
        child_options.thread_ids = vec!["child".into()];
        assert!(matches!(
            run(home.path(), &child_options),
            Err(UpdateError::RootNotFound { .. })
        ));
    }

    #[test]
    fn idle_selection_orders_roots_filters_before_limit_and_applies_high_water_and_reprice_rules() {
        let home = TempDir::new().unwrap();
        let database = database(&home, false, &REQUIRED);
        let now = OffsetDateTime::now_utc().unix_timestamp();
        for (id, age) in [
            ("done", 1000),
            ("older", 1100),
            ("oldest", 1200),
            ("child", 1300),
        ] {
            insert(&database, id, Some(id), None, "legacy", now - age, Some(id));
            rollout(&home, id, (id == "child").then_some("older"));
        }
        let current = OffsetDateTime::from_unix_timestamp(now - 500).unwrap();
        index(&home, &[("done", "done · ⇄100", current)]);
        let mut options = options();
        options.idle_minutes = Some(1);
        options.limit = 1;
        assert_eq!(
            proposal_ids(&run(home.path(), &options).unwrap()),
            ["older"]
        );

        let cutoff = OffsetDateTime::now_utc() - TimeDuration::minutes(1);
        options.reprice_before = Some(cutoff);
        assert_eq!(proposal_ids(&run(home.path(), &options).unwrap()), ["done"]);
    }

    #[test]
    fn width_change_and_zero_deadline_keep_selection_safe_and_dry_run_does_not_mutate_stores() {
        let home = TempDir::new().unwrap();
        let database = database(&home, false, &REQUIRED);
        let now = OffsetDateTime::now_utc().unix_timestamp() - 600;
        insert(
            &database,
            "root",
            Some("A long stored task name"),
            None,
            "legacy",
            now,
            Some("Prompt"),
        );
        rollout(&home, "root", None);
        let index_bytes = format!(
            "{{\"id\":\"root\",\"thread_name\":\"A long stored task name · ⇄100\",\"updated_at\":\"{}\"}}\n",
            OffsetDateTime::now_utc().format(&Rfc3339).unwrap()
        );
        fs::write(home.path().join("session_index.jsonl"), index_bytes).unwrap();
        let before_index = fs::read(home.path().join("session_index.jsonl")).unwrap();
        let before_row: (Option<String>, Option<String>) = Connection::open(&database)
            .unwrap()
            .query_row(
                "SELECT title, name FROM threads WHERE id = 'root'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let mut options = options();
        options.idle_minutes = Some(1);
        assert!(run(home.path(), &options).unwrap().proposals.is_empty());
        options.title_format = format(24);
        assert_eq!(proposal_ids(&run(home.path(), &options).unwrap()), ["root"]);
        options.max_runtime = Some(Duration::ZERO);
        assert!(run(home.path(), &options).unwrap().proposals.is_empty());
        assert_eq!(
            fs::read(home.path().join("session_index.jsonl")).unwrap(),
            before_index
        );
        let after_row: (Option<String>, Option<String>) = Connection::open(&database)
            .unwrap()
            .query_row(
                "SELECT title, name FROM threads WHERE id = 'root'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(after_row, before_row);
    }
}
