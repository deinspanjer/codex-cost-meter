use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use thiserror::Error;

use crate::report::{ProjectReport, ProjectSelection, ReportContext, ReportError};

#[derive(Debug, Error)]
pub(crate) enum ProjectError {
    #[error("project path does not exist: {0}")]
    PathMissing(String),
    #[error("could not resolve project path {path}: {source}")]
    PathResolve {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("project source root is ambiguous: {path}; matches: {matches}")]
    AmbiguousSourceRoot { path: String, matches: String },
    #[error("project name is ambiguous: {reference}; matches: {matches}")]
    AmbiguousProjectName { reference: String, matches: String },
    #[error(
        "project reference has multiple high-confidence project-name matches: {reference}; matches: {matches}"
    )]
    AmbiguousProjectRef { reference: String, matches: String },
    #[error(
        "project reference has multiple high-confidence historical-CWD matches: {reference}; matches: {matches}"
    )]
    AmbiguousHistoricalCwd { reference: String, matches: String },
    #[error("no project name, valid local path, or historical starting-directory match: {0}")]
    ProjectRefNotFound(String),
    #[error("Desktop Project metadata is unavailable")]
    MetadataUnavailable,
    #[error("thread is not a root rollout: {0}")]
    ThreadNotRoot(String),
    #[error("thread has no recorded starting directory: {0}")]
    ThreadCwdMissing(String),
    #[error("thread {thread_id} does not belong to the selected project: {target}")]
    ThreadOutsideProject { thread_id: String, target: String },
    #[error(transparent)]
    Report(#[from] ReportError),
}

#[derive(Default, Deserialize)]
struct DesktopState {
    #[serde(rename = "local-projects")]
    projects: HashMap<String, DesktopProject>,
    #[serde(rename = "thread-project-assignments")]
    assignments: HashMap<String, ProjectAssignment>,
    #[serde(rename = "projectless-thread-ids")]
    projectless: HashSet<String>,
}

#[derive(Deserialize)]
struct DesktopProject {
    id: String,
    name: String,
    #[serde(rename = "rootPaths")]
    root_paths: Vec<PathBuf>,
}

#[derive(Deserialize)]
struct ProjectAssignment {
    #[serde(rename = "projectId")]
    project_id: String,
}

pub(crate) fn build(
    codex_home: &Path,
    thread_id: Option<&str>,
    project_ref: &str,
) -> Result<ProjectReport, ProjectError> {
    let context = ReportContext::new(codex_home).map_err(ReportError::from)?;
    let (state, metadata_available) = load_state(codex_home);
    if !project_ref.is_empty() && metadata_available {
        let named = state
            .projects
            .values()
            .filter(|project| project.name.eq_ignore_ascii_case(project_ref))
            .collect::<Vec<_>>();
        if named.len() > 1 {
            return Err(ProjectError::AmbiguousProjectName {
                reference: project_ref.into(),
                matches: project_names(&named),
            });
        }
        if let [project] = named.as_slice() {
            return build_scope(
                &context,
                &state,
                Some(project.id.as_str()),
                &project.root_paths,
                project.name.clone(),
                "project_name",
                thread_id,
            );
        }
    }
    if project_ref.is_empty()
        && let Some(thread_id) = thread_id
    {
        if !metadata_available {
            return Err(ProjectError::MetadataUnavailable);
        }
        let root = context
            .roots()
            .find(|root| root.id == thread_id)
            .cloned()
            .ok_or_else(|| ProjectError::ThreadNotRoot(thread_id.into()))?;
        if let Some(project) = state
            .assignments
            .get(&root.id)
            .and_then(|assignment| state.projects.get(&assignment.project_id))
        {
            return build_scope(
                &context,
                &state,
                Some(project.id.as_str()),
                &project.root_paths,
                project.name.clone(),
                "thread_assignment",
                None,
            );
        }
        return build_cwd_scope(
            &context,
            &state,
            root.cwd
                .as_deref()
                .ok_or_else(|| ProjectError::ThreadCwdMissing(thread_id.into()))?,
        );
    }
    let current_directory = project_ref.is_empty() && thread_id.is_none();
    let path = if project_ref.is_empty() {
        std::env::current_dir().map_err(|source| ProjectError::PathResolve {
            path: "current directory".into(),
            source,
        })?
    } else {
        match PathBuf::from(project_ref).canonicalize() {
            Ok(path) => path,
            Err(_) if metadata_available => return fuzzy_project(&context, &state, project_ref),
            Err(_) => {
                return match fuzzy_historical_cwd(&context, &state, project_ref) {
                    Err(ProjectError::ProjectRefNotFound(_)) => {
                        Err(ProjectError::MetadataUnavailable)
                    }
                    result => result,
                };
            }
        }
    };
    let path = path.canonicalize().map_err(|source| match source.kind() {
        std::io::ErrorKind::NotFound => ProjectError::PathMissing(path.display().to_string()),
        _ => ProjectError::PathResolve {
            path: path.display().to_string(),
            source,
        },
    })?;

    let owners = state
        .projects
        .values()
        .filter(|project| {
            project
                .root_paths
                .iter()
                .filter_map(|root| root.canonicalize().ok())
                .any(|root| root == path)
        })
        .collect::<Vec<_>>();
    if owners.len() > 1 {
        return Err(ProjectError::AmbiguousSourceRoot {
            path: path.display().to_string(),
            matches: project_names(&owners),
        });
    }

    let (target, resolver, selected_project) = match owners.as_slice() {
        [project] => (
            project.name.clone(),
            "source_root_path",
            Some(project.id.as_str()),
        ),
        [] => (
            path.display().to_string(),
            if current_directory {
                "current_directory"
            } else {
                "path"
            },
            None,
        ),
        _ => unreachable!(),
    };
    build_scope(
        &context,
        &state,
        selected_project,
        std::slice::from_ref(&path),
        target,
        resolver,
        thread_id,
    )
}

fn fuzzy_project(
    context: &ReportContext,
    state: &DesktopState,
    project_ref: &str,
) -> Result<ProjectReport, ProjectError> {
    let mut matches = state
        .projects
        .values()
        .filter(|project| high_confidence_match(project_ref, &project.name))
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| left.name.cmp(&right.name));
    match matches.as_slice() {
        [project] => build_scope(
            context,
            state,
            Some(project.id.as_str()),
            &project.root_paths,
            project.name.clone(),
            "fuzzy_project_name",
            None,
        ),
        [] => fuzzy_historical_cwd(context, state, project_ref),
        _ => Err(ProjectError::AmbiguousProjectRef {
            reference: project_ref.into(),
            matches: project_names(&matches),
        }),
    }
}

fn fuzzy_historical_cwd(
    context: &ReportContext,
    state: &DesktopState,
    project_ref: &str,
) -> Result<ProjectReport, ProjectError> {
    let mut matches = context
        .roots()
        .filter_map(|root| root.cwd.as_deref())
        .filter(|cwd| high_confidence_match(project_ref, cwd))
        .collect::<Vec<_>>();
    matches.sort_unstable();
    matches.dedup();
    match matches.as_slice() {
        [cwd] => build_scope(
            context,
            state,
            None,
            &[PathBuf::from(cwd)],
            (*cwd).into(),
            "fuzzy_historical_cwd",
            None,
        ),
        [] => Err(ProjectError::ProjectRefNotFound(project_ref.into())),
        _ => Err(ProjectError::AmbiguousHistoricalCwd {
            reference: project_ref.into(),
            matches: short_list(matches.iter().copied()),
        }),
    }
}

fn project_names(projects: &[&DesktopProject]) -> String {
    let mut names = projects
        .iter()
        .map(|project| project.name.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    short_list(names.into_iter())
}

fn short_list<'a>(values: impl Iterator<Item = &'a str>) -> String {
    let values = values.collect::<Vec<_>>();
    let suffix = if values.len() > 5 { ", ..." } else { "" };
    format!(
        "{}{}",
        values.into_iter().take(5).collect::<Vec<_>>().join(", "),
        suffix
    )
}

fn high_confidence_match(reference: &str, candidate: &str) -> bool {
    let reference = normalized(reference);
    let candidate = normalized(candidate);
    !reference.is_empty()
        && (candidate.contains(&reference)
            || levenshtein(&reference, &candidate)
                <= (reference.len().max(candidate.len()) / 5).max(1))
}

fn normalized(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn levenshtein(left: &str, right: &str) -> usize {
    let mut row = (0..=right.len()).collect::<Vec<_>>();
    for (left_index, left_char) in left.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = left_index + 1;
        for (right_index, right_char) in right.chars().enumerate() {
            let previous = row[right_index + 1];
            row[right_index + 1] = (row[right_index + 1] + 1)
                .min(row[right_index] + 1)
                .min(diagonal + usize::from(left_char != right_char));
            diagonal = previous;
        }
    }
    row[right.len()]
}

fn build_scope(
    context: &ReportContext,
    state: &DesktopState,
    selected_project: Option<&str>,
    fallback_paths: &[PathBuf],
    target: String,
    resolver: &'static str,
    required_thread: Option<&str>,
) -> Result<ProjectReport, ProjectError> {
    let fallback_paths = fallback_paths
        .iter()
        .map(|path| path.canonicalize().unwrap_or_else(|_| path.clone()))
        .collect::<Vec<_>>();
    let missing_source_roots = usize::from(selected_project.is_some())
        * fallback_paths.iter().filter(|path| !path.exists()).count();
    let mut thread_ids = Vec::new();
    let mut selection = ProjectSelection {
        target,
        resolver,
        missing_source_roots,
        direct_assignments: 0,
        workspace_fallbacks: 0,
        projectless_threads: 0,
        projectless_exclusions: 0,
        other_project_exclusions: 0,
        incomplete_threads: 0,
        unpriced_threads: 0,
    };
    for root in context.roots() {
        let Some(cwd) = root.cwd.as_deref().map(Path::new) else {
            continue;
        };
        let cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        let assignment = state
            .assignments
            .get(&root.id)
            .map(|entry| entry.project_id.as_str());
        let direct = selected_project.is_some_and(|project| assignment == Some(project));
        let beneath = fallback_paths.iter().any(|path| cwd.starts_with(path));
        if direct {
            selection.direct_assignments += 1;
            thread_ids.push(root.id.clone());
        } else if beneath && assignment.is_none() && !state.projectless.contains(&root.id) {
            selection.workspace_fallbacks += 1;
            thread_ids.push(root.id.clone());
        } else if beneath && state.projectless.contains(&root.id) {
            selection.projectless_exclusions += 1;
        } else if beneath && assignment.is_some() {
            selection.other_project_exclusions += 1;
        }
    }
    thread_ids.sort();
    if let Some(thread_id) = required_thread
        && !thread_ids.iter().any(|candidate| candidate == thread_id)
    {
        return Err(ProjectError::ThreadOutsideProject {
            thread_id: thread_id.into(),
            target: selection.target.clone(),
        });
    }
    let mut report = context
        .build_project(selection, &thread_ids)
        .map_err(ProjectError::from)?;
    if report.selection.missing_source_roots > 0 {
        report.incomplete_input_warnings.push(format!(
            "{} configured Desktop Project source root(s) unavailable; direct assignments and recorded workspace roots remain included",
            report.selection.missing_source_roots
        ));
    }
    Ok(report)
}

fn build_cwd_scope(
    context: &ReportContext,
    state: &DesktopState,
    cwd: &str,
) -> Result<ProjectReport, ProjectError> {
    let mut thread_ids = Vec::new();
    let mut selection = ProjectSelection {
        target: cwd.into(),
        resolver: "thread_cwd",
        missing_source_roots: 0,
        direct_assignments: 0,
        workspace_fallbacks: 0,
        projectless_threads: 0,
        projectless_exclusions: 0,
        other_project_exclusions: 0,
        incomplete_threads: 0,
        unpriced_threads: 0,
    };
    for root in context
        .roots()
        .filter(|root| root.cwd.as_deref() == Some(cwd))
    {
        if state.assignments.contains_key(&root.id) {
            selection.other_project_exclusions += 1;
        } else if state.projectless.contains(&root.id) {
            selection.projectless_threads += 1;
            thread_ids.push(root.id.clone());
        } else {
            selection.workspace_fallbacks += 1;
            thread_ids.push(root.id.clone());
        }
    }
    thread_ids.sort();
    context
        .build_project(selection, &thread_ids)
        .map_err(Into::into)
}

fn load_state(codex_home: &Path) -> (DesktopState, bool) {
    fs::read(codex_home.join(".codex-global-state.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .map(|state| (state, true))
        .unwrap_or_default()
}
