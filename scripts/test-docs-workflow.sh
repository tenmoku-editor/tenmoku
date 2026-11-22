#!/bin/sh
# Checks for the docs workflow (.github/workflows/docs.yml): actionlint, triggers, the pinned and
# checksummed mdBook, the tag filter, where write permission appears, and the tenmoku.dev publish.
# Needs actionlint (brew install actionlint).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
wf="$here/../.github/workflows/docs.yml"
fail() { echo "FAIL: $*"; exit 1; }
ok() { echo "ok: $*"; }
has() { grep -q -E -- "$1" "$wf" || fail "docs.yml lacks /$1/ ($2)"; }
job() { awk -v j="  $1:" '/^jobs:$/ { in_jobs = 1 } in_jobs && $0 == j { on = 1; next } on && /^  [^ ]/ { exit } on' "$wf"; }

command -v actionlint >/dev/null 2>&1 || fail "actionlint not installed (brew install actionlint)"
actionlint "$wf" || fail "actionlint docs.yml"
ok "actionlint"

has "^      - 'docs/user/\*\*'$" "docs/user path"
has "^      - 'crates/fe-config/src/settings.rs'$" "settings path"
has "^      - 'crates/fe-keymap/keymaps/\*\*'$" "keymaps path"
has "run: scripts/check-user-docs.sh$" "structure check"
has "run: mdbook build docs/user$" "book build"
ok "triggers and build steps"

has "^  MDBOOK_VERSION: v[0-9]+\.[0-9]+\.[0-9]+$" "pinned mdBook version"
has "^  MDBOOK_SHA256: [0-9a-f]{64}$" "mdBook checksum"
has "sha256sum -c" "checksum verification"
grep -q "cargo install" "$wf" && fail "docs.yml uses cargo install"
ok "mdBook pinned and verified"

has "^    tags: \['v\*', '!v\*-preview\*'\]$" "stable tags only"
has "name: user-docs$" "main artifact"
has "tenmoku-docs.tar.gz" "asset name"
ok "tag filter and artifacts"

# Write permission: no write grant of any kind outside the release-asset job.
rest=$(awk -v j="  release-asset:" '/^jobs:$/ { in_jobs = 1 } in_jobs && $0 == j { skip = 1; next } skip && /^  [^ ]/ { skip = 0 } !skip' "$wf")
printf '%s\n' "$rest" | grep -q -i -E 'write' && fail "a write permission outside release-asset"
[ "$(printf '%s\n' "$rest" | grep -c '^permissions:$')" = 1 ] || fail "top-level permissions missing"
printf '%s\n' "$rest" | grep -q -x '  contents: read' || fail "top-level permissions are not contents: read"
job release-asset | grep -q -x '      contents: write' || fail "contents: write is not in the release-asset job"
job release-asset | grep -q -i -E '(write-all|(actions|id-token|packages|pull-requests|issues|deployments|checks|statuses): )' && fail "release-asset has more than contents: write"
job release-asset | grep -q 'gh release upload "\$GITHUB_REF_NAME" tenmoku-docs.tar.gz' || fail "release-asset does not upload the book"
ok "write permission only in the release step"

pub=$(job publish-site)
printf '%s\n' "$pub" | grep -q "if: github.ref_type == 'tag'" || fail "publish-site is not tag-only"
printf '%s\n' "$pub" | grep -q "needs: release-asset" || fail "publish-site does not follow the asset upload"
printf '%s\n' "$pub" | grep -q 'secrets.TENMOKU_DEV_DISPATCH_TOKEN' || fail "publish-site lacks the token secret"
printf '%s\n' "$pub" | grep -q 'gh workflow run pages.yml -R tenmoku-editor/tenmoku.dev' || fail "publish-site does not start pages.yml"
[ "$(grep -c 'secrets.TENMOKU_DEV_DISPATCH_TOKEN' "$wf")" = 1 ] || fail "the secret appears outside publish-site"
# Run the step's script with a stub gh: no token skips with a notice, a token dispatches.
t=$(mktemp -d)
trap 'rm -rf "$t"' EXIT
printf '%s\n' "$pub" | awk '/^        run: \|$/ { on = 1; next } on { sub(/^          /, ""); print }' >"$t/step.sh"
[ -s "$t/step.sh" ] || fail "could not extract the publish step"
mkdir "$t/bin"
printf '#!/bin/sh\necho "$*" >>"%s/calls"\n' "$t" >"$t/bin/gh"
chmod +x "$t/bin/gh"
out=$(GH_TOKEN= PATH="$t/bin:$PATH" bash "$t/step.sh") || fail "the step fails with no token"
printf '%s\n' "$out" | grep -q '::notice::' || fail "no skip notice with no token"
[ ! -e "$t/calls" ] || fail "dispatch ran with no token"
GH_TOKEN=x PATH="$t/bin:$PATH" bash "$t/step.sh" >/dev/null || fail "the step fails with a token"
grep -q -x 'workflow run pages.yml -R tenmoku-editor/tenmoku.dev' "$t/calls" || fail "no dispatch with a token"
ok "tenmoku.dev publish"
