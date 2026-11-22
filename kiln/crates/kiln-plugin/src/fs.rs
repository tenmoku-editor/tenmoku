// SPDX-License-Identifier: Apache-2.0

//! The host's file-system imports for plugins.
//!
//! The interface is empty: files come through WASI preopens, which [`configure`] sets up from
//! the plugin's grants. A plugin reaches the workspace's folders only with `fs.read:workspace`
//! or `fs.write:workspace`, and nothing else on disk at all. WASI resolves every path inside
//! the preopen it starts from, so neither `..` nor a symlink leads out of it.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use kiln_command::Grants;
use kiln_command::capability::{Capability, FsScope};
use wasmtime_wasi::{FsPerms, WasiCtxBuilder};

use crate::bindings::HostState;
use crate::bindings::kiln::extension::fs::Host;
use crate::paths::{self, Rules};

/// The file-system slice of [`HostState`].
pub struct State;

impl Host for HostState {}

/// The folders open in the app, which `fs.read:workspace` and `fs.write:workspace` reach.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Workspace {
    pub roots: Vec<PathBuf>,
}

/// A workspace root that was preopened: the directory handle it was opened as, which is what
/// `process.start`'s `cwd` is resolved in, so its authority is exactly the WASI preopens.
#[derive(Clone, Debug)]
pub struct PreopenedRoot {
    /// As the app gave it.
    pub path: PathBuf,
    /// The directory's canonical path as of its preopen (before the hook below), where Windows
    /// resolves a `cwd`.
    pub canonical: PathBuf,
    /// The directory, opened once before its preopen and held since.
    pub dir: Arc<File>,
}

/// A hook that tests set to run code at an exact point; see [`hooks`].
type Hook = Mutex<Option<Arc<dyn Fn(&Path) + Send + Sync>>>;

/// Deterministic test hooks. A hook runs with the path it is about, on whichever thread reaches
/// that point; the guard clears it on drop. Not for use outside tests.
#[doc(hidden)]
pub mod hooks {
    use super::*;

    pub(crate) static AFTER_PREOPEN: Hook = Mutex::new(None);
    pub(crate) static BEFORE_LAUNCH: Hook = Mutex::new(None);

    /// Clears its hook when dropped.
    pub struct Guard(&'static Hook);

    impl Drop for Guard {
        fn drop(&mut self) {
            *self.0.lock().unwrap_or_else(PoisonError::into_inner) = None;
        }
    }

    fn set(slot: &'static Hook, hook: impl Fn(&Path) + Send + Sync + 'static) -> Guard {
        *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(hook));
        Guard(slot)
    }

    pub(crate) fn run(slot: &'static Hook, path: &Path) {
        let hook = slot.lock().unwrap_or_else(PoisonError::into_inner).clone();
        if let Some(hook) = hook {
            hook(path);
        }
    }

    /// Runs `hook` with a root's path after the root's preopen and before the roots are handed to
    /// the store.
    pub fn after_preopen(hook: impl Fn(&Path) + Send + Sync + 'static) -> Guard {
        set(&AFTER_PREOPEN, hook)
    }

    /// Runs `hook` with the resolved `cwd`'s path after its final resolution and before the
    /// launch.
    pub fn before_launch(hook: impl Fn(&Path) + Send + Sync + 'static) -> Guard {
        set(&BEFORE_LAUNCH, hook)
    }
}

/// `root`'s canonical path, if both it and `root` itself still name the directory `held` is open
/// on: none if either names another, or can't be read. A canonical path is never taken without
/// being checked against the handle it is stored with.
pub(crate) fn verified_canonical(root: &Path, held: &File) -> Option<PathBuf> {
    let identity = kiln_core::platform::file_identity(held).ok()?;
    let canonical = kiln_core::platform::canonicalize(root).ok()?;
    let names = |path: &Path| kiln_core::platform::path_identity(path, true).ok() == Some(identity);
    (names(root) && names(&canonical)).then_some(canonical)
}

/// [`configure_roots`], leaving out what it returns.
pub fn configure(wasi: &mut WasiCtxBuilder, grants: &Grants, workspace: &Workspace) {
    configure_roots(wasi, grants, workspace);
}

/// Sets `wasi`, a fresh builder, up as `grants` allow in `workspace`: each root preopened,
/// read-only for `fs.read:workspace` and read-write for `fs.write:workspace` (WASI has no
/// write-only directory), at its guest path ([`paths::root`]): its own absolute path on Unix,
/// `/C:/…` on Windows, so the paths a plugin sees are the app's, mapped as every path it's given
/// is. Without either grant nothing is preopened. Sockets and name lookups are denied. The rest is
/// left as [`WasiCtxBuilder::new`] makes it: no environment, no arguments, stdin closed, and
/// stdout and stderr discarded.
///
/// A root that can't be opened, or has no guest path (a network share, or a Windows root over
/// 260 characters), is left out, with a warning.
///
/// Returns the roots preopened, each as the directory handle it was opened as before its
/// preopen. wasmtime-wasi 48's `preopened_dir` takes only a path (its `Dir` is built from a
/// handle, but the builder's preopens are private), so WASI opens the root by path itself: the
/// handle is opened first, the preopen next, and the path's identity checked against the handle's
/// right after. A root whose path names another directory by then is left out, with a warning,
/// so the handle is the directory its preopen holds unless the path was retargeted and put back
/// within that window.
pub fn configure_roots(
    wasi: &mut WasiCtxBuilder,
    grants: &Grants,
    workspace: &Workspace,
) -> Vec<PreopenedRoot> {
    wasi.allow_tcp(false)
        .allow_udp(false)
        .allow_ip_name_lookup(false)
        .socket_addr_check(|_, _| Box::pin(async { false }));
    let perms = if grants.allows(&Capability::FsWrite(FsScope::Workspace)) {
        FsPerms::ReadWrite
    } else if grants.allows(&Capability::FsRead(FsScope::Workspace)) {
        FsPerms::ReadOnly
    } else {
        return Vec::new();
    };
    let mut opened = Vec::new();
    for root in &workspace.roots {
        let Some(guest_path) = paths::root(root, Rules::HOST) else {
            continue;
        };
        let held = match kiln_core::platform::open_dir(root) {
            Ok(held) => held,
            Err(error) => {
                log::warn!("not preopening {}: {error}", root.display());
                continue;
            }
        };
        if let Err(error) = wasi.preopened_dir(root, &guest_path, perms) {
            log::warn!("not preopening {}: {error:#}", root.display());
            continue;
        }
        let Some(canonical) = verified_canonical(root, &held) else {
            log::warn!("not using {} as a start root: it changed", root.display());
            continue;
        };
        opened.push(PreopenedRoot {
            path: root.clone(),
            canonical,
            dir: Arc::new(held),
        });
    }
    for root in &opened {
        hooks::run(&hooks::AFTER_PREOPEN, &root.path);
    }
    opened
}
