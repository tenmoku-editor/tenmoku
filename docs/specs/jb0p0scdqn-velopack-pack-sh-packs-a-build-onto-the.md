---
task_uid: "01m7th30f11yqm1gjb0p0scdqn"
task: "T1998"
rev: 2
type: "feature"
complexity: "m"
criteria:
  - id: "C1"
    text: "`velopack-pack.sh --linux` and velopack-pack.ps1 take `--channel-set stable|preview|both` (default stable, as today). A preview version (`X.Y.Z-preview.N`) packs only `\u003cos\u003e-\u003carch\u003e-preview`; a stable version packs `\u003cos\u003e-\u003carch\u003e-stable` and, with `both` (what the release workflow passes), `\u003cos\u003e-\u003carch\u003e-preview` too, so the preview feed carries the stable. The channel names are exactly `windows-x64-preview`, `windows-arm64-preview`, `linux-x86_64-preview`; each channel's feed is signed (`releases.\u003cchannel\u003e.json.sig`)."
    required: true
    manual: false
  - id: "C2"
    text: "A preview version never produces a `-stable` channel file: tested."
    required: true
    manual: false
  - id: "C3"
    text: "scripts/test-velopack-pack.sh (new, runs `--dry-run` on Linux, no vpk needed) asserts the vpk argument lists for stable, preview, and stable+both, and the signature steps."
    required: true
    manual: false
  - id: "C4"
    text: "Windows (ps1) behaviour is checked by its dry-run output in the same test where PowerShell is absent skipping with a clear message; real Windows packing is proven by CH9's CI run, so say in the finish note that nothing here ran vpk."
    required: true
    manual: false
---

# velopack-pack.sh packs a build onto the preview channel as well as stable (channels 6)

# Velopack channels

Existing: `scripts/velopack-pack.sh` refuses `--channel stable` and `osx` outside the Linux path and bakes channel `linux-x86_64-stable`; Windows channel names are in crates/fastedit/src/velopack_update.rs (`release_channel`). Read those, scripts/package-linux.sh and the vpk invocations first. Velopack versions are semver, so `0.3.0-preview.2` < `0.3.0` natively. Note the preview feed base must hold the .nupkg files its releases.json lists (CH10 uploads them to the rolling release).


Read the files named before starting. Do not widen --scope; if a file outside it is needed, stop and respec. New tests go in new files.

## Implementation notes

- The sh flag is `--channel-set`; the ps1 parameter is its PowerShell spelling, `-ChannelSet` (as `--version`/`-Version`, `--dry-run`/`-DryRun` already pair up). Both default to stable.
- A preview version with `stable` (the default) is refused ("a preview version goes on the preview channel only"), rather than quietly packed on preview: like make-appcast.sh's refusal, it catches a caller that forgot the set. `both` and `preview` pack it on `<os>-<arch>-preview` only.
- vpk packs once per channel into its own output directory. Each channel's `releases.<channel>.json`, `Tenmoku-<v>-<channel>-full.nupkg` and `.sig` go to the output directory; the nupkg names carry the channel, so the stable and preview copies of one version don't collide. There is one AppImage / Setup.exe per release, from the first pack (stable when the version is on it).
- The Linux path now takes the private key out of the environment before vpk runs (it used to do that only at signing time); the test's fake vpk checks it never sees the key.
- package-linux.sh and package-windows.ps1 accept `X.Y.Z-preview.N` (N without a leading zero, as check-release-version.sh), which a preview release needs before it gets to packing.
- scripts/test-release-workflow.sh was added to scope (self-respec): it grepped the ps1 for `$channel = "windows-$Arch-stable"` and `if ($signed -ne $public)`, both of which this card changes.
- Carried T1909 F3 fixed: the ps1 compares the signer's public half with `-cne`; test-velopack-pack.sh's fake-signer case refuses a case-only mismatch (when PowerShell is present).
- Carried T1909 F2 (vpk's portable zip) not fixed: publishing it in place of package-windows.ps1's zip needs an extraction smoke step and upload changes in .github/workflows/release.yml, which is outside this card's scope (CH9 owns the workflow).
- Nothing here ran a real vpk. The ps1 cases in test-velopack-pack.sh skip without pwsh, and pwsh is not installed on this Mac, so they were not run here; real Windows packing is proven by CH9's CI run.
