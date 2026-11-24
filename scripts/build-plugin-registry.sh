#!/usr/bin/env bash
# Adds the first-party plugins to the registry that scripts/build-grammar-registry.sh
# wrote: each plugin's archive and its entry in <out>/index.json
# (fe_registry::plugins::PluginEntry, docs/release/plugin-registry.md).
#
#   scripts/build-grammar-registry.sh
#   scripts/build-plugin-registry.sh --plugins-dir extensions/plugins --out target/registry
#   scripts/build-plugin-registry.sh ... --sign < private-key     # re-signs the index
#
# For each directory under --plugins-dir holding a registry.toml (the listing
# marker) and an extension.toml, it:
#
# 1. refuses the plugin unless its manifest has id, name, version and license,
#    an [engine] tenmoku = "1.<minor>" and a [contributes.plugin] wasm;
# 2. builds it (cargo build --release --target wasm32-wasip2 on the
#    directory's Cargo.toml; the module is target/wasm32-wasip2/release/<built>,
#    <built> being registry.toml's `built`, default the wasm's file name);
# 3. writes <out>/plugins/<id>/<version>/<id>-<version>.tar.gz: the
#    extension.toml, the .wasm, and LICENSE* and README* files, byte-identical
#    for the same inputs (sorted, zero mtime and owner);
# 4. merges the entries into <out>/index.json, replacing every plugin there,
#    and with --sign re-signs it (`grammar-registry sign`, the private key on
#    stdin as for the grammars). Without --sign a stale index.json.sig is
#    removed, since it no longer matches.
#
# --dry-run builds nothing: the module is the one already at the manifest's
# `wasm` path in the plugin directory (a fixture, for the tests). A missing or
# empty --plugins-dir adds nothing and succeeds. The index's url_template and
# serial are the grammar script's; GRAMMAR_REGISTRY is the grammar-registry
# command for --sign, REGISTRY_TOML the registry.toml for it.
set -euo pipefail
export LC_ALL=C

die() {
  echo "build-plugin-registry: $*" >&2
  exit 1
}

usage="usage: $0 --plugins-dir <dir> --out <dir> [--dry-run] [--sign]"
plugins_dir=
out=
dry_run=
sign=
while [ $# -gt 0 ]; do
  case $1 in
  --plugins-dir | --out)
    [ $# -ge 2 ] && [ -n "$2" ] || die "$1 needs a value ($usage)"
    case $1 in
    --plugins-dir) plugins_dir=$2 ;;
    --out) out=$2 ;;
    esac
    shift 2
    ;;
  --dry-run)
    dry_run=1
    shift
    ;;
  --sign)
    sign=1
    shift
    ;;
  *) die "unknown argument $1 ($usage)" ;;
  esac
done
[ -n "$plugins_dir" ] && [ -n "$out" ] || die "--plugins-dir and --out are required ($usage)"
mkdir -p "$out" 2>/dev/null || true
out=$(cd "$out" 2>/dev/null && pwd) || die "no $out"
[ -f "$out/index.json" ] || die "no $out/index.json: build it with scripts/build-grammar-registry.sh"
command -v python3 >/dev/null || die "python3 is required"

root=$(cd "$(dirname "$0")/.." && pwd)
registry_toml=${REGISTRY_TOML:-$root/extensions/grammars/registry.toml}

DRY_RUN=$dry_run python3 -I - "$plugins_dir" "$out" <<'PY'
import gzip, hashlib, io, json, os, re, shutil, subprocess, sys, tarfile, tomllib

plugins_dir, out = sys.argv[1], sys.argv[2]
dry = bool(os.environ.get("DRY_RUN"))


def die(message):
    shutil.rmtree(os.path.join(out, ".plugins-staging"), ignore_errors=True)
    print(f"build-plugin-registry: {message}", file=sys.stderr)
    sys.exit(1)


def plain(path):
    parts = path.split("/")
    return bool(path) and "\\" not in path and not path.startswith("/") and all(
        p not in ("", ".", "..") for p in parts
    )


index_path = os.path.join(out, "index.json")
plugins = os.path.join(out, "plugins")
backup = os.path.join(out, ".plugins-old")
sig = index_path + ".sig"
sig_backup = index_path + ".sig.old"
none_marker = os.path.join(out, ".plugins-none")  # no plugins directory before the commit
temp = index_path + ".tmp"


def step(name):
    if os.environ.get("PLUGIN_REGISTRY_FAIL_AT") == name:
        raise OSError(f"injected failure at {name}")


def rmtree(p):
    if os.path.isdir(p):
        shutil.rmtree(p)
    elif os.path.exists(p):
        os.remove(p)


def restore(old_index):
    """Puts the old artifacts, signature and index back. Fails loudly, and
    keeps the moved-aside copies, if any step fails."""
    try:
        step("rollback")
        if os.path.exists(backup):
            rmtree(plugins)
            os.rename(backup, plugins)
        elif os.path.exists(none_marker):
            rmtree(plugins)
        if os.path.exists(sig_backup):
            os.replace(sig_backup, sig)
        if old_index is not None:
            with open(temp + ".restore", "wb") as f:
                f.write(old_index)
            os.replace(temp + ".restore", index_path)
        rmtree(temp)
        rmtree(none_marker)
    except BaseException as error:
        print(f"build-plugin-registry: the rollback failed ({error}); the previous output is "
              f"kept in {backup} and {sig_backup}: put them back by hand", file=sys.stderr)
        sys.exit(2)


# A run killed partway (no handler ran) leaves its marker, index.json.tmp,
# which is only replaced into index.json as the commit's last step: with it
# the old output is restored; without it the commit finished and only the
# moved-aside copies are left to remove. Before anything else.
if os.path.exists(temp):
    print("build-plugin-registry: restoring the output of an interrupted run", file=sys.stderr)
    restore(None)
else:
    for leftover in (backup, sig_backup, none_marker):
        rmtree(leftover)
for leftover in (os.path.join(out, ".plugins-staging"), temp + ".restore"):
    rmtree(leftover)

# Everything is built in a staging directory and put in place only once
# every plugin has passed, so a failure leaves the last output as it was.
stage = os.path.join(out, ".plugins-staging")
shutil.rmtree(stage, ignore_errors=True)
dirs = []
if os.path.isdir(plugins_dir):
    dirs = sorted(
        d for d in os.listdir(plugins_dir)
        if os.path.isfile(os.path.join(plugins_dir, d, "registry.toml"))
    )
entries = {}
for name in dirs:
    d = os.path.join(plugins_dir, name)
    where = f"plugin {name}"
    try:
        with open(os.path.join(d, "extension.toml"), "rb") as f:
            manifest = tomllib.load(f)
        with open(os.path.join(d, "registry.toml"), "rb") as f:
            listing = tomllib.load(f)
    except (OSError, tomllib.TOMLDecodeError) as error:
        die(f"{where}: {error}")
    for key in ("id", "name", "version", "license"):
        value = manifest.get(key)
        if not isinstance(value, str) or not value.strip():
            die(f"{where}: its extension.toml has no {key}")
    pid, version = manifest["id"], manifest["version"]
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", pid):
        die(f"{where}: the id {pid!r} isn't plain")
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9.+_-]*", version):
        die(f"{where}: the version {version!r} isn't plain")
    if pid in entries:
        die(f"{where}: the id {pid} is listed twice")
    engine = manifest.get("engine", {}).get("tenmoku", "")
    m = re.fullmatch(r"1\.(\d+)", engine) if isinstance(engine, str) else None
    if not m:
        die(f'{where}: [engine] tenmoku must be "1.<minor>", not {engine!r}')
    plugin = manifest.get("contributes", {}).get("plugin")
    if not isinstance(plugin, dict):
        die(f"{where}: no [contributes.plugin]")
    wasm = plugin.get("wasm")
    if not isinstance(wasm, str) or not plain(wasm):
        die(f"{where}: [contributes.plugin] wasm isn't a plain relative path")
    caps = plugin.get("capabilities", [])
    if not isinstance(caps, list) or not all(isinstance(c, str) for c in caps):
        die(f"{where}: capabilities isn't a list of strings")

    if dry:
        module = os.path.join(d, wasm)
    else:
        manifest_path = os.path.join(d, "Cargo.toml")
        subprocess.run(
            ["cargo", "build", "--release", "--locked", "--target", "wasm32-wasip2",
             "--manifest-path", manifest_path],
            check=True,
        )
        meta = subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--no-deps",
             "--manifest-path", manifest_path],
            check=True, capture_output=True, text=True,
        )
        built = listing.get("built", os.path.basename(wasm))
        module = os.path.join(json.loads(meta.stdout)["target_directory"],
                              "wasm32-wasip2", "release", built)
    if not os.path.isfile(module):
        die(f"{where}: no module at {module}")

    if len(caps) > 256:
        die(f"{where}: {len(caps)} capabilities; the index takes at most 256")
    if int(m.group(1)) > 0xFFFFFFFF:
        die(f"{where}: the API minor {m.group(1)} doesn't fit u32")
    members = [("extension.toml", os.path.join(d, "extension.toml")), (wasm, module)]
    found = set()
    for f in sorted(os.listdir(d)):
        for kind in ("LICENSE", "README"):
            if f.startswith(kind) and os.path.isfile(os.path.join(d, f)):
                members.append((f, os.path.join(d, f)))
                found.add(kind)
    for kind in ("LICENSE", "README"):
        if kind not in found:
            die(f"{where}: no {kind}* file to archive")
    members.sort()
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w", format=tarfile.PAX_FORMAT) as tar:
        for arc, path in members:
            with open(path, "rb") as f:
                data = f.read()
            info = tarfile.TarInfo(arc)
            info.size, info.mode, info.mtime = len(data), 0o644, 0
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            tar.addfile(info, io.BytesIO(data))
    archive = gzip.compress(raw.getvalue(), mtime=0)
    file = f"{pid}-{version}.tar.gz"
    if len(archive) > 64 * 1024 * 1024 or len(members) > 10_000:
        die(f"{where}: the archive is over the index's size or entry limits")
    target = os.path.join(stage, pid, version)
    os.makedirs(target)  # id and version are separate components: no ambiguity
    with open(os.path.join(target, file), "wb") as f:
        f.write(archive)
    entries[pid] = {
        "id": pid,
        "name": manifest["name"],
        "version": version,
        "api_minor": int(m.group(1)),
        "license": manifest["license"],
        "file": file,
        "sha256": hashlib.sha256(archive).hexdigest(),
        "size": len(archive),
        "entries": len(members),
        "capabilities": caps,
    }
    print(f"plugin {pid} {version}: {file} ({len(archive)} bytes)")

with open(index_path, "rb") as f:
    old_index = f.read()
index = json.loads(old_index)
before = json.dumps(index.get("plugins", {}), sort_keys=True)
if entries:
    index["plugins"] = dict(sorted(entries.items()))
else:
    index.pop("plugins", None)
changed = json.dumps(index.get("plugins", {}), sort_keys=True) != before
# The commit: the new index is written beside the old (always: it is the
# marker of a commit in progress), the old artifacts and signature are moved
# aside, the new ones put in place, and index.json replaced, which removes the
# marker. If a step fails everything moved is put back, so the output is the
# old one or the new one; a rollback that fails keeps the copies and exits
# non-zero. PLUGIN_REGISTRY_FAIL_AT=<step> makes a step fail, for the tests.
try:
    with open(temp, "w") as f:
        json.dump(index, f, indent=2)
        f.write("\n")
    step("moving-old")
    if os.path.exists(plugins):
        os.rename(plugins, backup)
    else:
        open(none_marker, "w").close()
    if changed and os.path.exists(sig):
        os.rename(sig, sig_backup)
    step("installing-artifacts")
    if entries:
        os.rename(stage, plugins)
    step("writing-index")
    os.replace(temp, index_path)
except BaseException as error:
    restore(old_index)
    die(f"the output was left as it was: {error}")
rmtree(backup)
rmtree(sig_backup)
rmtree(none_marker)
rmtree(stage)
PY

if [ -n "$sign" ]; then
  read -r -a registry <<<"${GRAMMAR_REGISTRY:-cargo run --quiet --locked -p fe-registry --bin grammar-registry --}"
  (cd "$root" && "${registry[@]}" sign --index "$out/index.json" --registry-toml "$registry_toml")
fi
