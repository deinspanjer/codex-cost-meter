use std::{path::Path, time::Duration};

use rusqlite::{Connection, OpenFlags, OptionalExtension};

use crate::{session_index::Snapshot, title::strip_canonical_suffix};

/// Codex's persisted naming rules, without synthesizing a name from a prompt.
pub(crate) fn existing_name<'a>(
    title: Option<&'a str>,
    name: Option<&'a str>,
    history_mode: Option<&str>,
    prompt: Option<&str>,
    index_name: Option<&'a str>,
) -> Option<&'a str> {
    if history_mode == Some("paginated") {
        return nonblank(name);
    }
    nonblank(title)
        .filter(|title| Some(*title) != nonblank(prompt))
        .or_else(|| nonblank(index_name))
}

fn nonblank(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

pub(crate) struct Names {
    pub(crate) index: Snapshot,
    database: Option<Connection>,
}

impl Names {
    pub(crate) fn load(home: &Path) -> Self {
        let database = [
            home.join("state_5.sqlite"),
            home.join("sqlite/state_5.sqlite"),
        ]
        .into_iter()
        .find(|path| path.is_file())
        .and_then(|path| Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok())
        .and_then(|database| {
            // Naming enrichment must not wait behind Codex's active writer.
            database.busy_timeout(Duration::ZERO).ok()?;
            Some(database)
        });
        Self {
            index: Snapshot::load(home),
            database,
        }
    }

    pub(crate) fn get(&self, id: &str) -> Option<String> {
        let index_name = self
            .index
            .is_complete()
            .then(|| self.index.entry(id))
            .flatten()
            .map(|entry| entry.name.as_str());
        let database_name = self.database.as_ref().and_then(|database| {
            let mut statement = database
                .prepare("SELECT title, name, history_mode, first_user_message FROM threads WHERE id = ?1")
                .ok()?;
            statement
                .query_row([id], |row| {
                    let title: Option<String> = row.get(0)?;
                    let name: Option<String> = row.get(1)?;
                    let mode: Option<String> = row.get(2)?;
                    let prompt: Option<String> = row.get(3)?;
                    Ok(existing_name(
                        title.as_deref(),
                        name.as_deref(),
                        mode.as_deref(),
                        prompt.as_deref(),
                        index_name,
                    )
                    .map(str::to_owned))
                })
                .optional()
                .ok()
                .flatten()
                .flatten()
        });
        database_name
            .as_deref()
            .filter(|name| usable(name))
            .or_else(|| index_name.filter(|name| usable(name)))
            .map(str::to_owned)
    }
}

fn usable(name: &str) -> bool {
    let base = strip_canonical_suffix(name).trim();
    !base.is_empty() && !base.eq_ignore_ascii_case("Untitled") && !base.starts_with('<')
}
