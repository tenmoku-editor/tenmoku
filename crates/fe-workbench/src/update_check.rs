//! `app.check_for_updates`, Check for Updates…: asks GitHub for the latest
//! release and toasts how it compares with the running version, with a
//! Download button when it's newer. It only runs when asked: there is no
//! polling, no download and nothing kept.
//!
//! `update.channel` picks what counts. On stable, GitHub's latest release,
//! which is neither a draft nor a prerelease, with a tag of `vMAJOR.MINOR.PATCH`.
//! On preview, the newest by [`ReleaseVersion`] of the recent releases that
//! aren't drafts, tagged `vX.Y.Z` or `vX.Y.Z-preview.N`. A release's page must
//! be under [`RELEASE_PAGES`], as what's opened comes from the reply. Anything
//! else is no release found. A release is offered only if it's newer than the
//! running version, a preview being older than its own stable release.
//!
//! The network is behind [`UpdateSource`]: the app reaches GitHub through
//! `fe-net`, and anything on the test scheduler reaches only the fake it set,
//! so no test can touch the network. One check runs at a time in the app;
//! another asked for meanwhile answers [`already_checking`].
//!
//! The app binary can take the check over with an [`UpdaterHook`] (set by
//! [`set_updater_hook`]), as it owns a platform updater this crate can't
//! reach. A check asks the hook first: if it returns true, it has started its
//! own check and nothing else happens, with no toast and no GitHub request. If
//! there's no hook, or it returns false, the GitHub check runs as above.

use std::{sync::Arc, time::Duration};

use fe_config::{
    Settings,
    settings::ReleaseChannel,
    whats_new::{ReleaseVersion, parse_release_version},
};
use futures::{Stream, StreamExt as _, future::BoxFuture};
use gpui::{App, AppContext as _, AsyncApp, Entity, Global, SharedString};
use kiln::command::{Capability, Grants};
use kiln_l10n::tr;
use serde_json::Value;

use crate::workbench::toast::{self, Toast, ToastLayer};

/// The latest release: GitHub answers this directly, as `fe-net` follows no
/// redirects.
const LATEST_RELEASE: &str = "https://api.github.com/repos/tenmoku-editor/tenmoku/releases/latest";

/// The recent releases, newest first, for the preview channel, which GitHub
/// answers directly as well.
const RECENT_RELEASES: &str =
    "https://api.github.com/repos/tenmoku-editor/tenmoku/releases?per_page=10";

/// What a release's page must start with to be opened.
const RELEASE_PAGES: &str = "https://github.com/tenmoku-editor/tenmoku/releases/";

/// What the real fetcher may reach.
const CAPABILITY: &str = "net:api.github.com";

/// How long a read waits for GitHub's next bytes, well under `fe-net`'s
/// default, which is for replies that stream: a check holds up the next one.
const IDLE_TIMEOUT: Duration = Duration::from_secs(20);

/// The most of a reply that's read: a release's JSON is a few KiB, though its
/// notes can take it further.
pub(crate) const MAX_REPLY: usize = 4 * 1024 * 1024;

/// What a check answers while another's running.
pub(crate) fn already_checking() -> SharedString {
    tr!("workbench-extensions-update-already-checking")
}

/// What a check says when there's no release to compare with.
pub(crate) fn no_release() -> SharedString {
    tr!("workbench-extensions-update-no-release")
}

/// The Download button's title.
pub(crate) fn download() -> SharedString {
    tr!("workbench-extensions-update-download")
}

/// What a fetcher got: the HTTP status, and the body if that's 200.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fetched {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Asks for the latest release, or on the preview channel the recent ones; `Err` is why there's no reply to go by. Called
/// on the background executor.
pub type Fetcher = Arc<dyn Fn() -> BoxFuture<'static, Result<Fetched, String>> + Send + Sync>;

/// Where a check gets its answer, and the version it compares that with.
#[derive(Clone)]
pub struct UpdateSource {
    pub fetch: Fetcher,
    pub running: String,
}

impl Global for UpdateSource {}

impl UpdateSource {
    /// The one set, else the app's: GitHub, and this build's version. On the
    /// test scheduler, a fetcher that only fails, so that nothing but a fake
    /// answers a test.
    fn of(cx: &App, channel: ReleaseChannel) -> UpdateSource {
        if let Some(source) = cx.try_global::<UpdateSource>() {
            return source.clone();
        }
        let fetch: Fetcher = match cx.background_executor().is_test() {
            true => Arc::new(|| {
                Box::pin(async { Err(kiln_l10n::verbatim!("no fake fetcher is set").to_owned()) })
            }),
            false => Arc::new(move || Box::pin(fetch_latest(channel))),
        };
        UpdateSource {
            fetch,
            running: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }
}

/// The app binary's own updater: called with the app, it starts the platform's
/// user-driven check and returns true, or returns false to leave the check to
/// GitHub.
#[derive(Clone)]
pub struct UpdaterHook(pub Arc<dyn Fn(&mut App) -> bool + Send + Sync>);

impl Global for UpdaterHook {}

/// Sets the hook that `app.check_for_updates` asks first.
pub fn set_updater_hook(cx: &mut App, hook: UpdaterHook) {
    cx.set_global(hook);
}

/// Whether a check is running.
#[derive(Default)]
struct Checking(bool);

impl Global for Checking {}

/// How a check came out.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Newer {
        version: String,
        page: String,
        running: String,
        channel: ReleaseChannel,
    },
    UpToDate {
        running: String,
    },
    /// Nothing to compare with; `status` is an unsuccessful reply's that says
    /// so.
    NoRelease {
        status: Option<u16>,
    },
    /// Why there's no answer, one line.
    Failed(String),
}

/// Runs a check, unless one's running, saying it's going in `layer`.
pub(crate) fn run(layer: &Entity<ToastLayer>, cx: &mut App) {
    if let Some(hook) = cx.try_global::<UpdaterHook>().cloned()
        && (hook.0)(cx)
    {
        return;
    }
    if cx.default_global::<Checking>().0 {
        return toast::show(cx, Toast::info(already_checking()));
    }
    cx.default_global::<Checking>().0 = true;
    let channel = kiln_platform::config::settings::<Settings>(cx)
        .update
        .channel;
    let UpdateSource { fetch, running } = UpdateSource::of(cx, channel);
    let going = layer.update(cx, |layer, cx| {
        layer.push(Toast::info(tr!("workbench-extensions-update-checking")), cx)
    });
    let layer = layer.downgrade();
    let outcome = cx.background_spawn(async move { judge(fetch().await, &running, channel) });
    cx.spawn(async move |cx: &mut AsyncApp| {
        let outcome = outcome.await;
        cx.update(|cx| {
            cx.default_global::<Checking>().0 = false;
            (layer.update(cx, |layer, cx| layer.dismiss(going, cx))).ok();
            tell(outcome, cx);
        });
    })
    .detach();
}

/// Toasts `outcome`, in the active window: a window can have closed since it
/// started.
fn tell(outcome: Outcome, cx: &mut App) {
    let toast = match outcome {
        Outcome::Newer {
            version,
            page,
            running,
            channel,
        } => return offer(&version, &running, channel, page, cx),
        Outcome::UpToDate { running } => Toast::info(tr!(
            "workbench-extensions-update-up-to-date",
            running = running
        )),
        Outcome::NoRelease { status: None } => Toast::info(no_release()),
        Outcome::NoRelease {
            status: Some(status),
        } => Toast::info(tr!(
            "workbench-extensions-update-no-release-status",
            status = status.to_string()
        )),
        Outcome::Failed(reason) => {
            Toast::error(tr!("workbench-extensions-update-failed", reason = reason))
        }
    };
    toast::show(cx, toast);
}

/// Toasts that `version` is available, with a button opening `page`. It's a
/// toast's question, as its command-naming action couldn't carry the page, so
/// it stays until it's answered.
fn offer(version: &str, running: &str, channel: ReleaseChannel, page: String, cx: &mut App) {
    let (answer, answered) = futures::channel::oneshot::channel();
    let message = match channel {
        ReleaseChannel::Stable => tr!(
            "workbench-extensions-update-available",
            version = version,
            running = running
        ),
        ReleaseChannel::Preview => tr!(
            "workbench-extensions-update-available-preview",
            version = version,
            running = running
        ),
    };
    let offer = Toast::info(message).choices(vec![download().into()], move |choice| {
        answer.send(choice).ok();
    });
    toast::show(cx, offer);
    cx.spawn(async move |cx: &mut AsyncApp| {
        if answered.await == Ok(Some(0)) {
            cx.update(|cx| cx.open_url(&page));
        }
    })
    .detach();
}

/// What `fetched` says, at version `running`.
fn judge(fetched: Result<Fetched, String>, running: &str, channel: ReleaseChannel) -> Outcome {
    let Fetched { status, body } = match fetched {
        Ok(fetched) => fetched,
        Err(reason) => return Outcome::Failed(one_line(&reason)),
    };
    match status {
        200 => {}
        404 => {
            return Outcome::NoRelease {
                status: Some(status),
            };
        }
        403 | 429 => {
            return Outcome::Failed(
                tr!(
                    "workbench-extensions-update-refused",
                    status = status.to_string()
                )
                .into(),
            );
        }
        _ => {
            return Outcome::Failed(
                tr!(
                    "workbench-extensions-update-http-status",
                    status = status.to_string()
                )
                .into(),
            );
        }
    }
    let Ok(release) = serde_json::from_slice::<Value>(&body) else {
        return Outcome::Failed(
            tr!(
                "workbench-extensions-update-invalid-json",
                status = status.to_string()
            )
            .into(),
        );
    };
    let found = match channel {
        ReleaseChannel::Stable => release_of(&release, false),
        ReleaseChannel::Preview => release.as_array().and_then(|releases| {
            (releases
                .iter()
                .filter_map(|release| release_of(release, true)))
            .max_by_key(|found| found.1)
        }),
    };
    let Some((version, latest, page)) = found else {
        return Outcome::NoRelease { status: None };
    };
    let Some(current) = parse_release_version(running) else {
        return Outcome::Failed(
            tr!(
                "workbench-extensions-update-version-not-comparable",
                running = running
            )
            .into(),
        );
    };
    let running = running.to_owned();
    match latest > current {
        true => Outcome::Newer {
            version: version.to_owned(),
            page: page.to_owned(),
            running,
            channel,
        },
        false => Outcome::UpToDate { running },
    }
}

/// The version, as written and ordered, and the page of the release in
/// `reply`, if it counts as one: not a draft, and not a prerelease unless
/// `previews` (GitHub flags a preview build as one), with a tag of `vX.Y.Z`,
/// or on `previews` `vX.Y.Z-preview.N`.
fn release_of(reply: &Value, previews: bool) -> Option<(&str, ReleaseVersion, &str)> {
    let flag = |key: &str| reply[key].as_bool().unwrap_or(false);
    if flag("draft") || (flag("prerelease") && !previews) {
        return None;
    }
    let version = reply["tag_name"].as_str()?.strip_prefix('v')?;
    let ordered =
        parse_release_version(version).filter(|ordered| previews || ordered.pre.is_none())?;
    let page = reply["html_url"].as_str()?;
    page.starts_with(RELEASE_PAGES)
        .then_some((version, ordered, page))
}

/// `text` on one line, as a toast's reason.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Asks GitHub for the latest release, or on the preview channel the recent ones.
async fn fetch_latest(channel: ReleaseChannel) -> Result<Fetched, String> {
    let capability = Capability::parse(CAPABILITY).map_err(|error| error.to_string())?;
    let grants: Grants = std::iter::once(capability).collect();
    let client = fe_net::Client::new(&grants).with_idle_timeout(IDLE_TIMEOUT);
    let agent = format!("Tenmoku/{}", env!("CARGO_PKG_VERSION"));
    let headers = [
        ("user-agent", agent.as_str()),
        ("accept", "application/vnd.github+json"),
    ]
    .into_iter()
    .map(|(name, value)| Ok((name.parse()?, value.parse()?)))
    .collect::<Result<_, Box<dyn std::error::Error + Send + Sync>>>()
    .map_err(|error| error.to_string())?;
    let method = "GET"
        .parse()
        .map_err(|_| tr!("workbench-extensions-update-no-get").to_string())?;
    let url = match channel {
        ReleaseChannel::Stable => LATEST_RELEASE,
        ReleaseChannel::Preview => RECENT_RELEASES,
    };
    let reply = client.request(method, url, headers, Vec::new());
    let reply = reply.await.map_err(|error| error.to_string())?;
    read_reply(reply.status.as_u16(), reply.body).await
}

/// What a reply with `status` and the body `chunks` comes to. The body's read
/// only from a 200, the only reply it matters in; what stops it short says the
/// status, which is known by then.
pub(crate) async fn read_reply<E: std::fmt::Display>(
    status: u16,
    mut chunks: impl Stream<Item = Result<Vec<u8>, E>> + Unpin,
) -> Result<Fetched, String> {
    let mut body = Vec::new();
    if status == 200 {
        while let Some(chunk) = chunks.next().await {
            let chunk = chunk.map_err(|error| {
                tr!(
                    "workbench-extensions-update-broke-off",
                    error = error.to_string(),
                    status = status.to_string()
                )
                .to_string()
            })?;
            body.extend(chunk);
            if body.len() > MAX_REPLY {
                return Err(tr!(
                    "workbench-extensions-update-too-long",
                    status = status.to_string()
                )
                .into());
            }
        }
    }
    Ok(Fetched { status, body })
}
