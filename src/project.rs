use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

use serde::Deserialize;
use thiserror::Error;

use crate::{
    date_filter::Filter,
    homes::{HomeId, Homes},
    pricing::Catalog,
    progress::Progress,
    report::{ProjectReport, ProjectSelection, ReportContext, ReportError},
    rollout::discovery::{RolloutIndex, RolloutRecord, project_roots},
};

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

/// What a project reference resolved to, before roots are classified.
struct Target<'a> {
    /// The selected Desktop Project ID in each home, if that home has one.
    projects: Vec<Option<String>>,
    fallback_paths: Vec<PathBuf>,
    target: String,
    resolver: &'static str,
    required_thread: Option<&'a str>,
    /// Select roots whose recorded starting directory equals the target.
    exact_cwd: bool,
}

impl Target<'_> {
    fn unscoped(target: String, resolver: &'static str, homes: usize) -> Self {
        Self {
            projects: vec![None; homes],
            fallback_paths: Vec::new(),
            target,
            resolver,
            required_thread: None,
            exact_cwd: false,
        }
    }
}

pub(crate) struct ProjectScope {
    pub context: ReportContext,
    pub selection: ProjectSelection,
    pub thread_ids: Vec<String>,
}

impl ProjectScope {
    fn report(
        self,
        filter: &Filter,
        progress: &mut Progress,
    ) -> Result<ProjectReport, ProjectError> {
        let mut report = self.context.build_project_with_progress(
            self.selection,
            &self.thread_ids,
            filter,
            progress,
        )?;
        if report.selection.missing_source_roots > 0 {
            report.incomplete_input_warnings.push(format!("{} configured Desktop Project source root(s) unavailable; direct assignments and recorded workspace roots remain included", report.selection.missing_source_roots));
        }
        Ok(report)
    }
}

pub(crate) fn build_with_progress(
    homes: Rc<Homes>,
    thread_id: Option<&str>,
    project_ref: &str,
    filter: &Filter,
    progress: &mut Progress,
    catalog: &Catalog,
) -> Result<ProjectReport, ProjectError> {
    let mut scope = resolve_with_progress(homes, thread_id, project_ref, progress)?;
    scope.context.catalog = catalog.clone();
    scope.report(filter, progress)
}

pub(crate) fn resolve_with_progress(
    homes: Rc<Homes>,
    thread_id: Option<&str>,
    project_ref: &str,
    progress: &mut Progress,
) -> Result<ProjectScope, ProjectError> {
    // Several homes reuse this full index for the report; one home keeps its
    // narrower targeted index below.
    let multiple = homes.len() > 1;
    let scanned = ReportContext::scan(Rc::clone(&homes), multiple.then_some(&mut *progress))
        .map_err(ReportError::from)?;
    let paths = (0..homes.len())
        .map(|id| homes.path(id))
        .collect::<Vec<_>>();
    let roots = project_roots(&scanned.index, &paths);
    let states = paths
        .iter()
        .map(|path| load_state(path))
        .collect::<Vec<_>>();
    let target = resolve_target(&roots, &states, thread_id, project_ref)?;
    let (selection, thread_ids) = classify(&target, &roots, &scanned.index, &states)?;
    let context = if multiple {
        scanned
    } else {
        ReportContext::targeted(homes, &thread_ids, Some(progress)).map_err(ReportError::from)?
    };
    Ok(ProjectScope {
        context,
        selection,
        thread_ids,
    })
}

fn resolve_target<'a>(
    roots: &[RolloutRecord],
    states: &[(DesktopState, bool)],
    thread_id: Option<&'a str>,
    project_ref: &str,
) -> Result<Target<'a>, ProjectError> {
    let metadata_available = states.iter().any(|(_, available)| *available);
    if !project_ref.is_empty() && metadata_available {
        let named = matching_projects(states, |project| {
            project.name.eq_ignore_ascii_case(project_ref)
        });
        if !named.is_empty() {
            let grouped = one_group(&named).ok_or_else(|| ProjectError::AmbiguousProjectName {
                reference: project_ref.into(),
                matches: project_names(&named),
            })?;
            let mut target = project_target(states, &grouped, "project_name")?;
            target.required_thread = thread_id;
            return Ok(target);
        }
    }
    if project_ref.is_empty()
        && let Some(thread_id) = thread_id
    {
        if !metadata_available {
            return Err(ProjectError::MetadataUnavailable);
        }
        let root = roots
            .iter()
            .find(|root| root.id == thread_id)
            .ok_or_else(|| ProjectError::ThreadNotRoot(thread_id.into()))?;
        let (state, available) = &states[root.home];
        if !available {
            return Err(ProjectError::MetadataUnavailable);
        }
        if let Some(project) = state
            .assignments
            .get(&root.id)
            .and_then(|assignment| state.projects.get(&assignment.project_id))
        {
            return project_target(states, &[(root.home, project)], "thread_assignment");
        }
        let cwd = root
            .cwd
            .as_deref()
            .ok_or_else(|| ProjectError::ThreadCwdMissing(thread_id.into()))?;
        return Ok(Target {
            exact_cwd: true,
            ..Target::unscoped(cwd.into(), "thread_cwd", states.len())
        });
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
            Err(_) if metadata_available => return fuzzy_project(roots, states, project_ref),
            Err(_) => {
                return match fuzzy_historical_cwd(roots, states.len(), project_ref) {
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

    // A path owns a project only when it is one of that project's source roots.
    let owners = matching_projects(states, |project| {
        project
            .root_paths
            .iter()
            .filter_map(|root| root.canonicalize().ok())
            .any(|root| root == path)
    });
    let mut target = if owners.is_empty() {
        Target::unscoped(
            path.display().to_string(),
            if current_directory {
                "current_directory"
            } else {
                "path"
            },
            states.len(),
        )
    } else {
        let grouped = one_group(&owners).ok_or_else(|| ProjectError::AmbiguousSourceRoot {
            path: path.display().to_string(),
            matches: project_names(&owners),
        })?;
        project_target(states, &grouped, "source_root_path")?
    };
    target.fallback_paths = vec![path];
    target.required_thread = thread_id;
    Ok(target)
}

/// Projects that satisfy `matches`, with the home each belongs to.
fn matching_projects(
    states: &[(DesktopState, bool)],
    matches: impl Fn(&DesktopProject) -> bool,
) -> Vec<(HomeId, &DesktopProject)> {
    let mut projects = states
        .iter()
        .enumerate()
        .flat_map(|(home, (state, _))| {
            state
                .projects
                .values()
                .filter(|project| matches(project))
                .map(move |project| (home, project))
        })
        .collect::<Vec<_>>();
    projects.sort_by(|left, right| left.1.name.cmp(&right.1.name).then(left.0.cmp(&right.0)));
    projects
}

/// The matches when they name one project: at most one per home, all with the
/// same case-insensitive name. Same-named projects in different homes are one
/// project, because each account home assigns its own project IDs.
fn one_group<'a>(
    projects: &[(HomeId, &'a DesktopProject)],
) -> Option<Vec<(HomeId, &'a DesktopProject)>> {
    let (_, first) = projects.first()?;
    let mut homes = HashSet::new();
    projects
        .iter()
        .all(|(home, project)| {
            homes.insert(*home) && project.name.eq_ignore_ascii_case(&first.name)
        })
        .then(|| projects.to_vec())
}

/// Select the named project in every home. `fixed` projects are already
/// chosen for their homes; other homes select their same-named project.
fn project_target<'a>(
    states: &[(DesktopState, bool)],
    fixed: &[(HomeId, &DesktopProject)],
    resolver: &'static str,
) -> Result<Target<'a>, ProjectError> {
    let name = &fixed[0].1.name;
    let mut projects = Vec::with_capacity(states.len());
    let mut fallback_paths = Vec::new();
    for (home, (state, _)) in states.iter().enumerate() {
        let project = match fixed.iter().find(|(fixed_home, _)| *fixed_home == home) {
            Some((_, project)) => Some(*project),
            None => {
                let named = state
                    .projects
                    .values()
                    .filter(|project| project.name.eq_ignore_ascii_case(name))
                    .collect::<Vec<_>>();
                if named.len() > 1 {
                    return Err(ProjectError::AmbiguousProjectName {
                        reference: name.clone(),
                        matches: project_names(
                            &named
                                .into_iter()
                                .map(|project| (home, project))
                                .collect::<Vec<_>>(),
                        ),
                    });
                }
                named.into_iter().next()
            }
        };
        if let Some(project) = project {
            for path in &project.root_paths {
                if !fallback_paths.contains(path) {
                    fallback_paths.push(path.clone());
                }
            }
        }
        projects.push(project.map(|project| project.id.clone()));
    }
    Ok(Target {
        projects,
        fallback_paths,
        target: name.clone(),
        resolver,
        required_thread: None,
        exact_cwd: false,
    })
}

fn fuzzy_project<'a>(
    roots: &[RolloutRecord],
    states: &[(DesktopState, bool)],
    project_ref: &str,
) -> Result<Target<'a>, ProjectError> {
    let matches = matching_projects(states, |project| {
        high_confidence_match(project_ref, &project.name)
    });
    if matches.is_empty() {
        return fuzzy_historical_cwd(roots, states.len(), project_ref);
    }
    let grouped = one_group(&matches).ok_or_else(|| ProjectError::AmbiguousProjectRef {
        reference: project_ref.into(),
        matches: project_names(&matches),
    })?;
    project_target(states, &grouped, "fuzzy_project_name")
}

fn fuzzy_historical_cwd<'a>(
    roots: &[RolloutRecord],
    homes: usize,
    project_ref: &str,
) -> Result<Target<'a>, ProjectError> {
    let mut matches = roots
        .iter()
        .filter_map(|root| root.cwd.as_deref())
        .filter(|cwd| high_confidence_match(project_ref, cwd))
        .collect::<Vec<_>>();
    matches.sort_unstable();
    matches.dedup();
    match matches.as_slice() {
        [cwd] => Ok(Target {
            fallback_paths: vec![PathBuf::from(cwd)],
            ..Target::unscoped((*cwd).into(), "fuzzy_historical_cwd", homes)
        }),
        [] => Err(ProjectError::ProjectRefNotFound(project_ref.into())),
        _ => Err(ProjectError::AmbiguousHistoricalCwd {
            reference: project_ref.into(),
            matches: short_list(matches.iter().copied()),
        }),
    }
}

fn project_names(projects: &[(HomeId, &DesktopProject)]) -> String {
    let mut names = projects
        .iter()
        .map(|(_, project)| project.name.as_str())
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

/// Why one home includes or excludes a root, weakest first. A root copied into
/// several homes takes the strongest reason any holding home gives.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Outcome {
    Unrelated,
    OtherProject,
    ProjectlessExcluded,
    ProjectlessIncluded,
    WorkspaceFallback,
    DirectAssignment,
}

fn classify(
    target: &Target<'_>,
    roots: &[RolloutRecord],
    index: &RolloutIndex,
    states: &[(DesktopState, bool)],
) -> Result<(ProjectSelection, Vec<String>), ProjectError> {
    let fallback_paths = target
        .fallback_paths
        .iter()
        .map(|path| path.canonicalize().unwrap_or_else(|_| path.clone()))
        .collect::<Vec<_>>();
    let missing_source_roots = usize::from(target.projects.iter().any(Option::is_some))
        * fallback_paths.iter().filter(|path| !path.exists()).count();
    let mut selection = ProjectSelection {
        target: target.target.clone(),
        resolver: target.resolver,
        missing_source_roots,
        direct_assignments: 0,
        workspace_fallbacks: 0,
        projectless_threads: 0,
        projectless_exclusions: 0,
        other_project_exclusions: 0,
        incomplete_root_reports: 0,
        unpriced_root_reports: 0,
    };
    let mut thread_ids = Vec::new();
    for root in roots {
        let Some(cwd) = root.cwd.as_deref() else {
            continue;
        };
        let beneath = if target.exact_cwd {
            cwd == target.target
        } else {
            let cwd = Path::new(cwd);
            let cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
            fallback_paths.iter().any(|path| cwd.starts_with(path))
        };
        let outcome = index
            .holders(&root.id)
            .iter()
            .map(|home| {
                let state = &states[*home].0;
                let assignment = state
                    .assignments
                    .get(&root.id)
                    .map(|entry| entry.project_id.as_str());
                let projectless = state.projectless.contains(&root.id);
                if target.projects[*home]
                    .as_deref()
                    .is_some_and(|project| assignment == Some(project))
                {
                    Outcome::DirectAssignment
                } else if !beneath {
                    Outcome::Unrelated
                } else if target.exact_cwd {
                    if assignment.is_some() {
                        Outcome::OtherProject
                    } else if projectless {
                        Outcome::ProjectlessIncluded
                    } else {
                        Outcome::WorkspaceFallback
                    }
                } else if projectless {
                    Outcome::ProjectlessExcluded
                } else if assignment.is_some() {
                    Outcome::OtherProject
                } else {
                    Outcome::WorkspaceFallback
                }
            })
            .max()
            .unwrap_or(Outcome::Unrelated);
        match outcome {
            Outcome::DirectAssignment => selection.direct_assignments += 1,
            Outcome::WorkspaceFallback => selection.workspace_fallbacks += 1,
            Outcome::ProjectlessIncluded => selection.projectless_threads += 1,
            Outcome::ProjectlessExcluded => selection.projectless_exclusions += 1,
            Outcome::OtherProject => selection.other_project_exclusions += 1,
            Outcome::Unrelated => {}
        }
        if outcome >= Outcome::ProjectlessIncluded {
            thread_ids.push(root.id.clone());
        }
    }
    thread_ids.sort();
    if let Some(thread_id) = target.required_thread
        && !thread_ids.iter().any(|candidate| candidate == thread_id)
    {
        return Err(ProjectError::ThreadOutsideProject {
            thread_id: thread_id.into(),
            target: selection.target.clone(),
        });
    }
    Ok((selection, thread_ids))
}

fn load_state(codex_home: &Path) -> (DesktopState, bool) {
    fs::read(codex_home.join(".codex-global-state.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .map(|state| (state, true))
        .unwrap_or_default()
}
