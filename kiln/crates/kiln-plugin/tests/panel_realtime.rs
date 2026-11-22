// SPDX-License-Identifier: Apache-2.0

//! The realtime half of `[[contributes.plugin.panels]]`: `tick_ms` and `keys`,
//! and the `panels.realtime` capability that has to be asked for before either
//! may be declared.
//!
//! The refusal is a *manifest* refusal, not a runtime one. That is deliberate
//! and worth saying out loud, because it differs from `panels` itself:
//! `Capability::Panels` is checked when the guest calls the host
//! (by the app's panel host), not when the manifest is read, since showing a panel
//! is an operation. `tick_ms` and `keys` are not operations - they are static
//! declarations that commit the host to running a timer and routing keys - so
//! a manifest that asks for them without the capability is wrong on its face
//! and is refused while it is being read.

use std::{fs, path::Path};

use kiln_command::{Capability, Grants};
use kiln_core::{ApiVersion, ExtensionApi, Manifest, load_manifest};
use kiln_plugin::manifest::{
    ExtensionOrigin, PanelError, PluginManifest, PluginManifestError, plugin_kind,
};

fn api() -> ExtensionApi {
    ExtensionApi {
        app: "notes".into(),
        version: ApiVersion { major: 1, minor: 0 },
        reserved: vec![],
        kinds: vec![plugin_kind()],
        allowed_licenses: None,
        only_kinds: None,
    }
}

fn dir_with(name: &str, plugin: &str) -> tempdir::Dir {
    let dir = tempdir::Dir::new_named(name);
    let text = format!(
        "id = \"acme.tool\"\nname = \"Tool\"\nversion = \"1.0.0\"\nlicense = \"MIT\"\n\n\
         [engine]\nnotes = \"1.0\"\n\n{plugin}"
    );
    fs::write(dir.path().join("extension.toml"), text).unwrap();
    fs::write(dir.path().join("p.wasm"), b"\0asm").unwrap();
    dir
}

fn load(dir: &Path) -> Manifest {
    let (manifest, diagnostics) = load_manifest(dir, &api());
    manifest.unwrap_or_else(|| panic!("{diagnostics:?}"))
}

/// A `[contributes.plugin]` head asking for `capabilities`, followed by one
/// panel table carrying `panel`.
fn plugin_with(capabilities: &str, panel: &str) -> String {
    format!(
        "[contributes.plugin]\nwasm = \"p.wasm\"\ncapabilities = [{capabilities}]\n\n\
         [[contributes.plugin.panels]]\nid = \"notes\"\ntitle = \"Notes\"\nicon = \"book\"\n{panel}"
    )
}

fn parse(
    name: &str,
    capabilities: &str,
    panel: &str,
) -> Result<PluginManifest, PluginManifestError> {
    let dir = dir_with(name, &plugin_with(capabilities, panel));
    let manifest = load(dir.path());
    PluginManifest::from_extension(&manifest, ExtensionOrigin::User)
        .map(|plugin| plugin.expect("the entry contributes a plugin"))
}

/// The `reason` of a `PluginManifestError::Panel`, or a panic naming what came
/// back instead.
fn panel_error(error: PluginManifestError) -> PanelError {
    match error {
        PluginManifestError::Panel { reason, .. } => reason,
        other => panic!("expected a panel error, got {other:?}: {other}"),
    }
}

const REALTIME: &str = "\"panels\", \"panels.realtime\"";
const PANELS_ONLY: &str = "\"panels\"";

#[test]
fn realtime_fields_without_the_capability_are_refused() {
    // Each realtime field on its own is enough to need the capability: a panel
    // that only ticks and one that only reads keys are both refused.
    for panel in ["tick_ms = 100\n", "keys = true\n"] {
        let error = parse("no-cap", PANELS_ONLY, panel)
            .expect_err("realtime fields without `panels.realtime` must be refused");
        assert_eq!(
            panel_error(error),
            PanelError::RealtimeWithoutCapability("notes".to_owned()),
            "{panel}"
        );
    }
}

#[test]
fn the_refusal_names_the_panel_and_the_capability_it_needs() {
    let error = parse("names", PANELS_ONLY, "tick_ms = 100\n").unwrap_err();
    let text = error.to_string();
    // No literals in the message itself: this asserts the rendered ftl carries
    // the two things a plugin author needs in order to fix the manifest.
    assert!(text.contains("notes"), "should name the panel: {text}");
    assert!(
        text.contains("panels.realtime"),
        "should name the capability: {text}"
    );
}

#[test]
fn realtime_fields_are_accepted_with_the_capability() {
    let plugin = parse("with-cap", REALTIME, "tick_ms = 100\nkeys = true\n").unwrap();
    let panel = &plugin.panels[0];
    assert_eq!(panel.tick_ms, Some(100));
    assert!(panel.keys);
    assert!(plugin.capabilities.allows(&Capability::PanelsRealtime));
}

#[test]
fn a_panel_declaring_neither_field_needs_no_capability() {
    // The capability gates the realtime fields, not panels as such. A plain
    // panel keeps working for a plugin that never asks for `panels.realtime`.
    let plugin = parse("plain", PANELS_ONLY, "").unwrap();
    let panel = &plugin.panels[0];
    assert_eq!(panel.tick_ms, None);
    assert!(!panel.keys, "keys defaults to off");
}

#[test]
fn tick_ms_outside_the_range_is_refused_at_both_ends() {
    // 50..=1000. The values just outside are refused and the two on the
    // boundary are kept, so the test pins the ends rather than the middle.
    // -1 is here because TOML integers are signed: a negative tick must be
    // refused by the range check, not wrap on its way into a u32.
    for bad in [-1, 0, 49, 1001] {
        match parse("range", REALTIME, &format!("tick_ms = {bad}\n")) {
            Ok(plugin) => panic!(
                "tick_ms = {bad} should be refused, parsed as {:?}",
                plugin.panels[0].tick_ms
            ),
            Err(error) => assert_eq!(
                panel_error(error),
                PanelError::TickOutOfRange(bad),
                "tick_ms = {bad}"
            ),
        }
    }
}

#[test]
fn tick_ms_on_the_boundary_is_kept() {
    for good in [50, 1000] {
        let plugin = parse("bound", REALTIME, &format!("tick_ms = {good}\n"))
            .unwrap_or_else(|e| panic!("tick_ms = {good} should be accepted: {e}"));
        assert_eq!(plugin.panels[0].tick_ms, Some(good));
    }
}

#[test]
fn tick_ms_must_be_an_integer_and_keys_must_be_a_boolean() {
    let error = parse("types-tick", REALTIME, "tick_ms = \"100\"\n").unwrap_err();
    assert_eq!(panel_error(error), PanelError::NotAnInteger("tick_ms"));

    let error = parse("types-keys", REALTIME, "keys = \"yes\"\n").unwrap_err();
    assert_eq!(panel_error(error), PanelError::NotABool("keys"));
}

#[test]
fn the_capability_parses_takes_no_argument_and_round_trips() {
    assert_eq!(
        Capability::parse("panels.realtime").unwrap(),
        Capability::PanelsRealtime
    );
    assert_eq!(Capability::PanelsRealtime.to_string(), "panels.realtime");
    // Round trip, so Display and parse cannot drift apart.
    assert_eq!(
        Capability::parse(&Capability::PanelsRealtime.to_string()).unwrap(),
        Capability::PanelsRealtime
    );
    let error = Capability::parse("panels.realtime:notes").unwrap_err();
    assert_eq!(error.reason, "this capability takes no argument");
}

#[test]
fn the_capability_is_distinct_from_panels() {
    // `panels.realtime` must not be satisfied by holding `panels`, or the gate
    // would be decorative.
    let only_panels: Grants = [Capability::Panels].into_iter().collect();
    assert!(only_panels.allows(&Capability::Panels));
    assert!(!only_panels.allows(&Capability::PanelsRealtime));

    let only_realtime: Grants = [Capability::PanelsRealtime].into_iter().collect();
    assert!(!only_realtime.allows(&Capability::Panels));
}

#[test]
fn the_capability_has_a_consent_description() {
    // `describe` is an exhaustive match with no wildcard, so a new variant
    // without an arm will not compile. This asserts the arm renders a real
    // string rather than the ftl key falling through.
    let text = kiln_plugin::warnings::describe(&Capability::PanelsRealtime);
    assert!(!text.is_empty());
    assert!(
        !text.contains("kiln-plugin-capability"),
        "the ftl key leaked into the description: {text}"
    );
}

mod tempdir {
    use std::path::{Path, PathBuf};

    pub(crate) struct Dir(PathBuf);

    impl Dir {
        pub(crate) fn new_named(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "kiln-plugin-panel-realtime-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            std::fs::remove_dir_all(&path).ok();
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        pub(crate) fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }
}
