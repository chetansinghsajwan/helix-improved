//! Watches git directories so open documents can refresh their diff base when `HEAD`, the index
//! or a ref changes outside the editor (for example `git add`, `commit` or `checkout` run from
//! another pane).

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{mpsc, Mutex},
    time::Duration,
};

use anyhow::Result;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

/// How long to wait after the last event before reporting, so one git command that touches many
/// files triggers a single refresh.
const DEBOUNCE: Duration = Duration::from_millis(200);

/// Watches the git directories of open documents and reports each changed git dir, debounced.
pub struct GitWatcher {
    watcher: Mutex<RecommendedWatcher>,
    watched: Mutex<HashSet<PathBuf>>,
}

impl GitWatcher {
    /// Starts the watcher. `on_change` runs on a background thread with the git dir whose
    /// `HEAD`, index or refs changed.
    pub fn new(on_change: impl Fn(PathBuf) + Send + 'static) -> Result<Self> {
        let (tx, rx) = mpsc::channel::<PathBuf>();
        let watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
            let Ok(event) = event else { return };
            for path in event.paths {
                if let Some(git_dir) = git_dir_of(&path) {
                    let _ = tx.send(git_dir);
                }
            }
        })?;

        std::thread::spawn(move || {
            while let Ok(first) = rx.recv() {
                let mut dirs = HashSet::from([first]);
                while let Ok(next) = rx.recv_timeout(DEBOUNCE) {
                    dirs.insert(next);
                }
                dirs.into_iter().for_each(&on_change);
            }
        });

        Ok(Self {
            watcher: Mutex::new(watcher),
            watched: Mutex::default(),
        })
    }

    /// Starts watching `git_dir` (and its `refs`) unless it is already watched.
    pub fn watch(&self, git_dir: &Path) {
        if !self.watched.lock().unwrap().insert(git_dir.to_path_buf()) {
            return;
        }
        let mut watcher = self.watcher.lock().unwrap();
        if let Err(err) = watcher.watch(git_dir, RecursiveMode::NonRecursive) {
            log::debug!("failed to watch {}: {err}", git_dir.display());
            return;
        }
        let refs = git_dir.join("refs");
        if refs.is_dir() {
            let _ = watcher.watch(&refs, RecursiveMode::Recursive);
        }
    }
}

/// Maps a changed path to its git dir when it is one that affects the diff base: `HEAD`,
/// `index`, `packed-refs` or anything under `refs/`. Lock files and objects are ignored.
fn git_dir_of(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_str()?;
    if matches!(name, "HEAD" | "index" | "packed-refs") {
        return path.parent().map(Path::to_path_buf);
    }
    let refs = path.ancestors().find(|a| a.file_name().is_some_and(|n| n == "refs"))?;
    refs.parent().map(Path::to_path_buf)
}
