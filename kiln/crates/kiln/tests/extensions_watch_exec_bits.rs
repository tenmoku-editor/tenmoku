// SPDX-License-Identifier: Apache-2.0

//! A chmod-only change to a listed package file: the execute bits fire the
//! extension reload, so consent is checked again; the write bit does not.

#![cfg(unix)]

use std::{
    cell::Cell,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    rc::Rc,
};

use gpui::TestAppContext;
use kiln::{
    Config,
    core::{ApiVersion, extension::MANIFEST_FILE},
    platform::config::{BatchSender, batch_channel},
};

mod notes {
    kiln::commands!(
        notes,
        [
            /// New Note
            NewNote,
        ]
    );
}

/// Starts an app whose extension `acme.one` lists `server`, with the file at
/// `mode`, then sets `after` and reports the `on_extensions` calls before
/// and after a batch.
fn reloads(cx: &mut TestAppContext, mode: u32, after: u32) -> (usize, usize) {
    let state = tempfile::tempdir().unwrap();
    let root = state.path().join("extensions");
    let dir = root.join("one");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(MANIFEST_FILE),
        "id = \"acme.one\"\nname = \"one\"\nversion = \"1.0.0\"\nlicense = \"MIT\"\n\n\
         [engine]\nnotes = \"1.0\"\n",
    )
    .unwrap();
    let server = dir.join("server");
    fs::write(&server, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&server, fs::Permissions::from_mode(mode)).unwrap();
    let calls = Rc::new(Cell::new(0));
    let (sender, batches): (BatchSender, _) = batch_channel();
    let config = Config::new("com.example.Notes", "Notes")
        .commands(notes::register)
        .extensions(&root, "notes", ApiVersion { major: 1, minor: 0 })
        .extension_files(|_| vec![PathBuf::from("server")])
        .on_extensions({
            let calls = calls.clone();
            move |_, _| calls.set(calls.get() + 1)
        })
        .watch_with(batches);
    cx.update(|cx| kiln::init(cx, config));
    cx.run_until_parked();
    send(cx, &sender, &root);
    let before = calls.get();
    fs::set_permissions(&server, fs::Permissions::from_mode(after)).unwrap();
    send(cx, &sender, &root);
    (before, calls.get())
}

fn send(cx: &mut TestAppContext, sender: &BatchSender, root: &Path) {
    sender.send(vec![root.to_owned()]);
    cx.run_until_parked();
}

#[gpui::test]
fn an_execute_bit_change_reloads(cx: &mut TestAppContext) {
    let (before, after) = reloads(cx, 0o644, 0o755);
    assert_eq!(after, before + 1);
}

#[gpui::test]
fn a_write_bit_change_does_not_reload(cx: &mut TestAppContext) {
    let (before, after) = reloads(cx, 0o644, 0o444);
    assert_eq!(after, before);
}

#[gpui::test]
fn dropping_one_of_several_execute_bits_reloads(cx: &mut TestAppContext) {
    let (before, after) = reloads(cx, 0o755, 0o744);
    assert_eq!(after, before + 1);
}
