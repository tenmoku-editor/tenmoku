//! Sparkle, the macOS updater: found in the bundle at launch, loaded at run
//! time rather than linked, so that `cargo run`, tests, Linux and bundles
//! without it stay as they were. With it running, Check for Updates… is
//! Sparkle's and `update.automatic_checks` turns its daily check on and off;
//! otherwise Check for Updates… asks GitHub (`fe_workbench::update_check`).
//! `update.channel` picks its feed: the Info.plist's `SUFeedURL` for stable,
//! its `TenmokuPreviewFeedURL` for preview.

use std::path::{Path, PathBuf};

use fe_config::settings::ReleaseChannel;
use gpui::App;

/// Where `Sparkle.framework` is when the executable `exe` sits in a bundle's
/// `X.app/Contents/MacOS/`, and the framework is there.
pub(crate) fn framework_path(exe: &Path) -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let macos = exe.parent().filter(|dir| dir.ends_with("MacOS"))?;
    let contents = macos.parent().filter(|dir| dir.ends_with("Contents"))?;
    contents
        .parent()
        .filter(|app| app.extension().is_some_and(|ext| ext == "app"))?;
    let framework = contents.join("Frameworks/Sparkle.framework");
    framework.is_dir().then_some(framework)
}

/// What Sparkle does in this process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Policy {
    /// Not loaded: Check for Updates… asks GitHub.
    Off,
    /// Running for Check for Updates…, with its daily check off.
    Manual,
    /// Running, with its daily check on.
    Scheduled,
}

impl Policy {
    /// Off for a `--perf` or `--wait` launch and under test, or without a
    /// bundled framework and the feed keys it needs. Otherwise, the
    /// `update.automatic_checks` setting picks.
    pub(crate) fn decide(
        bundled_framework: bool,
        has_feed_keys: bool,
        perf: bool,
        wait_launch: bool,
        under_test: bool,
        automatic_checks: bool,
    ) -> Policy {
        if !bundled_framework || !has_feed_keys || perf || wait_launch || under_test {
            Policy::Off
        } else if automatic_checks {
            Policy::Scheduled
        } else {
            Policy::Manual
        }
    }
}

/// The `SULastCheckTime` to write before Sparkle starts, in seconds since the
/// Unix epoch, so that its first automatic check comes one interval after the
/// first launch rather than at once. A fresh install has no last check, which
/// Sparkle reads as "never" and checks for at once; a last check in the future
/// (the clock was set back) would delay checks for as long. A past last check is
/// Sparkle's own record, and is left alone.
///
/// Only the macOS platform module calls it; the tests run it everywhere.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn first_check_seed(last_check: Option<f64>, now: f64) -> Option<f64> {
    match last_check {
        Some(last) if last <= now => None,
        _ => Some(now),
    }
}

/// The feed Sparkle checks, as [`feed_for`] picks it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FeedChoice {
    /// The Info.plist's `SUFeedURL`, which Sparkle reads itself: the
    /// delegate answers nil. Holds that URL, for the log.
    Stable(String),
    /// The preview feed, which the delegate answers.
    Preview(String),
}

impl FeedChoice {
    /// The URL Sparkle will fetch.
    pub(crate) fn url(&self) -> &str {
        match self {
            FeedChoice::Stable(url) | FeedChoice::Preview(url) => url,
        }
    }
}

/// The feed for `channel`, given the bundle's `stable` (`SUFeedURL`) and
/// `preview` (`TenmokuPreviewFeedURL`) feeds. Preview takes the preview feed
/// when it is an https URL, as bundle-macos.sh writes it; a missing or other
/// one falls back to stable rather than to a feed Sparkle would fetch in the
/// clear or not at all. Switching back to stable never downgrades: Sparkle
/// offers only versions newer than the running one.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn feed_for(channel: ReleaseChannel, stable: &str, preview: Option<&str>) -> FeedChoice {
    match (channel, preview) {
        (ReleaseChannel::Preview, Some(preview)) if is_https_url(preview) => {
            FeedChoice::Preview(preview.to_owned())
        }
        _ => FeedChoice::Stable(stable.to_owned()),
    }
}

/// `https://` and a host, with no whitespace: what bundle-macos.sh accepts.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn is_https_url(url: &str) -> bool {
    url.strip_prefix("https://")
        .is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/'))
        && !url.contains(char::is_whitespace)
}

/// Starts Sparkle as [`Policy::decide`] says, on the main thread. A symlink's
/// launch counts as its bundle's: 'tenmoku' in PATH is one.
pub(crate) fn start(
    perf: bool,
    wait_launch: bool,
    under_test: bool,
    automatic_checks: bool,
    cx: &mut App,
) {
    let framework = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .ok()
        .and_then(|exe| framework_path(&exe));
    let policy = Policy::decide(
        framework.is_some(),
        framework.is_some() && platform::has_feed_keys(),
        perf,
        wait_launch,
        under_test,
        automatic_checks,
    );
    if let (Some(framework), Policy::Manual | Policy::Scheduled) = (framework, policy) {
        platform::start(&framework, policy == Policy::Scheduled, cx);
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use std::path::Path;

    use gpui::App;

    pub(super) fn has_feed_keys() -> bool {
        false
    }

    pub(super) fn start(_framework: &Path, _automatic_checks: bool, _cx: &mut App) {}
}

#[cfg(target_os = "macos")]
mod platform {
    use std::{cell::Cell, path::Path, ptr, sync::Arc};

    use fe_config::{Settings, settings::ReleaseChannel};
    use fe_workbench::update_check::{UpdaterHook, set_updater_hook};
    use gpui::{App, Global};
    use kiln::GlobalSettings;
    use objc2::{
        ClassType, DeclaredClass, declare_class, msg_send, msg_send_id, mutability,
        rc::{Allocated, Retained},
        runtime::{AnyObject, NSObject, NSObjectProtocol},
    };
    use objc2_foundation::{
        MainThreadMarker, NSBundle, NSClassFromString, NSDate, NSString, NSUserDefaults,
    };

    use super::{FeedChoice, feed_for, first_check_seed};

    declare_class!(
        /// The updater's `SPUUpdaterDelegate`, which picks the feed for each
        /// check from `update.channel`. Its one ivar is the setting, which
        /// the settings observer in [`start`] keeps current, so the next
        /// check follows a change or a reload without a restart.
        struct FeedDelegate;

        // SAFETY: NSObject has no subclassing requirements. The class is
        // main-thread only, as Sparkle calls its delegate there, and has no
        // Drop.
        unsafe impl ClassType for FeedDelegate {
            type Super = NSObject;
            type Mutability = mutability::MainThreadOnly;
            const NAME: &'static str = "TenmokuSparkleFeedDelegate";
        }

        impl DeclaredClass for FeedDelegate {
            type Ivars = Cell<ReleaseChannel>;
        }

        unsafe impl FeedDelegate {
            /// The preview feed, or nil for Sparkle's own `SUFeedURL`. Sparkle
            /// asks at each check.
            #[method_id(feedURLStringForUpdater:)]
            fn feed_url_string(&self, _updater: &AnyObject) -> Option<Retained<NSString>> {
                let channel = self.ivars().get();
                let stable = info_string("SUFeedURL").unwrap_or_default();
                let choice = feed_for(channel, &stable, info_string(PREVIEW_FEED_KEY).as_deref());
                log::info!(
                    target: "fastedit",
                    "Sparkle checks the {} feed: {}",
                    match channel {
                        ReleaseChannel::Stable => "stable",
                        ReleaseChannel::Preview => "preview",
                    },
                    choice.url()
                );
                match choice {
                    FeedChoice::Stable(_) => None,
                    FeedChoice::Preview(url) => Some(NSString::from_str(&url)),
                }
            }
        }

        unsafe impl NSObjectProtocol for FeedDelegate {}
    );

    impl FeedDelegate {
        fn new(channel: ReleaseChannel, mtm: MainThreadMarker) -> Retained<Self> {
            let this = mtm.alloc::<Self>().set_ivars(Cell::new(channel));
            // SAFETY: NSObject's designated initializer.
            unsafe { msg_send_id![super(this), init] }
        }
    }

    /// The Info.plist key bundle-macos.sh writes the preview feed to.
    const PREVIEW_FEED_KEY: &str = "TenmokuPreviewFeedURL";

    /// The main bundle's Info.plist string at `key`, if it has one.
    fn info_string(key: &str) -> Option<String> {
        let bundle = NSBundle::mainBundle();
        // SAFETY: a read of the main bundle's Info.plist, and a cast to the
        // class just checked.
        unsafe {
            let value = bundle.objectForInfoDictionaryKey(&NSString::from_str(key))?;
            let is_string: bool = msg_send![&*value, isKindOfClass: NSString::class()];
            is_string.then(|| Retained::cast::<NSString>(value).to_string())
        }
    }

    /// The running updater, kept for the life of the app.
    struct Updater {
        /// An `SPUStandardUpdaterController`.
        controller: Retained<AnyObject>,
        /// Its updater's delegate, which Sparkle holds weakly.
        delegate: Retained<FeedDelegate>,
        /// The setting as last given to Sparkle.
        automatic_checks: bool,
    }

    impl Global for Updater {}

    impl Updater {
        /// Gives Sparkle the setting, which it also keeps in its own user
        /// defaults: a launch can't assume they already agree.
        ///
        /// With `only_if_changed`, Sparkle isn't told what it already has:
        /// setting it resets its update cycle, and so its schedule.
        fn set_automatic_checks(&mut self, automatic_checks: bool, only_if_changed: bool) {
            self.automatic_checks = automatic_checks;
            // SAFETY: `updater` is a property of the controller, which returns
            // an `SPUUpdater`, whose `automaticallyChecksForUpdates` property
            // is a BOOL.
            unsafe {
                let updater: Retained<AnyObject> = msg_send_id![&*self.controller, updater];
                if only_if_changed {
                    let current: bool = msg_send![&*updater, automaticallyChecksForUpdates];
                    if current == automatic_checks {
                        return;
                    }
                }
                let () = msg_send![&*updater, setAutomaticallyChecksForUpdates: automatic_checks];
            }
        }

        /// Starts the updater, which the controller was made without starting
        /// so that a failure (an invalid public key, a missing XPC service)
        /// comes back here rather than as a controller that does nothing.
        fn start_updater(&self) -> Result<(), String> {
            // SAFETY: `startUpdater:` takes an NSError out-pointer and returns
            // a BOOL.
            unsafe {
                let updater: Retained<AnyObject> = msg_send_id![&*self.controller, updater];
                let mut error: *mut AnyObject = ptr::null_mut();
                let ok: bool = msg_send![&*updater, startUpdater: &mut error];
                if ok {
                    return Ok(());
                }
                let description = (!error.is_null())
                    .then(|| {
                        let text: Option<Retained<NSString>> =
                            msg_send_id![&*error, localizedDescription];
                        text.map(|text| text.to_string())
                    })
                    .flatten();
                Err(description.unwrap_or_else(|| "its updater did not start".into()))
            }
        }

        /// Starts Sparkle's user-driven check, in its own windows.
        fn check(&self) {
            let sender: *const AnyObject = ptr::null();
            // SAFETY: the controller's action takes a nullable sender.
            let () = unsafe { msg_send![&*self.controller, checkForUpdates: sender] };
        }
    }

    /// Whether the app's Info.plist has the keys Sparkle needs: without them,
    /// starting it would only show the user an error.
    pub(super) fn has_feed_keys() -> bool {
        let bundle = NSBundle::mainBundle();
        ["SUFeedURL", "SUPublicEDKey"].iter().all(|key| {
            // SAFETY: a read of the main bundle's Info.plist.
            unsafe { bundle.objectForInfoDictionaryKey(&NSString::from_str(key)) }.is_some()
        })
    }

    /// Seeds `SULastCheckTime` in the app's user defaults when
    /// [`first_check_seed`] says so. Sparkle reads it when the updater starts.
    fn seed_last_check() {
        // SAFETY: NSUserDefaults and NSDate calls with matching types; the key
        // holds an NSDate when Sparkle wrote it.
        unsafe {
            let defaults = NSUserDefaults::standardUserDefaults();
            let key = NSString::from_str("SULastCheckTime");
            let now = NSDate::now().timeIntervalSince1970();
            let last = defaults.objectForKey(&key).and_then(|value| {
                let is_date: bool = msg_send![&*value, isKindOfClass: NSDate::class()];
                // SAFETY: just checked to be an NSDate.
                is_date.then(|| {
                    let secs: f64 = msg_send![&*value, timeIntervalSince1970];
                    secs
                })
            });
            if let Some(seed) = first_check_seed(last, now) {
                defaults
                    .setObject_forKey(Some(&*NSDate::dateWithTimeIntervalSince1970(seed)), &key);
            }
        }
    }

    /// Loads the framework and creates its standard updater controller, not
    /// yet started. If that fails, as it does for a bundle whose signature the
    /// framework's doesn't match, no hook is set and Check for Updates… asks
    /// GitHub as before.
    fn load(
        framework: &Path,
        delegate: &FeedDelegate,
    ) -> Result<Retained<AnyObject>, &'static str> {
        let path = NSString::from_str(&framework.to_string_lossy());
        // SAFETY: on the main thread, as the controller's creator must be. The
        // framework is the one signed into this bundle, and the initializer
        // takes a BOOL and two nullable delegates.
        unsafe {
            let bundle = NSBundle::bundleWithPath(&path).ok_or("not a bundle")?;
            if !bundle.load() {
                return Err("it did not load");
            }
            let class = NSClassFromString(&NSString::from_str("SPUStandardUpdaterController"))
                .as_ref()
                .ok_or("it has no SPUStandardUpdaterController")?;
            let no_delegate: *const AnyObject = ptr::null();
            let controller: Allocated<AnyObject> = msg_send_id![class, alloc];
            let controller: Option<Retained<AnyObject>> = msg_send_id![
                controller,
                initWithStartingUpdater: false,
                updaterDelegate: delegate,
                userDriverDelegate: no_delegate
            ];
            controller.ok_or("its updater controller did not initialize")
        }
    }

    /// Starts the updater from `framework`, takes over Check for Updates… and
    /// follows the setting from here on.
    pub(super) fn start(framework: &Path, automatic_checks: bool, cx: &mut App) {
        if automatic_checks {
            seed_last_check();
        }
        let Some(mtm) = MainThreadMarker::new() else {
            log::warn!(
                target: "fastedit",
                "Sparkle at {}: not on the main thread; Check for Updates… will ask GitHub",
                framework.display()
            );
            return;
        };
        // The config service may not have set its global yet, and Sparkle
        // asks for the feed as its updater starts. The channel is user-only
        // (a project file can't set it), so the user's file is the value
        // until the observer below gives the service's.
        let channel = cx
            .try_global::<GlobalSettings<Settings>>()
            .map(|global| global.settings().update.channel)
            .unwrap_or_else(|| {
                fe_config::SettingsStore::load(fe_config::user_settings_path(), None)
                    .settings()
                    .update
                    .channel
            });
        let delegate = FeedDelegate::new(channel, mtm);
        let controller = match load(framework, &delegate) {
            Ok(controller) => controller,
            Err(why) => {
                log::warn!(
                    target: "fastedit",
                    "Sparkle at {}: {why}; Check for Updates… will ask GitHub",
                    framework.display()
                );
                return;
            }
        };
        let mut updater = Updater {
            controller,
            delegate,
            automatic_checks,
        };
        // Before the updater starts, so before its first scheduled look. A
        // scheduled launch leaves a value Sparkle already has alone; a manual
        // one overrides the plist's SUEnableAutomaticChecks always.
        updater.set_automatic_checks(automatic_checks, automatic_checks);
        if let Err(why) = updater.start_updater() {
            log::warn!(
                target: "fastedit",
                "Sparkle at {}: {why}; Check for Updates… will ask GitHub",
                framework.display()
            );
            return;
        }
        cx.set_global(updater);
        set_updater_hook(
            cx,
            UpdaterHook(Arc::new(|cx| {
                cx.try_global::<Updater>().is_some_and(|updater| {
                    updater.check();
                    true
                })
            })),
        );
        cx.observe_global::<GlobalSettings<Settings>>(|cx| {
            let settings = cx.global::<GlobalSettings<Settings>>().settings().clone();
            // Read by the delegate at the next check.
            cx.global::<Updater>()
                .delegate
                .ivars()
                .set(settings.update.channel);
            let now = settings.update.automatic_checks;
            if cx.global::<Updater>().automatic_checks != now {
                cx.global_mut::<Updater>().set_automatic_checks(now, false);
            }
        })
        .detach();
    }
}
