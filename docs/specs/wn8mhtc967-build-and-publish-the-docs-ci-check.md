---
task_uid: "01m7th3bqfpgv9s0wn8mhtc967"
task: "T2012"
rev: 2
type: "chore"
complexity: "s"
criteria:
  - id: "C1"
    text: "New docs.yml: on pull requests touching docs/user/**, crates/fe-config/src/settings.rs or crates/fe-keymap/keymaps/** it runs scripts/check-user-docs.sh and `mdbook build docs/user` (mdBook downloaded at a pinned version and verified by sha256, no `cargo install`); on a `v*` tag (stable only, not -preview) it also uploads `tenmoku-docs.tar.gz` (the built book) as a release asset, and on push to main it uploads the book as a workflow artifact `user-docs`."
    required: true
    manual: false
  - id: "C2"
    text: "scripts/test-docs-workflow.sh checks the workflow text (pinned version and checksum present, tag filter excludes `-preview`, no write permission outside the release step, the tenmoku.dev publish runs only on a stable tag after the asset upload and is skipped with a notice when the TENMOKU_DEV_DISPATCH_TOKEN secret is absent), as test-release-workflow.sh does for release.yml."
    required: true
    manual: false
  - id: "C3"
    text: "Publishing to tenmoku.dev/docs (user decision 2026-11-20): after the stable-tag asset upload, docs.yml starts the tenmoku-editor/tenmoku.dev Pages workflow (`gh workflow run pages.yml -R tenmoku-editor/tenmoku.dev`, token from the TENMOKU_DEV_DISPATCH_TOKEN secret); new docs/release/user-docs.md gives the asset URL, the secret to create (fine-grained, Actions write on the site repo only) and the exact site-repo pages.yml change that downloads the latest stable `tenmoku-docs.tar.gz` into _site/docs; the finish note lists those human and site-repo steps."
    required: true
    manual: false
---

# Build and publish the docs: CI check, mdBook release asset, and a pipeline that deploys tenmoku.dev/docs (help 3)

# Docs CI and publishing

Files read first: .github/workflows/ci.yml, release.yml (asset upload), scripts/test-release-workflow.sh. The book is static HTML; tenmoku.dev/docs must serve it. User decision 2026-11-20: a GitHub Actions pipeline publishes it there.

Hosting facts: tenmoku.dev is the separate public repo github.com/tenmoku-editor/tenmoku.dev, deployed to GitHub Pages by its own .github/workflows/pages.yml on pushes to main and workflow_dispatch; that build copies only index.html, favicon.ico, CNAME and assets/ into _site. Pipeline: this repo's docs.yml uploads `tenmoku-docs.tar.gz` on a stable tag and then starts the site repo's Pages workflow (`gh workflow run pages.yml -R tenmoku-editor/tenmoku.dev`, token from secret TENMOKU_DEV_DISPATCH_TOKEN, a fine-grained token with Actions write on that repo only; the step is skipped with a notice when the secret is absent, e.g. on forks). The site repo's build downloads the latest stable release's `tenmoku-docs.tar.gz` and unpacks it to _site/docs, so any later site deploy keeps the docs. That site-repo edit and the secret are outside this repo: docs/release/user-docs.md lists them as the human/site steps (the site repo is edited outside Regatta, as its landing page was).


Workflow edit: a change to .github/workflows/** needs a human risk approval at finish. Say so in the finish note and do not retry around it.


Read the files named before starting. Do not widen --scope; if a file outside it is needed, stop and respec. New tests go in new files.
