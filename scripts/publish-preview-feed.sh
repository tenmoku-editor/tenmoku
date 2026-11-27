#!/usr/bin/env bash
# Publishes the preview channel's update feeds to the rolling prerelease tagged `preview`, after a
# tagged release (preview or stable) has published. GitHub's releases/latest/download never lists
# a prerelease, so the preview feeds can't be read from the newest preview release itself: the
# app's preview feed URLs are github.com/<repo>/releases/download/preview/<feed>.
#
#   scripts/publish-preview-feed.sh --dist dist --tag vX.Y.Z[-preview.N] --repo <owner>/<name> \
#     [--target <commit>]
#
# From <dist> (every build job's files, as the release workflow's publish job has them) it takes:
#   appcast-preview.xml               Sparkle's preview feed (macOS), when the build wrote one.
#                                     Its one enclosure must be exactly this tag's macOS zip in the
#                                     tagged release (https://github.com/<repo>/releases/download/
#                                     <tag>/Tenmoku-<v>-macos-arm64.zip), never the rolling release
#                                     or another host: the zip stays where it is.
#   releases.<os>-<arch>-preview.json Velopack's preview feeds (Windows, Linux), with the .sig
#                                     beside each when signed, and the .nupkg packages each lists:
#                                     Velopack resolves a package beside its feed, so those go in
#                                     the rolling release.
# Every feed must be this tag's version. Nothing else in <dist> (the stable feeds, the zips, the
# installers) is published here.
#
# Checks first, publishing nothing on a failure:
# - The rolling release's build, a watermark line in its notes (`Preview build: version=<v>
#   tag=<tag>`), must not be newer than this one: the channel never moves backwards. The same
#   version is a re-run, and gives the same assets. A release with assets and no watermark can't
#   be judged, and is refused; so is a draft.
# - Each feed the rolling release has must be one this build replaces: a feed left behind would
#   list packages this run removes. A signed feed (or appcast) is never replaced by an unsigned
#   one (or none), which every installed copy would refuse.
#
# Then: creates the prerelease (never latest) on the first run, or updates its notes; writes the
# watermark before any upload; uploads the packages; for each feed removes its old .sig before
# uploading the new feed and then its .sig, so a stale .sig never sits beside a new feed; uploads
# the appcast; and last removes the previous build's packages, and any feed or .sig no longer
# published. gh takes its token from GH_TOKEN. scripts/test-publish-preview-feed.sh tests this
# against a fake gh.
set -euo pipefail
export LC_ALL=C

release=preview

die() {
  echo "publish-preview-feed: $*" >&2
  exit 1
}

usage="usage: $0 --dist <dir> --tag vX.Y.Z[-preview.N] --repo <owner>/<name> [--target <commit>]"

dist='' tag='' repo='' target=''
while [ $# -gt 0 ]; do
  case $1 in
  --dist | --tag | --repo | --target)
    [ $# -ge 2 ] && [ -n "$2" ] || die "$1 needs a value ($usage)"
    case $1 in
    --dist) dist=$2 ;;
    --tag) tag=$2 ;;
    --repo) repo=$2 ;;
    --target) target=$2 ;;
    esac
    shift 2
    ;;
  *) die "unknown argument $1 ($usage)" ;;
  esac
done
[ -n "$dist" ] && [ -n "$tag" ] && [ -n "$repo" ] || die "--dist, --tag and --repo are required ($usage)"
[ -d "$dist" ] || die "no directory $dist"
[[ $repo =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || die "--repo $repo isn't <owner>/<name>"
command -v jq >/dev/null || die "needs jq"

version_re='^([0-9]+)\.([0-9]+)\.([0-9]+)(-preview\.([0-9]+))?$'
version=${tag#v}
[ "v$version" = "$tag" ] && [[ $version =~ $version_re ]] ||
  die "the tag $tag isn't vX.Y.Z or vX.Y.Z-preview.N"

# The order of version $1: X Y Z, then a stable build after every preview of its X.Y.Z, then N.
key() {
  [[ $1 =~ $version_re ]] || die "$1 isn't X.Y.Z or X.Y.Z-preview.N"
  local stable=1 n=0
  if [ -n "${BASH_REMATCH[4]}" ]; then
    stable=0 n=${BASH_REMATCH[5]}
  fi
  printf '%d %d %d %d %d\n' "$((10#${BASH_REMATCH[1]}))" "$((10#${BASH_REMATCH[2]}))" \
    "$((10#${BASH_REMATCH[3]}))" "$stable" "$((10#$n))"
}
# Prints -1, 0 or 1 as version $1 is older than, the same as or newer than $2.
compare() {
  local a b i
  read -r -a a <<<"$(key "$1")"
  read -r -a b <<<"$(key "$2")"
  for i in 0 1 2 3 4; do
    if [ "${a[i]}" -lt "${b[i]}" ]; then echo -1 && return; fi
    if [ "${a[i]}" -gt "${b[i]}" ]; then echo 1 && return; fi
  done
  echo 0
}

size() {
  wc -c <"$1" | tr -d ' '
}

# Never the caller's stdin.
gh_run() {
  gh "$@" --repo "$repo" </dev/null
}

# 1. What this build publishes: `files` in upload order (packages, then each feed and its .sig,
# then the appcast), `feeds` and `packages` by name.
packages=() feeds=() names=()
appcast=
if [ -e "$dist/appcast-preview.xml" ]; then
  appcast=$dist/appcast-preview.xml
  urls=$(grep -o '<enclosure url="[^"]*"' "$appcast" | sed 's/^<enclosure url="//; s/"$//' || true)
  [ "$(grep -c . <<<"$urls")" = 1 ] || die "appcast-preview.xml has $(grep -c . <<<"$urls") enclosures, not one"
  zip=Tenmoku-$version-macos-arm64.zip
  # The zip stays in its tagged release: the rolling release holds only the feeds and packages.
  case $urls in
  */releases/download/$release/*)
    die "appcast-preview.xml's enclosure $urls is in the rolling $release release: it must be the tagged release's zip" ;;
  esac
  # Exactly the tagged release's zip: nowhere else is known to hold the zip just published.
  expected_url="https://github.com/$repo/releases/download/$tag/$zip"
  [ "$urls" = "$expected_url" ] ||
    die "appcast-preview.xml's enclosure $urls isn't the $tag release's zip, $expected_url"
  shown=$(sed -n 's/.*<sparkle:shortVersionString>\(.*\)<\/sparkle:shortVersionString>.*/\1/p' "$appcast")
  [ "$shown" = "$version" ] || die "appcast-preview.xml is version ${shown:-unknown}, not $version"
fi
for feed in "$dist"/releases.*-preview.json; do
  [ -f "$feed" ] || continue
  name=${feed##*/}
  listed=$(jq -r '.Assets[] | [.Version, .FileName, .Size] | @tsv' "$feed") ||
    die "$name isn't a Velopack feed"
  [ -n "$listed" ] || die "$name lists no packages"
  while IFS=$'\t' read -r listed_version file listed_size; do
    [ "$listed_version" = "$version" ] || die "$name lists $file of version $listed_version, not $version"
    [[ $file =~ ^[A-Za-z0-9._-]+\.nupkg$ ]] || die "$name lists $file, which isn't a .nupkg name"
    [ -f "$dist/$file" ] || die "$name lists $file, which isn't in $dist"
    [ "$(size "$dist/$file")" = "$listed_size" ] || die "$dist/$file isn't the size $name lists ($listed_size)"
    packages+=("$file")
  done <<<"$listed"
  feeds+=("$name")
done
[ ${#feeds[@]} -gt 0 ] || [ -n "$appcast" ] || die "no appcast-preview.xml or releases.*-preview.json in $dist: nothing to publish"
names=(${packages[@]+"${packages[@]}"})
for name in ${feeds[@]+"${feeds[@]}"}; do
  names+=("$name")
  [ ! -e "$dist/$name.sig" ] || names+=("$name.sig")
done
[ -z "$appcast" ] || names+=(appcast-preview.xml)
publishing() {
  printf '%s\n' "${names[@]}" | grep -qxF "$1"
}

# 2. The rolling release as it is: release_state absent, draft or published, and its assets'
# names. gh's lookup finds drafts too.
release_state=absent
assets=
err=$(mktemp)
trap 'rm -f "$err"' EXIT
if out=$(gh_run release view "$release" --json isDraft,assets --jq '"draft\t\(.isDraft)", (.assets[] | .name)' 2>"$err"); then
  case $(head -n 1 <<<"$out") in
  $'draft\ttrue') die "the $release release is a draft: check it, delete it by hand (gh release delete $release --repo $repo) and rerun. Publishing nothing." ;;
  $'draft\tfalse') release_state=published ;;
  *) die "gh release view $release didn't say whether it's a draft" ;;
  esac
  assets=$(tail -n +2 <<<"$out")
elif ! grep -qi 'not found' "$err"; then
  cat "$err" >&2
  die "gh release view $release failed"
fi
has_asset() {
  grep -qxF "$1" <<<"$assets"
}

watermark="Preview build: version=$version tag=$tag"
if [ "$release_state" = published ]; then
  body=$(gh_run release view "$release" --json body --jq .body) ||
    die "couldn't read the $release release's notes: publishing nothing"
  published=$(sed -n 's/^Preview build: version=\([^ ]*\) tag=.*/\1/p' <<<"$body" | tail -n 1)
  if [ -n "$published" ]; then
    [[ $published =~ $version_re ]] ||
      die "the $release release's watermark names version $published, which isn't X.Y.Z or X.Y.Z-preview.N: publishing nothing"
    case $(compare "$version" "$published") in
    -1) die "the $release release holds $published, newer than $version: the preview channel never moves backwards, so publishing nothing" ;;
    0) echo "the $release release holds $version already: publishing it again" ;;
    1) echo "the $release release holds $published; publishing $version" ;;
    esac
  elif [ -n "$assets" ]; then
    die "the $release release has assets but no 'Preview build:' watermark in its notes, so whether $version is newer can't be established: publishing nothing"
  fi
fi

# A feed the rolling release has and this build doesn't replace would list packages removed below;
# a signed one replaced by an unsigned one is refused by every installed copy.
while IFS= read -r asset; do
  case $asset in
  appcast-preview.xml)
    [ -n "$appcast" ] ||
      die "the $release release has appcast-preview.xml and this build made none: set SPARKLE_ED_PRIVATE_KEY, or delete that asset first. Publishing nothing." ;;
  releases.*.json)
    publishing "$asset" ||
      die "the $release release has $asset and this build made none: its packages would be removed. Publishing nothing." ;;
  releases.*.json.sig)
    publishing "$asset" ||
      die "the $release release has $asset and this build's ${asset%.sig} is unsigned: set TENMOKU_UPDATE_PRIVATE_KEY, or delete that asset first. Publishing nothing." ;;
  esac
done <<<"$assets"

# 3. Publish. The watermark first, so whatever a failed upload leaves, the release says the newest
# build that started to publish.
notes="The preview channel's update feeds: appcast-preview.xml (Sparkle, macOS) and
releases.<os>-<arch>-preview.json (Velopack, Windows and Linux), with the packages they list. Each
tagged release, preview or stable, replaces them with its own when it is the newest; the macOS zip
stays in that tagged release. Download Tenmoku from the tagged releases, not this one.

$watermark"
if [ "$release_state" = absent ]; then
  set -- --title "Tenmoku preview channel" --notes "$notes" --prerelease --latest=false
  [ -z "$target" ] || set -- "$@" --target "$target"
  gh_run release create "$release" "$@"
else
  gh_run release edit "$release" --notes "$notes" --prerelease --latest=false
fi
if [ ${#packages[@]} -gt 0 ]; then
  paths=()
  for file in "${packages[@]}"; do paths+=("$dist/$file"); done
  gh_run release upload "$release" "${paths[@]}" --clobber
fi
for name in ${feeds[@]+"${feeds[@]}"}; do
  # The old .sig goes before the new feed arrives: a feed with no .sig is refused until its own
  # arrives, never checked against another feed's.
  if has_asset "$name.sig"; then
    gh_run release delete-asset "$release" "$name.sig" --yes
  fi
  gh_run release upload "$release" "$dist/$name" --clobber
  if [ -e "$dist/$name.sig" ]; then
    gh_run release upload "$release" "$dist/$name.sig" --clobber
  fi
done
[ -z "$appcast" ] || gh_run release upload "$release" "$appcast" --clobber
# Last, once no feed lists them: the previous build's packages, and any feed or .sig not published
# now. Anything else on the release is left alone.
while IFS= read -r asset; do
  case $asset in
  *.nupkg | releases.*.json | releases.*.json.sig | appcast-preview.xml)
    publishing "$asset" || gh_run release delete-asset "$release" "$asset" --yes ;;
  esac
done <<<"$assets"
echo "published $version ($tag) to the $release release: ${names[*]}"
