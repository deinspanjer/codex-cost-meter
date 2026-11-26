use std::{
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

use crate::cache::RolloutCache;

/// Position of a Codex home in the selected home set.
pub(crate) type HomeId = usize;

/// The non-empty, ordered set of Codex homes one report reads.
pub(crate) struct Homes {
    entries: Vec<Home>,
}

struct Home {
    path: PathBuf,
    canonical: Option<PathBuf>,
    cache: Rc<RolloutCache>,
}

/// One home's discovery inputs.
#[derive(Clone, Copy)]
pub(crate) struct HomeScan<'a> {
    pub id: HomeId,
    pub path: &'a Path,
    pub cache: Option<&'a RolloutCache>,
}

impl Homes {
    pub(crate) fn open(paths: Vec<PathBuf>, refresh: bool) -> Self {
        assert!(!paths.is_empty(), "a report needs at least one Codex home");
        Self {
            entries: paths
                .into_iter()
                .map(|path| Home {
                    cache: Rc::new(RolloutCache::open(&path, refresh)),
                    canonical: fs::canonicalize(&path).ok(),
                    path,
                })
                .collect(),
        }
    }

    pub(crate) fn single(path: &Path, cache: Rc<RolloutCache>) -> Self {
        Self {
            entries: vec![Home {
                path: path.into(),
                canonical: fs::canonicalize(path).ok(),
                cache,
            }],
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn path(&self, id: HomeId) -> &Path {
        &self.entries[id].path
    }

    pub(crate) fn cache(&self, id: HomeId) -> &RolloutCache {
        &self.entries[id].cache
    }

    pub(crate) fn caches(&self) -> impl Iterator<Item = &RolloutCache> {
        self.entries.iter().map(|home| home.cache.as_ref())
    }

    pub(crate) fn scans(&self) -> Vec<HomeScan<'_>> {
        self.entries
            .iter()
            .enumerate()
            .map(|(id, home)| HomeScan {
                id,
                path: &home.path,
                cache: Some(&home.cache),
            })
            .collect()
    }

    /// Home IDs in an order that does not depend on how the homes were listed.
    pub(crate) fn stable_order(&self) -> Vec<HomeId> {
        let mut ids = (0..self.entries.len()).collect::<Vec<_>>();
        ids.sort_by_key(|id| {
            let home = &self.entries[*id];
            home.canonical.clone().unwrap_or_else(|| home.path.clone())
        });
        ids
    }

    pub(crate) fn display(&self) -> String {
        self.entries
            .iter()
            .map(|home| home.path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}
