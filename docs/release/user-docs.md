# Publishing the user docs

`.github/workflows/docs.yml` builds the user guide (`docs/user`, an mdBook).

- Pull requests touching `docs/user/**`, `crates/fe-config/src/settings.rs` or
  `crates/fe-keymap/keymaps/**` run `scripts/check-user-docs.sh` and `mdbook build docs/user`.
- Pushes to `main` keep the book as the `user-docs` workflow artifact.
- A stable `v*` tag (not `-preview`) uploads `tenmoku-docs.tar.gz` to that release, then starts
  the site repo's Pages workflow. mdBook is downloaded at a pinned version and checked by sha256
  (`MDBOOK_VERSION`, `MDBOOK_SHA256` in the workflow); bump both together.

Asset URL of the latest stable release:
`https://github.com/tenmoku-editor/tenmoku/releases/latest/download/tenmoku-docs.tar.gz`

## Human steps

1. Create the secret `TENMOKU_DEV_DISPATCH_TOKEN` in this repo (Settings, Secrets and variables,
   Actions). It is a fine-grained personal access token whose only repository is
   `tenmoku-editor/tenmoku.dev`, with the single permission *Actions: Read and write*. Without
   it the publish step prints a notice and is skipped (as on forks).
2. Edit `.github/workflows/pages.yml` in `tenmoku-editor/tenmoku.dev`: it must keep
   `workflow_dispatch:` and, before the Pages upload step, unpack the docs into `_site/docs`:

   ```yaml
         - name: Add the user docs
           env:
             GH_TOKEN: ${{ github.token }}
           run: |
             mkdir -p _site/docs
             curl -sSfL https://github.com/tenmoku-editor/tenmoku/releases/latest/download/tenmoku-docs.tar.gz \
               | tar -xz -C _site/docs
   ```

   `releases/latest` is the latest stable release, so any later site deploy keeps the docs.
   The step (not the build that copies `index.html`, `favicon.ico`, `CNAME` and `assets/`) is the
   only place `_site/docs` is filled.
