// SPDX-License-Identifier: Apache-2.0

//! `[[contributes.plugin.bindings]]`: a plugin binding keys to its own
//! commands (T2106 C1).
//!
//! Every refusal here happens while the manifest is READ, before the plugin
//! is loaded. `manifest.rs` already states the reason for the sibling case:
//! a panel's `tick_ms` and `keys` are "declarations that commit the host to
//! a timer and to the user's keystrokes before the plugin runs at all, so
//! they are refused while the manifest is read". A key binding is exactly
//! that kind of declaration.

use std::{fs, path::Path};

use kiln_core::{ApiVersion, ExtensionApi, KeySequence, Manifest, load_manifest};
use kiln_plugin::manifest::{ExtensionOrigin, PluginManifest, PluginManifestError, plugin_kind};

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

fn dir_with(plugin: &str) -> tempdir::Dir {
    let dir = tempdir::Dir::new();
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

const PLUGIN: &str = "[contributes.plugin]\nwasm = \"p.wasm\"\n\
    capabilities = [\"buffer.read\", \"commands\"]\n";

/// The same plugin table without `commands`.
const PLUGIN_NO_COMMANDS: &str = "[contributes.plugin]\nwasm = \"p.wasm\"\n\
    capabilities = [\"buffer.read\"]\n";

/// A keystroke that passes the first-keystroke rule on every OS: it carries
/// ctrl, so it is never a bare typing key, and the app does not bind it.
const OK_KEYS: &str = "ctrl-alt-9";

fn with_bindings(bindings: &str) -> String {
    format!("{PLUGIN}\n{bindings}")
}

fn plugin_of(text: &str) -> Result<PluginManifest, PluginManifestError> {
    let dir = dir_with(text);
    let manifest = load(dir.path());
    PluginManifest::from_extension(&manifest, ExtensionOrigin::User)
        .map(|plugin| plugin.expect("the manifest declares a plugin"))
}

fn refusal(text: &str) -> PluginManifestError {
    plugin_of(text).expect_err("the entry should have been refused")
}

#[test]
fn a_binding_declares_keys_command_and_context() {
    let plugin = plugin_of(&with_bindings(
        "[[contributes.plugin.bindings]]\n\
         keys = \"ctrl-alt-9\"\n\
         command = \"acme.tool.run\"\n\
         context = \"editor && !vim.enabled\"\n",
    ))
    .unwrap();
    assert_eq!(plugin.bindings.len(), 1, "{:?}", plugin.bindings);
    let binding = &plugin.bindings[0];
    assert_eq!(binding.keys, KeySequence::parse(OK_KEYS).unwrap());
    assert_eq!(binding.command, "acme.tool.run");
    // Parsed here, not kept as text: a context that does not parse is
    // refused at read, so the host can trust what it gets.
    assert_eq!(
        binding.context.as_ref().map(ToString::to_string),
        Some("editor && !vim.enabled".to_owned())
    );

    // context is optional.
    let plugin = plugin_of(&with_bindings(
        "[[contributes.plugin.bindings]]\n\
         keys = \"ctrl-alt-9\"\n\
         command = \"acme.tool.run\"\n",
    ))
    .unwrap();
    assert!(plugin.bindings[0].context.is_none());
}

#[test]
fn keys_by_platform_are_accepted() {
    // A table by platform, as keymap.toml allows; read for the OS the
    // manifest is read on.
    let plugin = plugin_of(&with_bindings(
        "[[contributes.plugin.bindings]]\n\
         command = \"acme.tool.run\"\n\
         [contributes.plugin.bindings.keys]\n\
         macos = \"cmd-alt-9\"\n\
         linux = \"ctrl-alt-9\"\n\
         windows = \"ctrl-alt-9\"\n",
    ))
    .unwrap();
    assert_eq!(plugin.bindings.len(), 1, "{:?}", plugin.bindings);
    let expected = if cfg!(target_os = "macos") {
        "cmd-alt-9"
    } else {
        "ctrl-alt-9"
    };
    assert_eq!(
        plugin.bindings[0].keys,
        KeySequence::parse(expected).unwrap()
    );

    // `default` alone binds everywhere.
    let plugin = plugin_of(&with_bindings(
        "[[contributes.plugin.bindings]]\n\
         command = \"acme.tool.run\"\n\
         [contributes.plugin.bindings.keys]\n\
         default = \"ctrl-alt-9\"\n",
    ))
    .unwrap();
    assert_eq!(
        plugin.bindings[0].keys,
        KeySequence::parse(OK_KEYS).unwrap()
    );

    // A platform table naming none of this OS and no default binds nothing
    // here, and that is not an error: a macOS-only binding is legitimate.
    let plugin = plugin_of(&with_bindings(
        "[[contributes.plugin.bindings]]\n\
         command = \"acme.tool.run\"\n\
         [contributes.plugin.bindings.keys]\n\
         plan9 = \"ctrl-alt-9\"\n",
    ));
    match plugin {
        Ok(plugin) => assert!(
            plugin.bindings.is_empty(),
            "an entry for no known platform binds nothing: {:?}",
            plugin.bindings
        ),
        Err(error) => assert!(
            error.to_string().contains("plan9"),
            "if it is refused instead, it must name the key: {error}"
        ),
    }
}

#[test]
fn a_binding_to_another_namespace_is_refused() {
    // A plugin binds only its own commands. Binding an app command would
    // let a manifest reach the whole editor.
    let error = refusal(&with_bindings(
        "[[contributes.plugin.bindings]]\n\
         keys = \"ctrl-alt-9\"\n\
         command = \"editor.delete_all_right\"\n",
    ));
    let text = error.to_string();
    assert!(text.contains("editor.delete_all_right"), "{error}");
    assert!(text.contains("acme.tool"), "names the plugin: {error}");

    // The prefix must be the id plus a dot, not merely a textual prefix:
    // `acme.toolbar.run` belongs to a different extension.
    let error = refusal(&with_bindings(
        "[[contributes.plugin.bindings]]\n\
         keys = \"ctrl-alt-9\"\n\
         command = \"acme.toolbar.run\"\n",
    ));
    assert!(error.to_string().contains("acme.toolbar.run"), "{error}");

    // The bare id with nothing after the dot is not a command either.
    let error = refusal(&with_bindings(
        "[[contributes.plugin.bindings]]\n\
         keys = \"ctrl-alt-9\"\n\
         command = \"acme.tool\"\n",
    ));
    assert!(error.to_string().contains("acme.tool"), "{error}");
}

#[test]
fn a_binding_without_the_commands_capability_is_refused() {
    let error = refusal(&format!(
        "{PLUGIN_NO_COMMANDS}\n\
         [[contributes.plugin.bindings]]\n\
         keys = \"ctrl-alt-9\"\n\
         command = \"acme.tool.run\"\n"
    ));
    let text = error.to_string();
    assert!(text.contains("commands"), "names the capability: {error}");
    assert!(text.contains("acme.tool"), "names the plugin: {error}");
}

#[test]
fn a_bad_keystroke_string_is_refused() {
    let error = refusal(&with_bindings(
        "[[contributes.plugin.bindings]]\n\
         keys = \"ctrl-nosuchkey\"\n\
         command = \"acme.tool.run\"\n",
    ));
    assert!(error.to_string().contains("ctrl-nosuchkey"), "{error}");

    // Empty keys are missing keys, not a parse failure.
    let error = refusal(&with_bindings(
        "[[contributes.plugin.bindings]]\n\
         keys = \"\"\n\
         command = \"acme.tool.run\"\n",
    ));
    assert!(error.to_string().contains("keys"), "{error}");

    // No keys at all.
    let error = refusal(&with_bindings(
        "[[contributes.plugin.bindings]]\n\
         command = \"acme.tool.run\"\n",
    ));
    assert!(error.to_string().contains("keys"), "{error}");

    // f21 is past kiln-core's f1-f20, so it is a key that does not parse
    // rather than a binding that is refused for its modifiers.
    let error = refusal(&with_bindings(
        "[[contributes.plugin.bindings]]\n\
         keys = \"f21\"\n\
         command = \"acme.tool.run\"\n",
    ));
    assert!(error.to_string().contains("f21"), "{error}");

    // Not a string and not a platform table.
    let error = refusal(&with_bindings(
        "[[contributes.plugin.bindings]]\n\
         keys = 9\n\
         command = \"acme.tool.run\"\n",
    ));
    assert!(error.to_string().contains("keys"), "{error}");
}

#[test]
fn a_bad_context_is_refused() {
    let error = refusal(&with_bindings(
        "[[contributes.plugin.bindings]]\n\
         keys = \"ctrl-alt-9\"\n\
         command = \"acme.tool.run\"\n\
         context = \"editor &&\"\n",
    ));
    assert!(error.to_string().contains("editor &&"), "{error}");
}

#[test]
fn a_bare_typing_key_or_alt_only_key_is_refused() {
    // The first keystroke must be f1-f24, or carry ctrl or this platform's
    // secondary modifier.
    //
    // `alt` alone is refused on EVERY OS, not only macOS: there Option
    // composes characters, so a plugin taking alt-x would eat the user's
    // typing - and one rule on all platforms is also one rule for a plugin
    // author to read.
    for keys in [
        "x",
        "shift-z",
        "alt-x",
        "alt-shift-x",
        "escape",
        "tab",
        "enter",
    ] {
        let error = refusal(&with_bindings(&format!(
            "[[contributes.plugin.bindings]]\n\
             keys = \"{keys}\"\n\
             command = \"acme.tool.run\"\n"
        )));
        assert!(
            error.to_string().contains(keys),
            "{keys} should be refused and named: {error}"
        );
    }

    // f-keys need no modifier; ctrl or secondary make a typing key fine.
    //
    // The card says "f1-f24", but kiln-core's `Key::parse` accepts f1-f20
    // only (`(1..=20).contains(n)`, keystroke.rs), and `kiln-core/src/
    // keystroke.rs` is NOT in this card's scope. So the rule implemented
    // here is "any function key kiln-core can parse", and f21-f24 are
    // refused one step earlier, as keys that do not parse. Raised with the
    // Mac lead as a respec item rather than widening scope to the parser.
    for keys in ["f5", "f20", "ctrl-x", "ctrl-shift-x"] {
        let plugin = plugin_of(&with_bindings(&format!(
            "[[contributes.plugin.bindings]]\n\
             keys = \"{keys}\"\n\
             command = \"acme.tool.run\"\n"
        )))
        .unwrap_or_else(|error| panic!("{keys} should be accepted: {error}"));
        assert_eq!(plugin.bindings.len(), 1, "{keys}");
    }

    // Only the FIRST keystroke is restricted: a chord may continue with a
    // bare key, as the app's own `secondary-k z` does.
    let plugin = plugin_of(&with_bindings(
        "[[contributes.plugin.bindings]]\n\
         keys = \"ctrl-alt-9 z\"\n\
         command = \"acme.tool.run\"\n",
    ))
    .unwrap();
    assert_eq!(
        plugin.bindings[0].keys,
        KeySequence::parse("ctrl-alt-9 z").unwrap()
    );
}

#[test]
fn too_many_or_duplicate_bindings_are_refused() {
    // One distinct chord per entry. The second keystroke must be a single
    // key: `ctrl-alt-9 10` is not a keystroke, and the implementation
    // rightly refuses it - which is how this generator was first caught.
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let one = |n: usize| {
        let ch = ALPHABET[n] as char;
        format!(
            "[[contributes.plugin.bindings]]\n\
             keys = \"ctrl-alt-9 {ch}\"\n\
             command = \"acme.tool.run\"\n"
        )
    };

    // 32 is the cap, and 32 is allowed.
    let thirty_two: String = (0..32).map(one).collect();
    let plugin = plugin_of(&with_bindings(&thirty_two)).unwrap();
    assert_eq!(plugin.bindings.len(), 32);

    // 33 is not.
    let thirty_three: String = (0..33).map(one).collect();
    let error = refusal(&with_bindings(&thirty_three));
    assert!(error.to_string().contains("acme.tool"), "{error}");

    // The same keys AND context twice is worth naming: one of the two could
    // never run.
    let error = refusal(&with_bindings(
        "[[contributes.plugin.bindings]]\n\
         keys = \"ctrl-alt-9\"\n\
         command = \"acme.tool.run\"\n\
         context = \"editor\"\n\
         [[contributes.plugin.bindings]]\n\
         keys = \"ctrl-alt-9\"\n\
         command = \"acme.tool.other\"\n\
         context = \"editor\"\n",
    ));
    assert!(error.to_string().contains("ctrl-alt-9"), "{error}");

    // Duplicate with no context on either is the same mistake.
    let error = refusal(&with_bindings(
        "[[contributes.plugin.bindings]]\n\
         keys = \"ctrl-alt-9\"\n\
         command = \"acme.tool.run\"\n\
         [[contributes.plugin.bindings]]\n\
         keys = \"ctrl-alt-9\"\n\
         command = \"acme.tool.other\"\n",
    ));
    assert!(error.to_string().contains("ctrl-alt-9"), "{error}");

    // The same keys in DIFFERENT contexts is fine: that is how a binding is
    // narrowed, and the matcher picks by context depth.
    let plugin = plugin_of(&with_bindings(
        "[[contributes.plugin.bindings]]\n\
         keys = \"ctrl-alt-9\"\n\
         command = \"acme.tool.run\"\n\
         context = \"editor\"\n\
         [[contributes.plugin.bindings]]\n\
         keys = \"ctrl-alt-9\"\n\
         command = \"acme.tool.other\"\n\
         context = \"terminal\"\n",
    ))
    .unwrap();
    assert_eq!(plugin.bindings.len(), 2);
}

mod tempdir {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    pub(crate) struct Dir(PathBuf);

    impl Dir {
        pub(crate) fn new() -> Self {
            // Each manifest needs its own directory: several are read per
            // test here, and the sibling harness reuses one path per
            // thread, which would have the second read overwrite the first.
            let path = std::env::temp_dir().join(format!(
                "kiln-plugin-manifest-bindings-{}-{:?}-{}",
                std::process::id(),
                std::thread::current().id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
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
