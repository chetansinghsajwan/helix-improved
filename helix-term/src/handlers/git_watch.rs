//! Keeps the diff gutter and line highlights in sync with git changes made outside the editor.

use std::sync::OnceLock;

use helix_event::register_hook;
use helix_vcs::GitWatcher;
use helix_view::{events::DocumentDidOpen, handlers::Handlers};

use crate::job;

/// The single watcher, started the first time a document is opened.
static WATCHER: OnceLock<Option<GitWatcher>> = OnceLock::new();

fn watcher() -> Option<&'static GitWatcher> {
    WATCHER
        .get_or_init(|| {
            let watcher = GitWatcher::new(|vcs_dir| {
                job::dispatch_blocking(move |editor, _| editor.refresh_diff_bases(&vcs_dir));
            });
            watcher
                .map_err(|err| log::warn!("git watcher unavailable: {err}"))
                .ok()
        })
        .as_ref()
}

pub(super) fn register_hooks(_handlers: &Handlers) {
    register_hook!(move |event: &mut DocumentDidOpen<'_>| {
        let doc = event.editor.document(event.doc).unwrap();
        let Some(path) = doc.path() else {
            return Ok(());
        };
        let trust_full = event
            .editor
            .workspace_trust
            .query(doc.workspace_root(), helix_loader::workspace_trust::TrustQuery::Git)
            .is_trusted();
        if let Some(vcs_dir) = event.editor.diff_providers.get_vcs_dir(path, trust_full) {
            if let Some(watcher) = watcher() {
                watcher.watch(&vcs_dir);
            }
        }
        Ok(())
    });
}
