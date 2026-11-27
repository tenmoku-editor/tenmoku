#!/usr/bin/env bash
# Fixture test for scripts/publish-grammar-registry.sh: a registry of two
# languages and a plugin, built by grammar-registry and
# scripts/build-plugin-registry.sh --dry-run (python3 3.11 or later) and signed
# with a throwaway key, published against a fake gh on PATH that keeps its
# releases in a directory. No network.
#
#   scripts/test-publish-grammar-registry.sh
#   GRAMMAR_REGISTRY=target/debug/grammar-registry scripts/test-publish-grammar-registry.sh
#
# Prints one line per check and exits non-zero on the first failure.
# crates/fe-registry/tests/publish_script.rs runs it with the built binary.
set -euo pipefail
export LC_ALL=C

here=$(cd "$(dirname "$0")" && pwd)
publish=$here/publish-grammar-registry.sh
export GRAMMAR_REGISTRY=${GRAMMAR_REGISTRY:-cargo run --quiet --locked --manifest-path $here/../Cargo.toml -p fe-registry --bin grammar-registry --}
read -r -a registry <<<"$GRAMMAR_REGISTRY"
t=$(mktemp -d)
trap 'rm -rf "$t"' EXIT

fail() {
  echo "FAIL: $*"
  [ -f "$t/out" ] && sed 's/^/  | /' "$t/out"
  exit 1
}
ok() { echo "ok: $*"; }

repo=test-owner/registry
grammars=$t/grammars
wasm=$t/wasm
dist=$t/dist
plugins=$t/plugins
mkdir -p "$grammars" "$wasm" "$plugins/demo" "$t/bin" "$t/state"

# The fake gh: `release view|create|edit|upload|download <tag> ...`. Each call is
# a line of the log, `<verb> <tag>` and the base names of the files it was
# given. A release is the directory $GH_STATE/<tag>, holding its assets; it's
# a draft while $GH_STATE/<tag>.draft exists. view prints what the script's
# --jq asks for: `draft\t<bool>`, then each asset's `<name>\t<size>`.
cat >"$t/bin/gh" <<'EOF'
#!/usr/bin/env bash
set -eu
[ "$1" = release ] || { echo "fake gh: no $1" >&2; exit 2; }
verb=$2 tag=$3
shift 3
files=() json= pattern= into= notes=
while [ $# -gt 0 ]; do
  case $1 in
  --repo)
    [ "$2" = "$GH_REPO_EXPECTED" ] || { echo "fake gh: --repo $2" >&2; exit 2; }
    shift 2 ;;
  --json) json=$2; shift 2 ;;
  --pattern) pattern=$2; shift 2 ;;
  --dir) into=$2; shift 2 ;;
  --notes) notes=$2; shift 2 ;;
  --target | --title | --jq) shift 2 ;;
  --latest=false | --clobber) shift ;;
  -*) echo "fake gh: unknown flag $1" >&2; exit 2 ;;
  *) files+=("$1"); shift ;;
  esac
done
line="$verb $tag"
for file in ${files[@]+"${files[@]}"}; do line+=" $(basename "$file")"; done
echo "$line" >>"$GH_LOG"
release=$GH_STATE/$tag
add() {
  for file in ${files[@]+"${files[@]}"}; do cp "$file" "$release/"; done
}
case $verb in
view)
  [ -d "$release" ] || { echo "release not found" >&2; exit 1; }
  if [ "$json" = body ]; then cat "$release.notes" 2>/dev/null || true; exit 0; fi
  [ "$json" = isDraft,assets ] || { echo "fake gh: --json $json" >&2; exit 2; }
  if [ -e "$release.draft" ]; then printf 'draft\ttrue\n'; else printf 'draft\tfalse\n'; fi
  for asset in "$release"/*; do
    [ -f "$asset" ] && printf '%s\t%s\n' "$(basename "$asset")" "$(wc -c <"$asset" | tr -d ' ')"
  done
  true ;;
create)
  [ ! -d "$release" ] || { echo "a release with the same tag name already exists: $tag" >&2; exit 1; }
  mkdir "$release"
  printf '%s\n' "$notes" >"$release.notes"
  add ;;
edit)
  [ -d "$release" ] || { echo "release not found" >&2; exit 1; }
  printf '%s\n' "$notes" >"$release.notes" ;;
upload)
  [ -d "$release" ] || { echo "release not found" >&2; exit 1; }
  add ;;
download)
  [ -f "$release/$pattern" ] || { echo "no asset $pattern" >&2; exit 1; }
  cp "$release/$pattern" "$into/" ;;
*) echo "fake gh: no release $verb" >&2; exit 2 ;;
esac
EOF
chmod +x "$t/bin/gh"

# The fixture registry: kotlin from a crate, ruby from git with a NOTICE.
"${registry[@]}" generate-key --out-private "$t/key" >"$t/public"
cat >"$grammars/registry.toml" <<EOF
schema = 1
url_template = "https://github.com/$repo/releases/download/{tag}/{file}"
index_url = "https://github.com/$repo/releases/download/index/index.json"
public_key = "$(cat "$t/public")"

[packs.jvm]
name = "JVM"
description = "Kotlin and friends"

[packs.web]
name = "Web"
description = "Servers"
EOF
language() {
  local language=$1 name=$2 pack=$3 source=$4 dir=$grammars/$1
  mkdir -p "$dir/test"
  cat >"$dir/extension.toml" <<EOF
id = "tenmoku.grammar_$language"
name = "$name syntax"
version = "1.2.3"
license = "MIT"

[engine]
tenmoku = "1.0"

[[contributes.grammars]]
language = "$language"
wasm = "tree-sitter-$language.wasm"
extensions = ["$language"]
highlights = "highlights.scm"
injections = "injections.scm"
packs = ["$pack"]
EOF
  echo "; $language" >"$dir/highlights.scm"
  echo "; none" >"$dir/injections.scm"
  echo "$language's LICENSE" >"$dir/LICENSE"
  echo "sample" >"$dir/test/sample.x"
  printf '%s\n' "$source" >"$dir/grammar-source.toml"
  printf '\0asm %s' "$language" >"$wasm/tree-sitter-$language.wasm"
  echo 14 >"$wasm/tree-sitter-$language.wasm.abi"
}
language kotlin Kotlin jvm $'crate = "tree-sitter-kotlin-sg"\nversion = "0.4.1"\ngrammar = "kotlin"'
language ruby Ruby web $'git = "https://github.com/tree-sitter/tree-sitter-ruby"\nrev = "0123456789abcdef0123456789abcdef01234567"'
echo "ruby's NOTICE" >"$grammars/ruby/NOTICE"

# The fixture plugin, its module prebuilt.
cat >"$plugins/demo/extension.toml" <<'EOF'
id = "acme.demo"
name = "Demo"
version = "0.2.0"
license = "MIT"

[engine]
tenmoku = "1.1"

[contributes.plugin]
wasm = "demo.wasm"
capabilities = ["commands"]
EOF
echo 'listed = true' >"$plugins/demo/registry.toml"
printf '\0asm demo' >"$plugins/demo/demo.wasm"
echo "demo's LICENSE" >"$plugins/demo/LICENSE"
echo "# Demo" >"$plugins/demo/README.md"
plugin_file=acme.demo-0.2.0.tar.gz
plugin_path=plugins/acme.demo/0.2.0/$plugin_file

# A signed registry in $1 at the serial $2, as CI builds it.
build_dist() {
  "${registry[@]}" generate --grammars "$grammars" --wasm "$wasm" --out "$1" --serial "$2" >/dev/null
  mkdir -p "$1/packs"
  for name in Tenmoku-language-pack-jvm Tenmoku-language-pack-web Tenmoku-language-pack; do
    echo "$name" >"$1/packs/$name.tar.gz"
    echo "stub  $name.tar.gz" >"$1/packs/$name.tar.gz.sha256"
  done
  "${registry[@]}" finalize --index "$1/index.json" --tarballs "$1/packs" >/dev/null
  "$here/build-plugin-registry.sh" --dry-run --plugins-dir "$plugins" --out "$1" >/dev/null
  "${registry[@]}" sign --index "$1/index.json" --registry-toml "$grammars/registry.toml" \
    <"$t/key" >/dev/null
}
build_dist "$dist" 7
# A plugin's tag: <id>-<the first 16 hex digits of its archive's SHA-256>.
tag_of_archive() {
  local sum
  sum=$(if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi)
  printf 'acme.demo-%s\n' "${sum:0:16}"
}
plugin_tag=$(tag_of_archive "$dist/$plugin_path")
cp "$dist/index.json" "$t/index.json.good"
cp "$dist/index.json.sig" "$t/index.json.sig.good"

tag_of() { "${registry[@]}" languages --index "$dist/index.json" | awk -F'\t' -v l="$1" '$1 == l { print $2 }'; }
files_of() { "${registry[@]}" languages --index "$dist/index.json" "$1" | cut -f1 | tr '\n' ' ' | sed 's/ $//'; }
kotlin=$(tag_of kotlin)
ruby=$(tag_of ruby)
[ -n "$kotlin" ] && [ -n "$ruby" ] || fail "the fixture index has no tags"

# Runs the publish script with the fake gh and a fresh log; true if it succeeds.
run() {
  : >"$t/log"
  PATH="$t/bin:$PATH" GH_LOG=$t/log GH_STATE=$t/state GH_REPO_EXPECTED=$repo \
    GRAMMARS_DIR=$grammars TENMOKU_COMMIT=abc123 \
    "$publish" --dist "$dist" --repo "$repo" --registry-toml "$grammars/registry.toml" "$@" \
    >"$t/out" 2>&1
}
log() { cat "$t/log"; }
reset() {
  rm -rf "$t/state"
  mkdir -p "$t/state"
  cp "$t/index.json.good" "$dist/index.json"
  cp "$t/index.json.sig.good" "$dist/index.json.sig"
}
# Makes the fake's release $1 from `<name>\t<size>` lines on stdin, each
# asset that many bytes.
fake_release() {
  mkdir -p "$t/state/$1"
  while IFS=$'\t' read -r name bytes; do
    head -c "$bytes" /dev/zero >"$t/state/$1/$name"
  done
}

# 1. Nothing published yet: each language is created with exactly its files,
# then the index release, its packs, the sig and the index, in that order.
run || fail "the first publish failed"
expected="view index
view $kotlin
view $ruby
view $plugin_tag
create $kotlin $(files_of kotlin)
create $ruby $(files_of ruby)
create $plugin_tag $plugin_file
create index
upload index Tenmoku-language-pack-jvm.tar.gz Tenmoku-language-pack-jvm.tar.gz.sha256 Tenmoku-language-pack-web.tar.gz Tenmoku-language-pack-web.tar.gz.sha256 Tenmoku-language-pack.tar.gz Tenmoku-language-pack.tar.gz.sha256
upload index index.json.sig
upload index index.json"
[ "$(log)" = "$expected" ] || fail "the first publish's gh calls:
$(log)
expected:
$expected"
ok "a new tag is created with exactly its files, and the index uploads come last, the sig before the index"
[ "$(files_of ruby)" = "LICENSE NOTICE extension.toml highlights.scm injections.scm tree-sitter-ruby.wasm" ] ||
  fail "ruby's files: $(files_of ruby)"
held=$(cd "$t/state/$kotlin" && printf '%s ' *)
[ "$held" = "$(files_of kotlin) " ] || fail "kotlin's release holds $held"
cmp -s "$t/state/$plugin_tag/$plugin_file" "$dist/$plugin_path" || fail "the plugin's release doesn't hold its archive"
ok "the fake's releases hold the entries' files"

# 2. Published again, as a retry does: both releases match their entries and
# are skipped, and the published index at the same serial is replaced.
run || fail "the second publish failed"
grep -q "^create" "$t/log" && fail "the second publish created a release:
$(log)"
grep -q "^download index" "$t/log" || fail "the second publish didn't read the published index"
grep -q "kotlin: $kotlin is published" "$t/out" || fail "kotlin wasn't reported as published"
grep -q "^download $plugin_tag" "$t/log" || fail "the published plugin archive wasn't checked"
grep -q "acme.demo 0.2.0: $plugin_tag is published" "$t/out" || fail "the plugin wasn't reported as published"
[ "$(tail -n 2 "$t/log")" = "upload index index.json.sig
upload index index.json" ] || fail "the second publish didn't end with the sig, then the index"
ok "an existing matching release is skipped"

# 3. A release under the tag with other files fails naming the language,
# before anything is created or uploaded.
reset
printf 'tree-sitter-kotlin.wasm\t1\n' | fake_release "$kotlin"
run && fail "a mismatching release didn't fail"
grep -q "kotlin: the release $kotlin exists with other files" "$t/out" ||
  fail "the failure doesn't name kotlin"
grep -qv "^view" "$t/log" && fail "a mismatching release still published:
$(log)"
[ "$(cd "$t/state/$kotlin" && echo *)" = tree-sitter-kotlin.wasm ] || fail "kotlin's release was changed"
ok "a mismatching release fails naming the language, and nothing is published"
# A size alone differing is a mismatch too.
reset
"${registry[@]}" languages --index "$dist/index.json" kotlin |
  awk -F'\t' 'NR == 1 { $2 = $2 + 1 } { print $1 "\t" $2 }' | fake_release "$kotlin"
run && fail "a release with one size off didn't fail"
grep -q "publish-grammar-registry: kotlin: the release" "$t/out" || fail "the size mismatch doesn't name kotlin"
ok "a release whose sizes differ fails too"

# 4. A bad signature publishes nothing, nor runs gh at all.
reset
sed 's/"serial": 7/"serial": 8/' "$t/index.json.good" >"$dist/index.json"
run && fail "a tampered index was published"
[ -s "$t/log" ] && fail "a tampered index ran gh:
$(log)"
grep -q "doesn't verify: publishing nothing" "$t/out" || fail "no verify failure reported"
reset
"${registry[@]}" generate-key --out-private "$t/other-key" >"$t/other-public"
sed "s|^public_key = .*|public_key = \"$(cat "$t/other-public")\"|" "$grammars/registry.toml" >"$t/other.toml"
"${registry[@]}" sign --index "$dist/index.json" --registry-toml "$t/other.toml" <"$t/other-key" >/dev/null
run && fail "an index signed by another key was published"
[ -s "$t/log" ] && fail "an index signed by another key ran gh"
reset
rm "$dist/index.json.sig"
run && fail "an unsigned index was published"
[ -s "$t/log" ] && fail "an unsigned index ran gh"
ok "a bad or missing signature publishes nothing"

# 5. A file in dist that isn't the one the index names, or an index for
# another repository, publishes nothing either.
reset
echo "changed" >>"$dist/languages/ruby/highlights.scm"
run && fail "a changed artifact file was published"
grep -qv "^view" "$t/log" && fail "a changed artifact file still published"
grep -q "highlights.scm isn't the file the index names" "$t/out" || fail "the changed file isn't named"
echo "; ruby" >"$dist/languages/ruby/highlights.scm"
PATH="$t/bin:$PATH" GH_LOG=$t/log GH_STATE=$t/state GH_REPO_EXPECTED=other/registry \
  GRAMMARS_DIR=$grammars TENMOKU_COMMIT=abc123 \
  "$publish" --dist "$dist" --repo other/registry --registry-toml "$grammars/registry.toml" \
  >"$t/out" 2>&1 && fail "an index for another repository was published"
grep -q "it's for another repository" "$t/out" || fail "the other repository isn't reported"
ok "a changed file or another repository's index publishes nothing"

# 6. --dry-run prints the gh commands, runs none, and publishes nothing.
reset
run --dry-run || fail "the dry run failed"
[ -s "$t/log" ] && fail "the dry run ran gh:
$(log)"
[ -z "$(ls "$t/state")" ] || fail "the dry run published"
commands=$(grep '^gh ' "$t/out" | awk '{ print $2, $3, $4 }')
expected="release view index
release view $kotlin
release view $ruby
release view $plugin_tag
release create $kotlin
release create $ruby
release create $plugin_tag
release create index
release upload index
release upload index
release upload index"
[ "$commands" = "$expected" ] || fail "the dry run printed:
$commands"
grep -q "^gh release create $kotlin .*--title Kotlin\\\\ syntax\\\\ 1.2.3" "$t/out" || fail "the dry run's title"
tail -n 3 "$t/out" | head -n 1 | grep -q 'index.json.sig$' || fail "the dry run's sig upload isn't before the index"
tail -n 2 "$t/out" | head -n 1 | grep -q 'index.json$' || fail "the dry run's last upload isn't the index"
grep -q "^gh release create $plugin_tag --repo $repo .*--latest=false --title Demo\\\\ plugin\\\\ 0.2.0 .*/$plugin_path\$" "$t/out" ||
  fail "the dry run's plugin upload"
ok "--dry-run prints every gh command and runs none"

# The release notes name the source pin and the commit.
grep -q "Grammar source: the crate tree-sitter-kotlin-sg 0.4.1" "$t/out" || fail "kotlin's notes"
grep -q "tree-sitter-ruby at 0123456789abcdef0123456789abcdef01234567" "$t/out" || fail "ruby's notes"
grep -q "abc123" "$t/out" || fail "the notes don't name the commit"
ok "the release notes give the source pin and the commit"

# 7. A draft release, which gh leaves when a create is cut off between its
# uploads and publishing, isn't published: matching assets or not, the run
# fails naming it before anything is created or uploaded.
reset
mkdir -p "$t/state/$kotlin"
for file in $(files_of kotlin); do cp "$dist/languages/kotlin/$file" "$t/state/$kotlin/"; done
touch "$t/state/$kotlin.draft"
run && fail "a draft language release was taken as published"
grep -q "kotlin: the release $kotlin is a draft" "$t/out" || fail "the draft isn't named"
grep -qv "^view" "$t/log" && fail "a draft language release still published:
$(log)"
reset
mkdir -p "$t/state/index"
touch "$t/state/index.draft"
run && fail "a draft index release was uploaded to"
grep -q "the index: the release index is a draft" "$t/out" || fail "the draft index isn't named"
[ "$(log)" = "view index" ] || fail "a draft index release still published:
$(log)"
ok "a draft language or index release fails the run, and nothing is published"

# 8. A rerun of an older run never replaces a newer index: B (serial 8)
# publishes, then A (serial 7) is refused before anything is created or
# uploaded, and B stays current. B again, a retry, goes ahead.
reset
build_dist "$t/dist-b" 8
dist_a=$dist
dist=$t/dist-b
run || fail "B's publish failed"
dist=$dist_a
run && fail "A, older than the published B, was published"
grep -q "is serial 8, newer than this one's 7" "$t/out" || fail "A's refusal doesn't give the serials"
grep -qvE "^(view|download) " "$t/log" && fail "A still published:
$(log)"
cmp -s "$t/state/index/index.json" "$t/dist-b/index.json" || fail "B's index was replaced"
cmp -s "$t/state/index/index.json.sig" "$t/dist-b/index.json.sig" || fail "B's signature was replaced"
dist=$t/dist-b
run || fail "B's retry failed"
dist=$dist_a
ok "an older run's index never replaces a newer one, and a retry goes ahead"

# 9. The newest serial survives a failed upload. B (serial 8) is published;
# a retry of B then fails after gh --clobber has deleted index.json, as the
# real one does before its replacement is up. A (serial 7) is still refused,
# by the watermark in the release's notes, and B's retry recovers.
reset
dist=$t/dist-b
run || fail "B's publish failed"
rm "$t/state/index/index.json"
dist=$dist_a
run && fail "A was published over B's index release after its index.json was deleted"
grep -q "watermark is serial 8, newer than this one's 7" "$t/out" || fail "A's refusal doesn't come from the watermark"
grep -qvE "^view " "$t/log" && fail "A still published:
$(log)"
[ ! -e "$t/state/index/index.json" ] || fail "A uploaded an index.json"
dist=$t/dist-b
run || fail "B's recovery after the deleted index.json failed"
cmp -s "$t/state/index/index.json" "$t/dist-b/index.json" || fail "B's index wasn't restored"
dist=$dist_a
ok "a deleted index.json doesn't open the index release to an older run, and the same index recovers it"
# A published release with neither an index.json nor a watermark can't be judged.
reset
mkdir -p "$t/state/index"
run && fail "a published index release with no record of its serial was published to"
grep -q "whether this index is the newest can't be established" "$t/out" || fail "the unjudgeable release isn't explained"
grep -qvE "^view " "$t/log" && fail "an unjudgeable release still published"
ok "an index release with no index.json and no watermark is refused"

# 10. The serial is only a commit time: another commit's index with the same
# serial isn't a retry. B publishes, then C (serial 8, a grammar changed) is
# refused, from the index.json and, with that deleted, from the watermark.
reset
echo "; changed" >>"$grammars/ruby/highlights.scm"
build_dist "$t/dist-c" 8
sed -i.bak '$d' "$grammars/ruby/highlights.scm" && rm "$grammars/ruby/highlights.scm.bak"
cmp -s "$t/dist-b/index.json" "$t/dist-c/index.json" && fail "the fixture's C index is B's"
dist=$t/dist-b
run || fail "B's publish failed"
dist=$t/dist-c
run && fail "C, the same serial as B but other bytes, was published"
grep -q "another commit's index with the same commit time" "$t/out" || fail "C's refusal isn't explained"
grep -qvE "^(view|download) " "$t/log" && fail "C still published:
$(log)"
rm "$t/state/index/index.json"
run && fail "C was published after B's index.json was deleted"
grep -q "watermark is serial 8 too" "$t/out" || fail "C's refusal doesn't come from the watermark"
dist=$t/dist-b
run || fail "B's retry failed"
dist=$dist_a
ok "an equal serial is a retry only if the bytes are the same"

# 11. Plugins. A missing, changed or extra file in dist publishes nothing, the
# new languages' releases included.
nothing_published() {
  grep -qvE "^(view|download) " "$t/log" && fail "$1 still published:
$(log)"
  true
}
reset
cp "$dist/$plugin_path" "$t/archive.good"
rm "$dist/$plugin_path"
run && fail "an index whose plugin archive is missing was published"
grep -q "the index names .*$plugin_file, which isn't there" "$t/out" || fail "the missing archive isn't named"
nothing_published "a missing plugin archive"
run --dry-run && fail "the dry run passed a missing plugin archive"
grep -qE "^gh release (create|upload)" "$t/out" && fail "the dry run would publish with a missing plugin archive"
head -c "$(wc -c <"$t/archive.good" | tr -d ' ')" /dev/zero >"$dist/$plugin_path"
run && fail "a changed plugin archive was published"
grep -q "$plugin_file isn't the file the index names" "$t/out" || fail "the changed archive isn't named"
nothing_published "a changed plugin archive"
cp "$t/archive.good" "$dist/$plugin_path"
echo extra >"$dist/plugins/acme.demo/0.2.0/extra"
run && fail "an unlisted plugin file was passed over"
grep -q "holds files its index entry doesn't list" "$t/out" || fail "the unlisted file isn't reported"
nothing_published "an unlisted plugin file"
rm "$dist/plugins/acme.demo/0.2.0/extra"
ok "a missing, changed or unlisted plugin file publishes nothing"

# 12. A published plugin release that differs from the entry (name, size, or
# only its bytes) fails naming it, publishes nothing and is left alone.
size=$(wc -c <"$t/archive.good" | tr -d ' ')
for case in "other.tar.gz $size" "$plugin_file $((size + 1))" "$plugin_file $size"; do
  read -r held bytes <<<"$case"
  reset
  printf '%s\t%s\n' "$held" "$bytes" | fake_release "$plugin_tag"
  cp "$t/state/$plugin_tag/$held" "$t/held"
  run && fail "a differing plugin release ($case) didn't fail"
  grep -q "acme.demo 0.2.0: the release $plugin_tag exists with" "$t/out" ||
    fail "the failure doesn't name the plugin ($case)"
  grep -q "A plugin's release is never replaced" "$t/out" || fail "the failure doesn't say what to do ($case)"
  nothing_published "a differing plugin release ($case)"
  [ "$(cd "$t/state/$plugin_tag" && echo *)" = "$held" ] && cmp -s "$t/held" "$t/state/$plugin_tag/$held" ||
    fail "the plugin's release was changed ($case)"
done
ok "a published plugin release that differs fails, and is never replaced"
reset
mkdir -p "$t/state/$plugin_tag"
cp "$t/archive.good" "$t/state/$plugin_tag/$plugin_file"
touch "$t/state/$plugin_tag.draft"
run && fail "a draft plugin release was taken as published"
grep -q "acme.demo 0.2.0: the release $plugin_tag is a draft" "$t/out" || fail "the draft plugin release isn't named"
nothing_published "a draft plugin release"
ok "a draft plugin release fails the run"

# 13. The tag is a content address: a changed archive, its version unchanged,
# is a new release, and the old one, which cached indexes point at, stays.
reset
run || fail "the setup publish failed"
echo "changed" >>"$plugins/demo/README.md"
build_dist "$t/dist-d" 9
new_tag=$(tag_of_archive "$t/dist-d/$plugin_path")
[ "$new_tag" != "$plugin_tag" ] || fail "the changed archive has the old tag"
dist=$t/dist-d
run || fail "a changed plugin's publish failed"
dist=$dist_a
[ "$(grep '^create' "$t/log")" = "create $new_tag $plugin_file" ] || fail "the changed plugin's creates:
$(log)"
cmp -s "$t/state/$plugin_tag/$plugin_file" "$t/archive.good" || fail "the old archive's release was changed"
ok "a changed plugin archive gets its own release, and the old one stays"

# 14. A version that can't name a release publishes nothing.
sed -i.bak 's/^version = .*/version = "0.3.0+build"/' "$plugins/demo/extension.toml" && rm "$plugins/demo/extension.toml.bak"
build_dist "$t/dist-e" 10
reset
dist=$t/dist-e
run && fail "a version with a + was published"
grep -q "has characters a release can't be named with" "$t/out" || fail "the version's refusal isn't explained"
nothing_published "an unnameable version"
dist=$dist_a
ok "a plugin version that can't name a release publishes nothing"
