#!/usr/bin/env bash
# Fixture test for scripts/publish-preview-feed.sh, against a fake gh on PATH that keeps each
# release in a directory, and for the release workflow's job that runs it. No network.
#
#   scripts/test-publish-preview-feed.sh
#
# The cases: the first publish creates the rolling `preview` prerelease (never latest); a preview
# after a preview, and a stable build after a preview, replace its feeds and packages (an old .sig
# gone before its new feed arrives, old packages only once no feed lists them); a re-run of a tag
# gives the same assets; an older build is refused and changes nothing; and the refusals that
# keep the channel whole (an enclosure in the rolling release, a feed left behind, a signed feed
# replaced by an unsigned one, a feed listing a package that isn't there).
# Prints one line per check and exits non-zero on the first failure. Needs jq.
set -euo pipefail
export LC_ALL=C

here=$(cd "$(dirname "$0")" && pwd)
publish=$here/publish-preview-feed.sh
wf=$here/../.github/workflows/release.yml
t=$(mktemp -d)
trap 'rm -rf "$t"' EXIT

fail() {
  echo "FAIL: $*"
  [ -f "$t/out" ] && sed 's/^/  | /' "$t/out"
  exit 1
}
ok() { echo "ok: $*"; }

repo=test-owner/tenmoku
mkdir -p "$t/bin" "$t/state"

# The fake gh: `release view|create|edit|upload|delete-asset <tag> ...`. Each changing call is a
# line of $GH_LOG: `<verb> <tag>` and the base names of its files or asset. A release is the
# directory $GH_STATE/<tag> of its assets, with its notes in <tag>.notes and its create/edit flags
# in <tag>.flags (one a line, the last call's).
cat >"$t/bin/gh" <<'EOF'
#!/usr/bin/env bash
set -eu
[ "$1" = release ] || { echo "fake gh: no $1" >&2; exit 2; }
verb=$2 tag=$3
shift 3
files=() json= notes= flags=()
while [ $# -gt 0 ]; do
  case $1 in
  --repo)
    [ "$2" = "$GH_REPO_EXPECTED" ] || { echo "fake gh: --repo $2" >&2; exit 2; }
    shift 2 ;;
  --json) json=$2; shift 2 ;;
  --notes) notes=$2; shift 2 ;;
  --title | --target) flags+=("$1 $2"); shift 2 ;;
  --jq) shift 2 ;;
  --prerelease | --latest=false | --latest) flags+=("$1"); shift ;;
  --clobber | --yes) shift ;;
  -*) echo "fake gh: unknown flag $1" >&2; exit 2 ;;
  *) files+=("$1"); shift ;;
  esac
done
release=$GH_STATE/$tag
if [ "$verb" = view ]; then
  [ -d "$release" ] || { echo "release not found" >&2; exit 1; }
  case $json in
  body) cat "$release.notes" ;;
  isDraft,assets)
    if [ -e "$release.draft" ]; then printf 'draft\ttrue\n'; else printf 'draft\tfalse\n'; fi
    for asset in "$release"/*; do [ -f "$asset" ] && basename "$asset"; done
    true ;;
  *) echo "fake gh: --json $json" >&2; exit 2 ;;
  esac
  exit 0
fi
line="$verb $tag"
for file in ${files[@]+"${files[@]}"}; do line+=" $(basename "$file")"; done
echo "$line" >>"$GH_LOG"
case $verb in
create)
  [ ! -d "$release" ] || { echo "a release with the same tag name already exists: $tag" >&2; exit 1; }
  [ ${#files[@]} = 0 ] || { echo "fake gh: create with assets" >&2; exit 2; }
  mkdir "$release"
  printf '%s\n' "$notes" >"$release.notes"
  printf '%s\n' ${flags[@]+"${flags[@]}"} >"$release.flags" ;;
edit)
  [ -d "$release" ] || { echo "release not found" >&2; exit 1; }
  printf '%s\n' "$notes" >"$release.notes"
  printf '%s\n' ${flags[@]+"${flags[@]}"} >"$release.flags" ;;
upload)
  [ -d "$release" ] || { echo "release not found" >&2; exit 1; }
  for file in "${files[@]}"; do cp "$file" "$release/"; done ;;
delete-asset)
  [ -f "$release/${files[0]}" ] || { echo "asset not found" >&2; exit 1; }
  rm "$release/${files[0]}" ;;
*) echo "fake gh: no release $verb" >&2; exit 2 ;;
esac
EOF
chmod +x "$t/bin/gh"
export PATH="$t/bin:$PATH" GH_STATE="$t/state" GH_LOG="$t/log" GH_REPO_EXPECTED=$repo

platforms="linux-x86_64 windows-x64 windows-arm64"

# dist VERSION [unsigned] [no-appcast]: a fresh $t/dist as the publish job has it for VERSION: the
# zip and installers; per platform the preview feed and its package, and for a stable version the
# stable feed and package too, each feed's .sig unless unsigned; appcast-preview.xml (and for a
# stable version appcast.xml) with the zip in the tagged release, unless no-appcast. The same
# arguments give the same bytes.
dist() {
  local v=$1 d=$t/dist p c channels file
  shift
  rm -rf "$d"
  mkdir -p "$d"
  echo "zip $v" >"$d/Tenmoku-$v-macos-arm64.zip"
  case $v in
  *-preview.*) channels=preview ;;
  *) channels="stable preview" ;;
  esac
  for p in $platforms; do
    echo "setup $v $p" >"$d/Tenmoku-$v-$p-Setup.exe"
    for c in $channels; do
      file=Tenmoku-$v-$p-$c-full.nupkg
      echo "package $v $p $c" >"$d/$file"
      printf '{"Assets":[{"PackageId":"Tenmoku","Version":"%s","Type":"Full","FileName":"%s","SHA256":"00","Size":%s}]}' \
        "$v" "$file" "$(wc -c <"$d/$file" | tr -d ' ')" >"$d/releases.$p-$c.json"
      case " $* " in
      *" unsigned "*) ;;
      *) echo "signature of $v $p $c" >"$d/releases.$p-$c.json.sig" ;;
      esac
    done
  done
  case " $* " in
  *" no-appcast "*) ;;
  *)
    for c in $channels; do
      [ "$c" = stable ] && file=appcast.xml || file=appcast-preview.xml
      cat >"$d/$file" <<XML
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <item>
      <sparkle:version>$v</sparkle:version>
      <sparkle:shortVersionString>$v</sparkle:shortVersionString>
      <enclosure url="https://github.com/$repo/releases/download/v$v/Tenmoku-$v-macos-arm64.zip" length="4" type="application/octet-stream" sparkle:edSignature="AbCd=="/>
    </item>
  </channel>
</rss>
XML
    done
    ;;
  esac
}

# run VERSION: publishes $t/dist as tag vVERSION; output in $t/out, gh's changing calls in $t/log.
run() {
  : >"$t/log"
  "$publish" --dist "$t/dist" --tag "v$1" --repo "$repo" --target 0123abc >"$t/out" 2>&1
}

# The rolling release's assets, `<name> <first line>` each.
snapshot() {
  local f
  for f in "$t/state/preview"/*; do printf '%s %s\n' "${f##*/}" "$(head -n 1 "$f")"; done
}

# The assets the rolling release must hold for VERSION, as snapshot prints them.
expected() {
  local v=$1 p
  shift
  {
    echo "appcast-preview.xml <?xml version=\"1.0\" encoding=\"utf-8\"?>"
    for p in $platforms; do
      echo "Tenmoku-$v-$p-preview-full.nupkg package $v $p preview"
      printf 'releases.%s-preview.json {"Assets":[{"PackageId":"Tenmoku","Version":"%s","Type":"Full","FileName":"Tenmoku-%s-%s-preview-full.nupkg","SHA256":"00","Size":%s}]}\n' \
        "$p" "$v" "$v" "$p" "$(printf 'package %s %s preview\n' "$v" "$p" | wc -c | tr -d ' ')"
      case " $* " in
      *" unsigned "*) ;;
      *) echo "releases.$p-preview.json.sig signature of $v $p preview" ;;
      esac
    done
  } | sort
}

check_holds() {
  local v=$1
  shift
  diff <(expected "$v" "$@") <(snapshot) >"$t/diff" ||
    fail "the preview release doesn't hold $v's feeds and packages: $(cat "$t/diff")"
  grep -qx "Preview build: version=$v tag=v$v" "$t/state/preview.notes" ||
    fail "the notes' watermark isn't $v: $(cat "$t/state/preview.notes")"
  grep -qx -- --prerelease "$t/state/preview.flags" && grep -qx -- --latest=false "$t/state/preview.flags" ||
    fail "the preview release isn't a prerelease that is never latest: $(cat "$t/state/preview.flags")"
}

# Dies unless line $1 of the log comes before line $2.
before() {
  local a b
  a=$(grep -nxF "$1" "$t/log" | head -n 1 | cut -d: -f1)
  b=$(grep -nxF "$2" "$t/log" | head -n 1 | cut -d: -f1)
  [ -n "$a" ] && [ -n "$b" ] && [ "$a" -lt "$b" ] || fail "'$1' isn't before '$2' in: $(cat "$t/log")"
}

# Dies unless publishing VERSION (dist's further arguments) is refused with message $2 and the
# release and its notes are as they were.
refused() {
  local v=$1 why=$2
  shift 2
  dist "$v" "$@"
  snapshot >"$t/before"
  cp "$t/state/preview.notes" "$t/notes.before"
  if run "$v"; then fail "publishing $v $* was not refused"; fi
  grep -qF -- "$why" "$t/out" || fail "publishing $v $* was refused, but not because: $why"
  [ ! -s "$t/log" ] || fail "publishing $v $* was refused after changing the release: $(cat "$t/log")"
  snapshot | cmp -s - "$t/before" && cmp -s "$t/state/preview.notes" "$t/notes.before" ||
    fail "a refused $v $* changed the release"
}

# First create: a preview build, with no preview release yet.
dist 1.2.0-preview.1
run 1.2.0-preview.1 || fail "the first publish failed"
check_holds 1.2.0-preview.1
[ "$(head -n 1 "$t/log")" = "create preview" ] || fail "the first publish didn't create the release first: $(cat "$t/log")"
grep -qx -- '--target 0123abc' "$t/state/preview.flags" || fail "the release wasn't created at --target"
! grep -qx -- --latest "$t/state/preview.flags" || fail "the preview release was marked latest"
ok "first publish: creates the preview prerelease (never latest) holding appcast-preview.xml, each releases.<os>-<arch>-preview.json, its .sig and the .nupkg it lists, and none of the build's other files"

# Preview after preview.
dist 1.2.0-preview.2
run 1.2.0-preview.2 || fail "a preview after a preview failed"
check_holds 1.2.0-preview.2
grep -qx 'edit preview' "$t/log" || fail "the notes weren't updated: $(cat "$t/log")"
for p in $platforms; do
  before "delete-asset preview releases.$p-preview.json.sig" "upload preview releases.$p-preview.json"
  before "upload preview releases.$p-preview.json" "upload preview releases.$p-preview.json.sig"
  before "upload preview releases.$p-preview.json.sig" "delete-asset preview Tenmoku-1.2.0-preview.1-$p-preview-full.nupkg"
  before "edit preview" "upload preview releases.$p-preview.json"
done
before "upload preview Tenmoku-1.2.0-preview.2-linux-x86_64-preview-full.nupkg Tenmoku-1.2.0-preview.2-windows-arm64-preview-full.nupkg Tenmoku-1.2.0-preview.2-windows-x64-preview-full.nupkg" \
  "upload preview releases.linux-x86_64-preview.json"
ok "preview after preview: the old .sig removed before its new feed is uploaded, the new packages before the feeds, the old packages only after; the release holds only the new build's"

# Stable after preview: the stable build's preview feeds, never its stable ones.
dist 1.2.0
run 1.2.0 || fail "a stable build after a preview failed"
check_holds 1.2.0
! grep -q -e 'stable' -e 'appcast\.xml' -e 'Setup\.exe' -e '\.zip' "$t/log" ||
  fail "a stable build published more than its preview feeds: $(cat "$t/log")"
ok "stable after preview: the release holds the stable build's preview feeds and packages, not its stable feeds, zip or installers"

# A re-run of the same tag: the same assets.
snapshot >"$t/first"
dist 1.2.0
run 1.2.0 || fail "a re-run failed"
check_holds 1.2.0
snapshot | cmp -s - "$t/first" || fail "a re-run changed the assets: $(snapshot | diff "$t/first" -)"
grep -q 'holds 1.2.0 already' "$t/out" || fail "a re-run didn't say so: $(cat "$t/out")"
ok "a re-run of a tag gives the same assets"

# Older builds are refused: a preview of the version now stable, and an older stable.
refused 1.2.0-preview.3 "holds 1.2.0, newer than 1.2.0-preview.3"
refused 1.1.9 "holds 1.2.0, newer than 1.1.9"
ok "an older build (1.2.0-preview.3 or 1.1.9 after 1.2.0) is refused and changes nothing"

# The queue's order isn't the versions': runs for 1.2.1-preview.1, then 1.2.1-preview.3 (pending),
# then an older 1.2.1-preview.2 arriving after it. queue: max keeps every run, so the newest
# publishes and the older one, run last, is refused.
dist 1.2.1-preview.1 && run 1.2.1-preview.1 || fail "1.2.1-preview.1 failed"
dist 1.2.1-preview.3 && run 1.2.1-preview.3 || fail "the newest, 1.2.1-preview.3, failed"
refused 1.2.1-preview.2 "holds 1.2.1-preview.3, newer than 1.2.1-preview.2"
check_holds 1.2.1-preview.3
dist 1.2.0 && run 1.2.0 && fail "1.2.0 after 1.2.1-preview.3 was published"
grep -qF "holds 1.2.1-preview.3, newer than 1.2.0" "$t/out" || fail "no reason given for 1.2.0"
ok "runs out of version order: the newest publishes, an older one arriving after it is refused"

# Refusals that keep the channel whole.
dist 1.3.0-preview.1
sed -i.bak 's#/releases/download/v1.3.0-preview.1/#/releases/download/preview/#' "$t/dist/appcast-preview.xml"
cp "$t/state/preview.notes" "$t/notes.before"
if run 1.3.0-preview.1; then fail "an enclosure in the rolling release was published"; fi
grep -qF "is in the rolling preview release" "$t/out" || fail "no reason given for the enclosure"
# The right file name elsewhere: another tag's release, another repository's, another host.
for elsewhere in "https://github.com/$repo/releases/download/v1.2.0/" \
  "https://github.com/other-owner/tenmoku/releases/download/v1.3.0-preview.1/" \
  "https://downloads.example.test/v1.3.0-preview.1/"; do
  dist 1.3.0-preview.1
  sed -i.bak "s#https://github.com/$repo/releases/download/v1.3.0-preview.1/#$elsewhere#" "$t/dist/appcast-preview.xml"
  grep -qF "$elsewhere" "$t/dist/appcast-preview.xml" || fail "the fixture's enclosure wasn't moved to $elsewhere"
  if run 1.3.0-preview.1; then fail "an enclosure at $elsewhere was published"; fi
  grep -qF "isn't the v1.3.0-preview.1 release's zip, https://github.com/$repo/releases/download/v1.3.0-preview.1/Tenmoku-1.3.0-preview.1-macos-arm64.zip" "$t/out" ||
    fail "no reason given for the enclosure at $elsewhere"
done
[ ! -s "$t/log" ] && cmp -s "$t/state/preview.notes" "$t/notes.before" || fail "a refused enclosure changed the release"
ok "appcast-preview.xml's enclosure must be exactly the tagged release's zip: not the rolling release, another tag's, another repository's or another host's"
refused 1.3.0-preview.1 "has releases.linux-x86_64-preview.json.sig and this build's releases.linux-x86_64-preview.json is unsigned" unsigned
refused 1.3.0-preview.1 "has appcast-preview.xml and this build made none" no-appcast
dist 1.3.0-preview.1
rm "$t/dist/releases.windows-arm64-preview.json" "$t/dist/releases.windows-arm64-preview.json.sig"
if run 1.3.0-preview.1; then fail "a build without one of the release's feeds was published"; fi
grep -qF "has releases.windows-arm64-preview.json and this build made none" "$t/out" || fail "no reason given for the missing feed"
dist 1.3.0-preview.1
rm "$t/dist/Tenmoku-1.3.0-preview.1-linux-x86_64-preview-full.nupkg"
if run 1.3.0-preview.1; then fail "a feed listing a missing package was published"; fi
grep -qF "lists Tenmoku-1.3.0-preview.1-linux-x86_64-preview-full.nupkg, which isn't in" "$t/out" || fail "no reason given for the missing package"
[ ! -s "$t/log" ] || fail "a refused build changed the release: $(cat "$t/log")"
check_holds 1.2.1-preview.3
ok "refused, changing nothing: an unsigned feed over a signed one, no appcast over one, a feed left behind, a feed listing a package that isn't there"

# An unsigned channel publishes unsigned (no .sig to go stale), on a fresh release.
rm -rf "$t/state"/preview*
dist 1.3.0-preview.1 unsigned
run 1.3.0-preview.1 || fail "an unsigned first publish failed"
check_holds 1.3.0-preview.1 unsigned
dist 1.3.0-preview.2
run 1.3.0-preview.2 || fail "a signed build over an unsigned one failed"
check_holds 1.3.0-preview.2
ok "unsigned feeds publish with no .sig, and a signed build replaces them"

# A release that can't be judged, or a draft, is refused.
: >"$t/state/preview.notes"
refused 1.3.0-preview.3 "has assets but no 'Preview build:' watermark"
printf 'Preview build: version=1.3.0-preview.2 tag=v1.3.0-preview.2\n' >"$t/state/preview.notes"
: >"$t/state/preview.draft"
refused 1.3.0-preview.3 "the preview release is a draft"
rm "$t/state/preview.draft"
ok "a release with assets and no watermark, or a draft, is refused"

# The workflow runs it after publish, for every tag, in a job of its own that queues behind any
# other run's.
job=$(awk '/^jobs:$/ { in_jobs = 1 } in_jobs && $0 == "  preview-feed:" { on = 1; next } on && /^  [^ ]/ { exit } on' "$wf")
[ -n "$job" ] || fail "release.yml has no preview-feed job"
for line in "    needs: publish" "    if: github.event_name == 'push' && github.ref_type == 'tag'" \
  "      contents: write" "      group: preview-feed" "      cancel-in-progress: false" "      queue: max" \
  "          ref: \${{ github.ref }}" "          persist-credentials: false" "          merge-multiple: true" \
  "          GH_TOKEN: \${{ github.token }}" \
  '        run: scripts/publish-preview-feed.sh --dist dist --tag "$GITHUB_REF_NAME" --repo "$GH_REPO" --target "$GITHUB_SHA"'; do
  grep -qxF -- "$line" <<<"$job" || fail "the preview-feed job lacks: $line"
done
ok "release.yml's preview-feed job runs scripts/publish-preview-feed.sh after publish on every tag, one run at a time"
