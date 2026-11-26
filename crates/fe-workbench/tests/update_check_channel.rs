// SPDX-License-Identifier: Apache-2.0

//! The Check for Updates fallback follows `update.channel` and orders
//! versions with `ReleaseVersion`, with a fake `UpdateSource`, so no network:
//! stable asks for the latest release and ignores previews, preview takes the
//! newest of the recent releases, and neither offers a draft, a malformed tag
//! or a downgrade.

use std::{fs, path::PathBuf, sync::Arc};

use fe_command::{ContextFrame, ContextStack, Invocation};
use fe_config::SettingsStore;
use fe_gpui::{EditorSettings, EditorTheme, commands::RunCommand};
use fe_ui::{Role, nodes};
use fe_workbench::{
    Launch, Workbench,
    update_check::{Fetched, UpdateSource},
};
use futures::FutureExt as _;
use gpui::{Focusable as _, TestAppContext, VisualTestContext};
use kiln_platform::config::{self, ConfigService};
use serde_json::{Value, json};

/// The user's settings file, and what tells the config service it changed.
struct Settings {
    _dir: tempfile::TempDir,
    user: PathBuf,
    changed: config::BatchSender,
}

impl Settings {
    /// Sets `update.channel`, as the user's editing the file would.
    fn channel(&self, channel: &str, cx: &mut VisualTestContext) {
        fs::write(&self.user, format!("[update]\nchannel = \"{channel}\"\n")).unwrap();
        self.changed.send(vec![self.user.clone()]);
        cx.run_until_parked();
    }
}

/// A window, with `channel` set in a user settings file.
fn window<'a>(channel: &str, cx: &'a mut TestAppContext) -> (Settings, &'a mut VisualTestContext) {
    for var in ["TENMOKU_CONFIG_DIR", "XDG_CONFIG_HOME", "HOME"] {
        // SAFETY: no other thread is running yet to read the environment.
        unsafe { std::env::remove_var(var) };
    }
    let dir = tempfile::tempdir().unwrap();
    let user = dir.path().join("settings.toml");
    fs::write(&user, format!("[update]\nchannel = \"{channel}\"\n")).unwrap();
    let (changed, batches) = config::batch_channel();
    cx.update(|cx| {
        fe_gpui::init(cx);
        fe_workbench::init(cx);
        let store = SettingsStore::load(Some(user.clone()), None);
        let service = ConfigService::new()
            .settings(store.into_inner())
            .start_with(batches, cx);
        std::mem::forget(service);
    });
    let launch = Launch {
        root: None,
        file: None,
    };
    let (workbench, cx) = cx.add_window_view(move |window, cx| {
        let theme = EditorTheme::default_dark(cx);
        Workbench::new(launch, theme, EditorSettings::default(), window, cx)
    });
    cx.update(|window, cx| {
        window.activate_window();
        window.focus(&workbench.focus_handle(cx), cx);
    });
    cx.run_until_parked();
    let settings = Settings {
        _dir: dir,
        user,
        changed,
    };
    (settings, cx)
}

/// Has every check from now on get `reply` at version `running`.
fn fake(running: &str, reply: Value, cx: &mut VisualTestContext) {
    let body = reply.to_string().into_bytes();
    let running = running.to_owned();
    cx.update(|_, cx| {
        cx.set_global(UpdateSource {
            fetch: Arc::new(move || {
                let fetched = Fetched {
                    status: 200,
                    body: body.clone(),
                };
                futures::future::ready(Ok(fetched)).boxed()
            }),
            running: running.clone(),
        })
    });
}

fn draw(cx: &mut VisualTestContext) {
    for _ in 0..2 {
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
    }
    cx.run_until_parked();
}

/// Runs Check for Updates… against `reply` at `running`, and returns the
/// message of the toast it leaves, the last one.
fn check(running: &str, reply: Value, cx: &mut VisualTestContext) -> String {
    fake(running, reply, cx);
    let stack = ContextStack::new().with(ContextFrame::new("workbench"));
    let run = RunCommand(
        Invocation::of("app.check_for_updates")
            .unwrap()
            .context(stack),
    );
    cx.update(|window, cx| window.dispatch_action(Box::new(run), cx));
    draw(cx);
    cx.update(|window, cx| nodes(window, cx))
        .into_iter()
        .filter(|node| node.role == Role::Alert)
        .map(|node| node.label.to_string())
        .next_back()
        .expect("a toast")
}

fn release(tag: &str, draft: bool) -> Value {
    json!({
        "tag_name": tag,
        "html_url": format!("https://github.com/tenmoku-editor/tenmoku/releases/tag/{tag}"),
        "draft": draft,
        "prerelease": tag.contains('-'),
    })
}

fn releases(tags: &[&str]) -> Value {
    Value::Array(tags.iter().map(|tag| release(tag, false)).collect())
}

const NONE: &str = "No release was found on GitHub";

#[gpui::test]
fn stable_channel_ignores_a_newer_preview(cx: &mut TestAppContext) {
    let (_settings, cx) = window("stable", cx);
    // A preview, even a newer one, is no release on stable: the reply is the
    // latest release, which GitHub never makes a preview.
    assert_eq!(check("0.2.0", release("v0.3.0-preview.4", false), cx), NONE);
    // The latest, as today: its tag is `vX.Y.Z` only, and it's offered when
    // newer, a running preview being older than its own stable release.
    assert_eq!(
        check("0.3.0-preview.2", release("v0.3.0", false), cx),
        "Tenmoku 0.3.0 is available (you have 0.3.0-preview.2)"
    );
    assert_eq!(
        check("0.3.0", release("v0.3.0", false), cx),
        "Tenmoku is up to date (you have 0.3.0)"
    );
}

#[gpui::test]
fn preview_channel_offers_a_newer_preview(cx: &mut TestAppContext) {
    let (_settings, cx) = window("preview", cx);
    // The newest by version, wherever it is in the list.
    let reply = releases(&["v0.3.0-preview.3", "v0.3.0-preview.10", "v0.2.0"]);
    assert_eq!(
        check("0.3.0-preview.2", reply, cx),
        "Tenmoku 0.3.0-preview.10 (preview) is available (you have 0.3.0-preview.2)"
    );
}

#[gpui::test]
fn preview_channel_offers_the_stable_that_follows_the_running_preview(cx: &mut TestAppContext) {
    let (_settings, cx) = window("preview", cx);
    let reply = releases(&["v0.3.0", "v0.3.0-preview.4", "v0.2.0"]);
    assert_eq!(
        check("0.3.0-preview.4", reply, cx),
        "Tenmoku 0.3.0 (preview) is available (you have 0.3.0-preview.4)"
    );
}

#[gpui::test]
fn preview_channel_never_offers_a_downgrade(cx: &mut TestAppContext) {
    let (_settings, cx) = window("preview", cx);
    // The running build is ahead of every release, or the newest of them.
    let reply = releases(&["v0.3.0-preview.4", "v0.2.0"]);
    assert_eq!(
        check("0.3.0", reply.clone(), cx),
        "Tenmoku is up to date (you have 0.3.0)"
    );
    assert_eq!(
        check("0.3.0-preview.4", reply, cx),
        "Tenmoku is up to date (you have 0.3.0-preview.4)"
    );
}

#[gpui::test]
fn draft_and_malformed_tags_are_skipped_on_both_channels(cx: &mut TestAppContext) {
    let (settings, cx) = window("preview", cx);
    let mut reply = releases(&[
        "v0.9.0-rc.1",
        "v0.8",
        "0.7.0",
        "v0.6.0-preview",
        "v0.5.0-beta.1",
    ]);
    reply
        .as_array_mut()
        .unwrap()
        .insert(0, release("v1.0.0", true));
    reply
        .as_array_mut()
        .unwrap()
        .push(release("v0.3.0-preview.4", false));
    assert_eq!(
        check("0.2.0", reply, cx),
        "Tenmoku 0.3.0-preview.4 (preview) is available (you have 0.2.0)"
    );
    // Nothing but those: no release.
    let reply = Value::Array(vec![release("v1.0.0", true), release("v0.8", false)]);
    assert_eq!(check("0.2.0", reply, cx), NONE);

    // Stable skips them too: its latest release is the one reply.
    settings.channel("stable", cx);
    assert_eq!(check("0.2.0", release("v1.0.0", true), cx), NONE);
    assert_eq!(check("0.2.0", release("v0.8", false), cx), NONE);
    assert_eq!(check("0.2.0", release("0.7.0", false), cx), NONE);
}
