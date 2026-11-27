#!/usr/bin/env bash
# Publishes a signed grammar registry, as scripts/build-grammar-registry.sh
# wrote it, scripts/build-plugin-registry.sh added the plugins to it and
# `grammar-registry sign` signed it, to the GitHub releases of the registry
# repository (docs/release/grammar-registry.md,
# docs/release/plugin-registry.md).
#
#   scripts/publish-grammar-registry.sh --dist target/registry \
#     [--repo tenmoku-editor/registry] [--registry-toml FILE] [--dry-run]
#
# 1. Checks <dist>/index.json against <dist>/index.json.sig and the
#    registry.toml's public_key (`grammar-registry verify`), that the index's
#    url_template is the repository's, that the published index's serial
#    (or its watermark's) isn't newer than this one's, nor equal with other bytes, and that every file it names is in <dist>
#    with its size and SHA-256, plugins' archives included. A draft release (a
#    run cut off partway) fails the run. On any failure it publishes nothing.
# 2. For each language: if the release <tag> exists, its assets' names and
#    sizes must be the index entry's files', and it's skipped; otherwise
#    (once every existing one has checked out) it's created with exactly
#    those files. A language's release is never replaced or deleted: older
#    indexes still point at it, and its tag is its files' digest.
#    Each plugin's archive likewise, as the release <id>-<the first 16 hex
#    digits of its SHA-256>: a changed archive is a new release. The tag holds
#    only part of the digest, so an existing release's archive is also
#    downloaded and must have the entry's whole SHA-256.
# 3. Then the fixed release `index`, created once: the pack tarballs and
#    their .sha256 files, then index.json.sig, then index.json, each replacing
#    the last (--clobber). index.json goes last, so a reader never sees an
#    index whose files aren't up yet.
#
# gh takes its token from GH_TOKEN. --dry-run prints each gh command and runs
# none: it treats every release as absent. GRAMMAR_REGISTRY is the
# grammar-registry command (default a `cargo run` of crates/fe-registry's
# binary), GRAMMARS_DIR the grammars directory the release notes' source pins
# come from (default extensions/grammars), and TENMOKU_COMMIT the commit the
# notes name (default HEAD). scripts/test-publish-grammar-registry.sh tests
# this against a fake gh.
set -euo pipefail
# Byte order for globs and sort, whatever the caller's locale.
export LC_ALL=C

die() {
  echo "publish-grammar-registry: $*" >&2
  exit 1
}

usage="usage: $0 --dist <dir> [--repo <owner>/<name>] [--registry-toml <file>] [--dry-run]"

# An absolute path for $1, which must exist, so the cd below can't move it.
absolute() {
  [ -e "$1" ] || die "no $1"
  (cd "$(dirname "$1")" && printf '%s/%s\n' "$PWD" "$(basename "$1")")
}

dist=
repo=tenmoku-editor/registry
registry_toml=
dry_run=
while [ $# -gt 0 ]; do
  case $1 in
  --dist | --repo | --registry-toml)
    [ $# -ge 2 ] && [ -n "$2" ] || die "$1 needs a value ($usage)"
    case $1 in
    --dist) dist=$(absolute "$2") ;;
    --repo) repo=$2 ;;
    --registry-toml) registry_toml=$(absolute "$2") ;;
    esac
    shift 2
    ;;
  --dry-run)
    dry_run=1
    shift
    ;;
  *) die "unknown argument $1 ($usage)" ;;
  esac
done
[ -n "$dist" ] || die "--dist is required ($usage)"
[[ $repo =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || die "--repo $repo isn't <owner>/<name>"
grammars=$(absolute "${GRAMMARS_DIR:-$(dirname "$0")/../extensions/grammars}")

cd "$(dirname "$0")/.."
registry_toml=${registry_toml:-$PWD/extensions/grammars/registry.toml}
read -r -a registry <<<"${GRAMMAR_REGISTRY:-cargo run --quiet --locked -p fe-registry --bin grammar-registry --}"
commit=${TENMOKU_COMMIT:-$(git rev-parse HEAD)}

index=$dist/index.json
sig=$index.sig
[ -f "$index" ] || die "no $index: build it with scripts/build-grammar-registry.sh"
[ -f "$sig" ] || die "no $sig: sign the index with grammar-registry sign"

# Runs gh with the arguments, or with --dry-run prints them, shell-quoted.
gh_run() {
  if [ -n "$dry_run" ]; then
    printf 'gh'
    printf ' %q' "$@"
    printf '\n'
    return 0
  fi
  # Never the loop's stdin.
  gh "$@" </dev/null
}

sha256() {
  if command -v sha256sum >/dev/null; then
    sha256sum "$1"
  else
    shasum -a 256 "$1"
  fi | cut -d' ' -f1
}

size() {
  wc -c <"$1" | tr -d ' '
}

# Dies unless $1 is a file of size $2 and SHA-256 $3.
check_file() {
  [ -f "$1" ] || die "the index names $1, which isn't there"
  [ "$(size "$1")" = "$2" ] && [ "$(sha256 "$1")" = "$3" ] ||
    die "$1 isn't the file the index names (size $2, SHA-256 $3)"
}

# The value of key $1 in a grammar-source.toml $2, if it has it.
source_value() {
  sed -n "s/^$1 = \"\(.*\)\"\$/\1/p" "$2"
}

# The release notes of language $1: where its grammar comes from, and the
# commit that built it.
notes() {
  local file=$grammars/$1/grammar-source.toml pin
  [ -f "$file" ] || die "$1: no $file for its release notes"
  if [ -n "$(source_value git "$file")" ]; then
    pin="$(source_value git "$file") at $(source_value rev "$file")"
  elif [ -n "$(source_value crate "$file")" ]; then
    pin="the crate $(source_value crate "$file") $(source_value version "$file")"
  else
    die "$1: $file names no git or crate"
  fi
  printf 'Grammar source: %s.\nBuilt from Tenmoku commit %s.\n' "$pin" "$commit"
}

# What `gh release view` prints of a release: `draft\t<true|false>`, then its
# assets' `<name>\t<size>`.
view_jq='"draft\t\(.isDraft)", (.assets[] | "\(.name)\t\(.size)")'

# Looks up release $1: sets release_state to absent, draft or published, and
# release_assets to its assets' `<name>\t<size>` lines. gh's lookup finds
# drafts too, and gh creates a release with assets as a draft before it
# publishes it, so a run cut off partway can leave one. With --dry-run it
# prints the command, and every release is absent.
release_view() {
  release_state=absent
  release_assets=
  if [ -n "$dry_run" ]; then
    gh_run release view "$1" --repo "$repo" --json isDraft,assets --jq "$view_jq"
    return 0
  fi
  local err out
  err=$(mktemp)
  if out=$(gh release view "$1" --repo "$repo" --json isDraft,assets --jq "$view_jq" 2>"$err" </dev/null); then
    rm -f "$err"
    case $(head -n 1 <<<"$out") in
    $'draft\ttrue') release_state=draft ;;
    $'draft\tfalse') release_state=published ;;
    *) die "gh release view $1 didn't say whether it's a draft" ;;
    esac
    release_assets=$(tail -n +2 <<<"$out")
  elif grep -qi 'not found' "$err"; then
    rm -f "$err"
  else
    cat "$err" >&2
    rm -f "$err"
    die "gh release view $1 failed"
  fi
}

# Each plugin of the index as `<id>\t<version>\t<file>\t<size>\t<sha256>\t<name>`.
# grammar-registry has no command for them; it has verified and parsed the
# index by the time this reads it.
plugin_entries() {
  python3 -I - "$1" <<'PY'
import json, sys

with open(sys.argv[1], "rb") as f:
    plugins = json.load(f).get("plugins", {})
for _, p in sorted(plugins.items()):
    # The name, which may be empty or hold any text, goes last.
    fields = [p["id"], p["version"], p["file"], p["size"], p["sha256"], " ".join(p["name"].split())]
    print(*fields, sep="\t")
PY
}

# The release holding the plugin $1's archive of SHA-256 $2: the {tag} of its
# URL, a content address, which the app builds the same way
# (docs/release/plugin-registry.md).
plugin_tag() {
  printf '%s-%s\n' "$1" "${2:0:16}"
}

draft() {
  die "$1: the release $2 is a draft, which an earlier run left when it stopped partway: check it, delete it by hand (gh release delete $2 --repo $repo) and rerun. Publishing nothing."
}

# 1. Everything is checked before anything is published.
"${registry[@]}" verify --index "$index" --sig "$sig" --registry-toml "$registry_toml" ||
  die "$index doesn't verify: publishing nothing"

template=$(sed -n 's/^  "url_template": "\(.*\)",$/\1/p' "$index")
expected="https://github.com/$repo/releases/download/{tag}/{file}"
[ "$template" = "$expected" ] ||
  die "the index's url_template is $template, not $expected: it's for another repository"

languages=$("${registry[@]}" languages --index "$index")
tarballs=$("${registry[@]}" tarballs --index "$index")
serial=$("${registry[@]}" serial --index "$index")
plugins=$(plugin_entries "$index") || die "couldn't read the index's plugins: publishing nothing"

# Never replace a newer index: a rerun of an older run keeps its commit, and
# its older serial. Freshness is judged against two records of what was last
# published: the index.json asset, and a watermark in the release's notes,
# `serial=<n> sha256=<index.json's digest>`, written before any pack or index
# upload. `gh release upload --clobber` deletes the old asset before it
# uploads the new one, so a failed upload leaves the release without an
# index.json: the watermark is what still says how new it was. The same serial
# with the same bytes is a retry, and goes ahead; the serial is only a commit
# time, so the same serial with other bytes is another commit's index, and is
# refused. A published release with neither record can't be judged, and is
# refused. Neither is signature-checked here: only the registry's App can
# write the release, and an index signed by a rotated-out key must still be
# readable.
release_view index
index_state=$release_state
[ "$index_state" != draft ] || draft "the index" index
index_sha=$(sha256 "$index")
watermark_line="Publication watermark: serial=$serial sha256=$index_sha"
watermark_current=
# Dies unless a published index of serial $2 and digest $3, named $1, isn't
# newer than this one, nor another index of the same serial.
check_fresh() {
  [ "$serial" -ge "$2" ] ||
    die "the $1 is serial $2, newer than this one's $serial (a rerun of an older run?): publishing nothing"
  if [ "$serial" -eq "$2" ] && [ "$index_sha" != "$3" ]; then
    die "the $1 is serial $2 too, but not these bytes (SHA-256 $3, this one's $index_sha): another commit's index with the same commit time, so publishing nothing"
  fi
  echo "the $1 is serial $2; this one is $serial"
}
if [ "$index_state" = published ]; then
  body=$(gh release view index --repo "$repo" --json body --jq .body </dev/null) ||
    die "couldn't read the index release's notes: publishing nothing"
  mark=$(sed -n 's/.*Publication watermark: serial=\([0-9][0-9]*\) sha256=\([0-9a-f]*\).*/\1 \2/p' <<<"$body" | tail -n 1)
  known=
  if [ -n "$mark" ]; then
    read -r mark_serial mark_sha <<<"$mark"
    check_fresh "index release's watermark" "$mark_serial" "$mark_sha"
    [ "$mark_serial $mark_sha" != "$serial $index_sha" ] || watermark_current=1
    known=1
  fi
  if grep -q $'^index.json\t' <<<"$release_assets"; then
    published=$(mktemp -d)
    gh release download index --repo "$repo" --pattern index.json --dir "$published" </dev/null ||
      die "couldn't download the published index.json: publishing nothing"
    published_serial=$("${registry[@]}" serial --index "$published/index.json") ||
      die "the published index.json doesn't parse, so its serial is unknown: publishing nothing"
    published_sha=$(sha256 "$published/index.json")
    rm -rf "$published"
    check_fresh "published index" "$published_serial" "$published_sha"
    known=1
  fi
  [ -n "$known" ] ||
    die "the index release has neither an index.json nor a publication watermark, so whether this index is the newest can't be established: publishing nothing (restore index.json or add the watermark line to the release's notes by hand)"
fi

create=()
while IFS=$'\t' read -r language tag _ _; do
  [ -n "$language" ] || continue
  [[ $tag =~ ^[a-z0-9][a-z0-9._-]*$ ]] || die "$language's tag $tag is malformed"
  dir=$dist/languages/$language
  files=$("${registry[@]}" languages --index "$index" "$language")
  expected_assets=
  while IFS=$'\t' read -r name file_size file_sha; do
    check_file "$dir/$name" "$file_size" "$file_sha"
    expected_assets+="$name"$'\t'"$file_size"$'\n'
  done <<<"$files"
  [ "$(find "$dir" -mindepth 1 | wc -l | tr -d ' ')" = "$(grep -c . <<<"$files")" ] ||
    die "$dir holds files its index entry doesn't list"
  notes "$language" >/dev/null
  release_view "$tag"
  case $release_state in
  absent) create+=("$language") ;;
  draft) draft "$language" "$tag" ;;
  published)
    if [ "$(sort <<<"$release_assets")" != "$(printf '%s' "$expected_assets" | sort)" ]; then
      die "$language: the release $tag exists with other files than its index entry lists:
$release_assets
A language's release is never replaced: find out how it differs before deleting it by hand."
    fi
    echo "$language: $tag is published"
    ;;
  esac
done <<<"$languages"

create_plugins=()
while IFS=$'\t' read -r id version file file_size file_sha name; do
  [ -n "$id" ] || continue
  # What goes into a path, a tag and a URL: GitHub may rename an asset with
  # other characters, and then the index's URL wouldn't find it.
  for part in "$id" "$version" "$file"; do
    [[ $part =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]] ||
      die "the plugin $id $version: \"$part\" has characters a release can't be named with"
  done
  tag=$(plugin_tag "$id" "$file_sha")
  [[ $tag != *..* ]] || die "the plugin $id $version: $tag isn't a valid git tag"
  dir=$dist/plugins/$id/$version
  check_file "$dir/$file" "$file_size" "$file_sha"
  [ "$(find "$dir" -mindepth 1 | wc -l | tr -d ' ')" = 1 ] ||
    die "$dir holds files its index entry doesn't list"
  release_view "$tag"
  case $release_state in
  absent) create_plugins+=("$id") ;;
  draft) draft "$id $version" "$tag" ;;
  published)
    replaced="A plugin's release is never replaced: find out how it differs before deleting it by hand."
    if [ "$release_assets" != "$file"$'\t'"$file_size" ]; then
      die "$id $version: the release $tag exists with other files than its index entry lists:
$release_assets
$replaced"
    fi
    published=$(mktemp -d)
    gh release download "$tag" --repo "$repo" --pattern "$file" --dir "$published" </dev/null ||
      die "$id $version: couldn't download the published $file: publishing nothing"
    published_sha=$(sha256 "$published/$file")
    rm -rf "$published"
    [ "$published_sha" = "$file_sha" ] ||
      die "$id $version: the release $tag exists with another $file (SHA-256 $published_sha, the index lists $file_sha).
$replaced"
    echo "$id $version: $tag is published"
    ;;
  esac
done <<<"$plugins"

while IFS=$'\t' read -r name file_size file_sha; do
  [ -n "$name" ] || continue
  check_file "$dist/packs/$name" "$file_size" "$file_sha"
done <<<"$tarballs"
pack_files=()
for tarball in "$dist"/packs/*.tar.gz; do
  [ -f "$tarball" ] || continue
  [ -f "$tarball.sha256" ] || die "$tarball has no .sha256"
  pack_files+=("$tarball" "$tarball.sha256")
done

# 2. The new languages' releases.
for language in ${create[@]+"${create[@]}"}; do
  line=$(awk -F'\t' -v language="$language" '$1 == language' <<<"$languages")
  IFS=$'\t' read -r _ tag version name <<<"$line"
  files=$("${registry[@]}" languages --index "$index" "$language")
  text=$(notes "$language")
  paths=()
  while IFS=$'\t' read -r file _ _; do
    paths+=("$dist/languages/$language/$file")
  done <<<"$files"
  gh_run release create "$tag" --repo "$repo" --target main --latest=false \
    --title "$name syntax $version" --notes "$text" "${paths[@]}"
done

# The new plugin archives' releases.
for id in ${create_plugins[@]+"${create_plugins[@]}"}; do
  line=$(awk -F'\t' -v id="$id" '$1 == id' <<<"$plugins")
  IFS=$'\t' read -r _ version file _ file_sha name <<<"$line"
  gh_run release create "$(plugin_tag "$id" "$file_sha")" --repo "$repo" --target main --latest=false \
    --title "${name:-$id} plugin $version" \
    --notes "Tenmoku plugin $id $version."$'\n'"Built from Tenmoku commit $commit." \
    "$dist/plugins/$id/$version/$file"
done

# 3. The index, last of all. The concurrency group keeps another run from
# changing it since the check above.
index_notes="The grammar registry's index and offline packs. index.json is signed by
index.json.sig; check it with grammar-registry verify (docs/release/grammar-registry.md).
$watermark_line"
# The watermark goes in before any upload, so whatever a failed upload leaves,
# the release still records the newest serial that started to publish.
if [ "$index_state" = absent ]; then
  gh_run release create index --repo "$repo" --target main --latest=false \
    --title "Grammar index" --notes "$index_notes"
elif [ -z "$watermark_current" ]; then
  gh_run release edit index --repo "$repo" --notes "$index_notes"
fi
if [ ${#pack_files[@]} -gt 0 ]; then
  gh_run release upload index --repo "$repo" --clobber "${pack_files[@]}"
fi
gh_run release upload index --repo "$repo" --clobber "$sig"
gh_run release upload index --repo "$repo" --clobber "$index"
if [ -n "$dry_run" ]; then
  echo "dry run: would publish ${#create[@]} language and ${#create_plugins[@]} plugin releases, if none exist yet, and the index to $repo"
else
  echo "published ${#create[@]} new language releases, ${#create_plugins[@]} new plugin releases and the index (serial $serial) to $repo"
fi
