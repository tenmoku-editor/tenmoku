# Release channels

Tenmoku has two release channels. **Stable**, the default, gets stable releases only.
**Preview** gets the newest build, preview or stable: previews can come more often, and may be
less polished. The editor's `update.channel` setting picks one.

## Versions

| Version | Tag | GitHub Release | Offered to |
| --- | --- | --- | --- |
| `0.3.0-preview.N` | `v0.3.0-preview.N` | a prerelease, never "latest" | Preview |
| `0.3.0` | `v0.3.0` | a release, "latest" when first published | Stable and Preview |

`N` is a number with no leading zero. No other form is accepted (not `-preview`, not `-rc.1`).
Versions order as `0.2.5` < `0.3.0-preview.1` < `0.3.0-preview.2` < `0.3.0`: a preview is newer
than the stable release before it and older than its own stable. So once `0.3.0` is out, the next
preview is of a later version, such as `0.3.1-preview.1` or `0.4.0-preview.1`.

## When to cut which

- **A preview**, to put what is on `main` in front of preview users ahead of a stable release.
  Name the stable release it leads up to and count `N` up from 1. After a fix, cut the next `N`
  rather than moving a tag.
- **A stable**, when that build is ready for everyone. It goes to both channels, so preview users
  move to it by an ordinary update.
- **A stable older than the newest preview** (say `0.3.1` while the preview channel holds
  `0.4.0-preview.2`) still publishes for stable users, but the preview channel never moves
  backwards: the workflow's `preview-feed` job refuses it and the run shows as failed. That is
  expected. Preview users get those fixes only in a newer preview or stable that has them.

## Cutting a preview

These steps cut `0.3.1-preview.1`; use the version you are cutting.

1. Draft the notes. The draft starts at the nearest `v*` tag behind `HEAD`, preview or stable:

   ```zsh
   scripts/draft-release-notes.sh --version 0.3.1-preview.1
   ```

2. Edit `release-notes/0.3.1-preview.1.md` into what a user would notice, by the rules in
   [macos-signing.md](macos-signing.md#releasing), then check it:

   ```zsh
   scripts/check-release-version.sh --notes 0.3.1-preview.1
   ```

3. In `Cargo.toml`, set `version` under `[workspace.package]` to `0.3.1-preview.1`, and refresh
   `Cargo.lock`:

   ```zsh
   cargo check --workspace
   ```

4. Commit `Cargo.toml`, `Cargo.lock` and the notes as one version bump commit, and land it
   through the gate:

   ```zsh
   git add Cargo.toml Cargo.lock release-notes/0.3.1-preview.1.md
   git commit -m "Bump the workspace version to 0.3.1-preview.1"
   ```

5. Tag the landed commit and push the tag:

   ```zsh
   git tag v0.3.1-preview.1
   scripts/check-release-version.sh v0.3.1-preview.1
   git push origin v0.3.1-preview.1
   ```

`.github/workflows/release.yml` then builds the same four platforms as a stable release,
publishes a GitHub prerelease with the preview feeds only, and its `preview-feed` job copies those
feeds to the rolling `preview` release (below).

**Cutting a stable** is the same with `X.Y.Z`; [macos-signing.md](macos-signing.md#releasing) has
the full steps and the signing secrets. One difference after previews: stable users haven't seen
the previews' notes, so draft from the previous stable tag, not from the last preview:

```zsh
scripts/draft-release-notes.sh --since v0.3.0 --version 0.4.0
```

## Where the feeds live

| Updater | Stable feed | Preview feed |
| --- | --- | --- |
| Sparkle (macOS) | `https://github.com/tenmoku-editor/tenmoku/releases/latest/download/appcast.xml` | `https://github.com/tenmoku-editor/tenmoku/releases/download/preview/appcast-preview.xml` |
| Velopack (Windows, Linux) | `https://github.com/tenmoku-editor/tenmoku/releases/latest/download/releases.<os>-<arch>-stable.json` | `https://github.com/tenmoku-editor/tenmoku/releases/download/preview/releases.<os>-<arch>-preview.json` |

`<os>-<arch>` is `windows-x64`, `windows-arm64` or `linux-x86_64`.

GitHub's `releases/latest/download` never serves a prerelease, so the preview feeds are in one
**rolling prerelease tagged `preview`**. After every tagged release, preview or stable,
`scripts/publish-preview-feed.sh` replaces what is there with that build's preview feeds, so the
preview feed carries each stable release too (unless it is older than what the channel holds). The
release holds `appcast-preview.xml`, the three Velopack preview feeds with their `.sig` files
when signed, and the `.nupkg` packages they list. The macOS zip stays in the tagged release,
which the appcast points to. The first tagged release to run the job creates the rolling release;
until then the preview URLs answer 404.

Don't download from the rolling release, and don't edit the `Preview build: version=... tag=...`
line in its notes: the script reads it to refuse an older build.

The feed URLs are built into the app. The repository variables `TENMOKU_PREVIEW_FEED_URL` (macOS,
the whole appcast URL) and `TENMOKU_UPDATE_FEED_PREVIEW` (Windows and Linux, the directory)
override the preview ones; the workflow still publishes only to GitHub, so copying the files to
another host is then up to you.

## Switching channels (users)

Open **Settings > Update > Channel** (Settings is <kbd>⌘,</kbd> on macOS, <kbd>Ctrl+,</kbd> on
Windows and Linux) and choose `preview` or `stable`. Or in your user `settings.toml`:

```toml
[update]
channel = "preview"   # or "stable", the default
```

It is a user setting only: a project's settings file can't set it. It needs no restart and applies
to the next update check:

- **macOS:** Sparkle's daily check (while `update.automatic_checks` is on), or Tenmoku >
  **Check for Updates…**.
- **Windows (Setup.exe install) and Linux (AppImage):** **Check for Updates…**. It downloads
  the update, then quits the app (asking about unsaved files) and restarts it updated.

The setting only works where the app updates itself: a macOS release with Sparkle signing, or a
Windows or Linux release whose feeds are signed with the update key (see
[platforms.md](platforms.md)). Otherwise, including the portable zip and the tar.gz,
**Check for Updates…** asks GitHub for the latest stable release whatever the channel says. To
try a preview there, download it from its prerelease on the
[releases page](https://github.com/tenmoku-editor/tenmoku/releases).

Installing a preview by hand does not change the setting. Choose `preview` too, or the app only
offers stable releases newer than that preview.

## Going back to stable

**Choosing Stable does not downgrade.** The editor stays on the preview you have until the next
stable release **newer than it**, and updates to that one. For example, on `0.4.0-preview.2` you
wait for `0.4.0` or later; a `0.3.1` released meanwhile is not offered.

If you stay on Preview, you receive that stable anyway: the preview feed carries every stable
release newer than the channel's build, so it arrives as an ordinary update. You are then on the
stable version, and the next preview is offered when there is one.

**To be on stable immediately,** install the latest stable download over the preview:

1. Choose `stable` in Settings > Update > Channel first, or the next check offers the preview
   again. Quit Tenmoku.
2. Download your platform's file from the
   [latest release](https://github.com/tenmoku-editor/tenmoku/releases/latest), which is always a
   stable one. The release's **Downloads** paragraph says which file is which, as does
   [platforms.md](platforms.md).
3. Install it over what you have:
   - **macOS:** unzip it and replace `Tenmoku.app` in Applications.
   - **Windows:** run `Setup.exe`. It says a newer version is installed; confirm the downgrade.
     For the portable zip, replace the folder.
   - **Linux:** replace the AppImage file, or unpack the tar.gz and run its `install.sh`.

Your settings stay.

## Known gaps

- **The GitHub fallback ignores the channel.** Without a self-updater, Check for Updates… looks at
  stable releases only, and on a preview build it reports that the running version can't be
  compared.
