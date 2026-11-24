#!/usr/bin/env bash
# Fixture test for scripts/build-plugin-registry.sh: --dry-run with a tiny
# prebuilt module, so no cargo build. Prints one line per check and exits
# non-zero on the first failure.
#
#   scripts/test-build-plugin-registry.sh
set -euo pipefail
export LC_ALL=C

here=$(cd "$(dirname "$0")" && pwd)
build=$here/build-plugin-registry.sh
t=$(mktemp -d)
trap 'rm -rf "$t"' EXIT

fail() {
  echo "FAIL: $*"
  [ -f "$t/log" ] && sed 's/^/  | /' "$t/log"
  exit 1
}
ok() { echo "ok: $*"; }

sha256() {
  if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1
}

# A plugin directory $1 with the manifest's first lines $2.
make_plugin() {
  mkdir -p "$1"
  printf '%s\n' "$2" >"$1/extension.toml"
  cat >>"$1/extension.toml" <<'TOML'

[engine]
tenmoku = "1.3"

[contributes.plugin]
wasm = "demo.wasm"
capabilities = ["buffer.read", "process.spawn:cargo"]
TOML
  printf 'listed = true\n' >"$1/registry.toml"
  printf '\0asm\1\0\0\0' >"$1/demo.wasm"
  echo "MIT text" >"$1/LICENSE"
  echo "# Demo" >"$1/README.md"
  echo "not shipped" >"$1/notes.txt"
}

fresh_out() {
  rm -rf "$t/out"
  mkdir -p "$t/out"
  printf '{\n  "schema": 1,\n  "serial": 7,\n  "url_template": "u/{tag}/{file}",\n  "packs": {},\n  "languages": {}\n}\n' >"$t/out/index.json"
  echo stale >"$t/out/index.json.sig"
}

run() { "$build" --dry-run --plugins-dir "$t/plugins" --out "$t/out" >"$t/log" 2>&1; }

good='id = "acme.demo"
name = "Demo"
version = "1.2.0"
license = "MIT"'

# --- a good plugin
make_plugin "$t/plugins/demo" "$good"
fresh_out
run || fail "a good plugin was refused"
archive=$t/out/plugins/acme.demo/1.2.0/acme.demo-1.2.0.tar.gz
[ -f "$archive" ] || fail "no archive at $archive"
listing=$(tar -tzf "$archive" | sort | tr '\n' ' ')
[ "$listing" = "LICENSE README.md demo.wasm extension.toml " ] || fail "archive listing: $listing"
ok "the archive holds the manifest, module, LICENSE and README only"

python3 -I - "$t/out/index.json" "$archive" <<'PY' || fail "the entry's fields"
import hashlib, json, os, sys
index = json.load(open(sys.argv[1]))
assert index["serial"] == 7 and index["languages"] == {}, "the grammar index changed"
e = index["plugins"]["acme.demo"]
data = open(sys.argv[2], "rb").read()
assert list(e) == ["id", "name", "version", "api_minor", "license", "file",
                   "sha256", "size", "entries", "capabilities"], list(e)
assert (e["id"], e["name"], e["version"], e["license"]) == ("acme.demo", "Demo", "1.2.0", "MIT")
assert e["api_minor"] == 3 and e["entries"] == 4
assert e["file"] == "acme.demo-1.2.0.tar.gz"
assert e["capabilities"] == ["buffer.read", "process.spawn:cargo"]
assert e["size"] == len(data) and e["sha256"] == hashlib.sha256(data).hexdigest()
PY
ok "the entry has exactly the PluginEntry fields, and its size and hash match the archive"
[ "$(sha256 "$archive")" = "$(jq -r '.plugins["acme.demo"].sha256' "$t/out/index.json")" ] || fail "hash mismatch"
[ ! -e "$t/out/index.json.sig" ] || fail "a stale signature was left"
ok "the changed index's old signature is removed"

first=$(sha256 "$archive")
fresh_out
run || fail "rerun refused"
[ "$(sha256 "$archive")" = "$first" ] || fail "the archive isn't reproducible"
ok "the same inputs give the same archive"

# --- no plugins
for dir in "$t/none" "$t/plugins-empty"; do
  mkdir -p "$t/plugins-empty"
  fresh_out
  before=$(sha256 "$t/out/index.json")
  "$build" --dry-run --plugins-dir "$dir" --out "$t/out" >"$t/log" 2>&1 || fail "$dir refused"
  [ "$(sha256 "$t/out/index.json")" = "$before" ] || fail "$dir changed the index"
done
ok "a missing or empty plugins dir adds nothing and succeeds"

# --- refusals
for missing in id version license name; do
  rm -rf "$t/plugins"
  make_plugin "$t/plugins/demo" "$(printf '%s\n' "$good" | grep -v "^$missing =")"
  fresh_out
  if run; then fail "a manifest without $missing was accepted"; fi
  grep -q "no $missing" "$t/log" || fail "the refusal for $missing doesn't name it"
  grep -q '"plugins"' "$t/out/index.json" && fail "a refused plugin reached the index"
done
ok "a manifest missing id, name, version or license is refused"

rm -rf "$t/plugins"
make_plugin "$t/plugins/a" "$good"
make_plugin "$t/plugins/b" "$good"
fresh_out
if run; then fail "a duplicate id was accepted"; fi
ok "a duplicate id is refused"

rm -rf "$t/plugins"
make_plugin "$t/plugins/demo" "$good"
rm "$t/plugins/demo/demo.wasm"
fresh_out
if run; then fail "a missing module was accepted"; fi
ok "a missing module is refused"

fresh_out
rm -rf "$t/plugins"
if "$build" --dry-run --plugins-dir "$t/plugins" --out "$t/nowhere" >"$t/log" 2>&1; then fail "no index accepted"; fi
ok "no index.json is refused"
# --- a populated index, then no plugins: the entries go with the artifacts
rm -rf "$t/plugins"
make_plugin "$t/plugins/demo" "$good"
fresh_out
run || fail "setup run refused"
echo sig >"$t/out/index.json.sig"
rm -rf "$t/plugins"
mkdir -p "$t/plugins"
run || fail "an emptied plugins dir was refused"
jq -e 'has("plugins") | not' "$t/out/index.json" >/dev/null || fail "stale plugin entries stayed"
[ ! -e "$t/out/plugins" ] && [ ! -e "$t/out/index.json.sig" ] || fail "stale artifacts or signature stayed"
ok "emptying the plugins dir clears the entries, artifacts and signature"

# --- a failed run leaves the last output alone
rm -rf "$t/plugins"
make_plugin "$t/plugins/a" "$good"
fresh_out
run || fail "setup run refused"
cp "$t/out/index.json" "$t/index.before"
make_plugin "$t/plugins/b" 'id = "acme.other"
name = "Other"
version = "1.0.0"'
if run; then fail "an invalid second plugin was accepted"; fi
cmp -s "$t/index.before" "$t/out/index.json" || fail "a failed run changed the index"
[ -f "$archive" ] || fail "a failed run removed the archive"
[ ! -e "$t/out/.plugins-staging" ] || fail "staging left behind"
ok "a failed run leaves the previous index and artifacts"

# --- LICENSE and README are required
for gone in LICENSE README.md; do
  rm -rf "$t/plugins"
  make_plugin "$t/plugins/demo" "$good"
  rm "$t/plugins/demo/$gone"
  fresh_out
  if run; then fail "a plugin without $gone was accepted"; fi
done
ok "a plugin without a LICENSE or README is refused"

# --- the entry's limits
rm -rf "$t/plugins"
make_plugin "$t/plugins/demo" "$good"
caps=$(python3 -I -c 'print(", ".join(f"\"c{i}\"" for i in range(257)))')
sed -i.bak "s/^capabilities = .*/capabilities = [$caps]/" "$t/plugins/demo/extension.toml"
fresh_out
if run; then fail "257 capabilities were accepted"; fi
ok "more capabilities than the index takes are refused"

# --- a relative --out from another directory
rm -rf "$t/plugins"
make_plugin "$t/plugins/demo" "$good"
fresh_out
(cd "$t" && "$build" --dry-run --plugins-dir plugins --out out >"$t/log" 2>&1) || fail "relative paths refused"
jq -e '.plugins["acme.demo"]' "$t/out/index.json" >/dev/null || fail "relative --out missed the index"
ok "a relative --out works from any directory"
# --- the limits' edges: 256 capabilities pass, a minor beyond u32 doesn't
rm -rf "$t/plugins"
make_plugin "$t/plugins/demo" "$good"
caps=$(python3 -I -c 'print(", ".join(f"\"c{i}\"" for i in range(256)))')
sed -i.bak "s/^capabilities = .*/capabilities = [$caps]/" "$t/plugins/demo/extension.toml"
fresh_out
run || fail "256 capabilities were refused"
make_plugin "$t/plugins/demo" "$good"
sed -i.bak 's/^tenmoku = .*/tenmoku = "1.4294967296"/' "$t/plugins/demo/extension.toml"
fresh_out
if run; then fail "an API minor beyond u32 was accepted"; fi
grep -q 'u32' "$t/log" || fail "the u32 refusal doesn't say so"
ok "the capability and u32 limits hold at their edges"

# --- --sign runs the signer on the absolute index, from outside the repo
cat >"$t/signer" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$PWD" "$@" >"$SIGN_LOG"
cat >/dev/null
EOF
chmod +x "$t/signer"
rm -rf "$t/plugins"
make_plugin "$t/plugins/demo" "$good"
fresh_out
mkdir -p "$t/elsewhere"
(cd "$t/elsewhere" && echo key | SIGN_LOG="$t/signed" GRAMMAR_REGISTRY="$t/signer" \
  "$build" --dry-run --sign --plugins-dir ../plugins --out ../out >"$t/log" 2>&1) || fail "--sign failed"
grep -qx "$t/out/index.json" "$t/signed" || fail "the signer wasn't given the absolute index: $(cat "$t/signed")"
ok "--sign signs the requested index from another directory"
# --- an archive over 64 MiB (incompressible, so gzip can't shrink it)
rm -rf "$t/plugins"
make_plugin "$t/plugins/demo" "$good"
head -c $((65 * 1024 * 1024)) /dev/urandom >"$t/plugins/demo/demo.wasm"
fresh_out
if run; then fail "an archive over 64 MiB was accepted"; fi
grep -q 'size or entry limits' "$t/log" || fail "the size refusal doesn't say so"
grep -q '"plugins"' "$t/out/index.json" && fail "an oversized plugin reached the index"
ok "an archive over 64 MiB is refused"

# --- an archive over 10,000 entries
rm -rf "$t/plugins"
make_plugin "$t/plugins/demo" "$good"
python3 -I - "$t/plugins/demo" <<'PY'
import sys
for i in range(10_000):
    open(f"{sys.argv[1]}/LICENSE-{i:05}", "w").write("x\n")
PY
fresh_out
if run; then fail "an archive over 10,000 entries was accepted"; fi
grep -q 'size or entry limits' "$t/log" || fail "the entry refusal doesn't say so"
ok "an archive over 10,000 entries is refused"
# --- a failure while committing restores the previous output
rm -rf "$t/plugins"
make_plugin "$t/plugins/demo" "$good"
fresh_out
run || fail "setup run refused"
echo oldsig >"$t/out/index.json.sig"
snapshot() { (cd "$t/out" && find . -type f | sort | xargs cat | { sha256sum 2>/dev/null || shasum -a 256; }; find . | sort); }
before=$(snapshot)
make_plugin "$t/plugins/demo" "$(printf '%s\n' "$good" | sed 's/1.2.0/1.3.0/')"
for at in moving-old installing-artifacts writing-index; do
  if PLUGIN_REGISTRY_FAIL_AT=$at run; then fail "an injected failure at $at went unnoticed"; fi
  [ "$(snapshot)" = "$before" ] || fail "a failure at $at changed the output"
done
run || fail "the retry after failures was refused"
[ -f "$t/out/plugins/acme.demo/1.3.0/acme.demo-1.3.0.tar.gz" ] || fail "the retry didn't commit"
[ ! -e "$t/out/plugins/acme.demo/1.2.0" ] && [ ! -e "$t/out/.plugins-old" ] || fail "old artifacts or backup left"
ok "a failure at each commit step restores the old index, artifacts and signature"
# --- a run killed mid-commit (SIGKILL: no handler ran) is recovered by the next
rm -rf "$t/plugins"
make_plugin "$t/plugins/demo" "$good"
fresh_out
run || fail "setup run refused"
echo oldsig >"$t/out/index.json.sig"
before=$(snapshot)
# What the killed run leaves: the old artifacts and signature moved aside, a
# half-installed replacement, and the marker, index.json.tmp.
mv "$t/out/plugins" "$t/out/.plugins-old"
mv "$t/out/index.json.sig" "$t/out/index.json.sig.old"
mkdir -p "$t/out/plugins/plugin-half-done"
echo '{"half": true}' >"$t/out/index.json.tmp"
run || fail "a run after an interrupted one was refused"
[ "$(snapshot)" = "$before" ] || fail "the interrupted run wasn't rolled back"
ok "an interrupted commit is rolled back by the next run"

# Killed after index.json was replaced: only the moved-aside copies are left.
cp -R "$t/out/plugins" "$t/out/.plugins-old"
echo oldsig >"$t/out/index.json.sig.old"
run || fail "a run after a finished-but-uncleaned commit was refused"
[ "$(snapshot)" = "$before" ] || fail "leftovers of a finished commit stayed or the new output was lost"
ok "the leftovers of a finished commit are removed, the new output kept"

# A rollback that fails exits non-zero and keeps the backup.
mv "$t/out/plugins" "$t/out/.plugins-old"
mkdir -p "$t/out/plugins"
echo '{"half": true}' >"$t/out/index.json.tmp"
if PLUGIN_REGISTRY_FAIL_AT=rollback run; then fail "a failed rollback exited zero"; fi
[ -d "$t/out/.plugins-old/acme.demo/1.2.0" ] || fail "a failed rollback lost the backup"
grep -q 'rollback failed' "$t/log" || fail "a failed rollback didn't say so"
run || fail "the retry after a failed rollback was refused"
[ "$(snapshot)" = "$before" ] || fail "the retry didn't recover"
ok "a failed rollback exits non-zero, keeps the backup, and the next run recovers"
# --- a first publish (no plugins directory before): failure and recovery leave none
rm -rf "$t/plugins"
make_plugin "$t/plugins/demo" "$good"
for at in installing-artifacts writing-index; do
  fresh_out
  cp "$t/out/index.json" "$t/index.first"
  if PLUGIN_REGISTRY_FAIL_AT=$at run; then fail "an injected failure at $at went unnoticed"; fi
  [ ! -e "$t/out/plugins" ] || fail "a first publish failed at $at and left artifacts"
  cmp -s "$t/index.first" "$t/out/index.json" || fail "a first publish failed at $at and changed the index"
  [ ! -e "$t/out/.plugins-none" ] || fail "the first-publish marker was left at $at"
done
# killed after installing: the marker, the half-installed artifacts and the tmp
fresh_out
cp "$t/out/index.json" "$t/index.first"
touch "$t/out/.plugins-none"
mkdir -p "$t/out/plugins/acme.demo/1.2.0"
echo junk >"$t/out/plugins/acme.demo/1.2.0/half-installed"
echo '{"half": true}' >"$t/out/index.json.tmp"
run || fail "a run after an interrupted first publish was refused"
[ ! -e "$t/out/plugins/acme.demo/1.2.0/half-installed" ] || fail "the half-installed artifacts stayed"
[ -f "$t/out/plugins/acme.demo/1.2.0/acme.demo-1.2.0.tar.gz" ] && [ ! -e "$t/out/.plugins-none" ] || fail "the rerun didn't build the plugin cleanly"
ok "a failed or interrupted first publish is rolled back to no artifacts"

# --- ids and versions that would join to the same name are both built
rm -rf "$t/plugins"
make_plugin "$t/plugins/a" 'id = "foo-1.0.0"
name = "A"
version = "2.0.0"
license = "MIT"'
make_plugin "$t/plugins/b" 'id = "foo"
name = "B"
version = "1.0.0-2.0.0"
license = "MIT"'
fresh_out
run || fail "two distinct id/version pairs that join alike were refused: $(cat "$t/log")"
[ -f "$t/out/plugins/foo-1.0.0/2.0.0/foo-1.0.0-2.0.0.tar.gz" ] && [ -f "$t/out/plugins/foo/1.0.0-2.0.0/foo-1.0.0-2.0.0.tar.gz" ] || fail "an artifact is missing"
[ "$(jq '.plugins | length' "$t/out/index.json")" = 2 ] || fail "both entries aren't in the index"
ok "id foo-1.0.0 @ 2.0.0 and id foo @ 1.0.0-2.0.0 are both built"
echo "all passed"
