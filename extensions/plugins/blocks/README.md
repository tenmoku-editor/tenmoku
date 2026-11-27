# Blocks

A falling-blocks game in a panel. Start it with **Blocks: Play** from the
command palette, which shows the Blocks panel and gives it focus, or open the
panel from the sidebar and press Enter.

Blocks is an optional plugin: it is listed in the extension registry and is
never installed or turned on by default. It asks for three capabilities:

- `panels`, to show its panel;
- `panels.realtime`, to be ticked for gravity and handed the keys while its
  panel has focus;
- `commands`, for **Blocks: Play**.

It reads no files, runs no programs and makes no network requests. The best
score is kept in the plugin's own data, which the editor holds for it.

## Playing

The well is ten squares wide and twenty high. Pieces come from a shuffled bag
of all seven, so each run of seven holds one of each. Clearing lines scores
points, and every ten lines the level goes up and the pieces fall faster, from
one row every 500 ms down to one every 80 ms. A piece that has no room to come
in ends the game.

| Key | Does |
|---|---|
| Enter | start a game |
| ← → or h l | move |
| ↑ or k | rotate clockwise |
| ↓ or j | soft drop |
| Space | hard drop |
| P | pause or resume |
| R | restart |
| Esc | back to the editor |

The game only runs while its panel is shown and has focus. Anywhere else, in
the editor or with the panel hidden, it waits where it was.

## Building

```zsh
rustup target add wasm32-wasip2   # once
./build.zsh                       # arguments go to cargo, such as --offline
cargo test                        # the game's rules, natively
```

`build.zsh` builds the crate for `wasm32-wasip2` and copies the component to
`blocks.wasm`, which `extension.toml` names and the repository keeps, since CI
doesn't build guests. `crates/fe-workbench/tests/first_party_blocks.rs` loads
it in the editor's plugin host. Rebuild it after any change here, to the WIT or
to the host's world.

## Known limits

This is a gap in the host, not in the plugin:

- **The description shows in the manifest and here only.** A signed index's
  plugin entry has no description field, and the Extensions list shows a
  plugin's name, version and capabilities, so "a falling-blocks game started
  with Blocks: Play" is in `extension.toml`'s `description` and in this
  README, which the registry archive carries. Showing it in both is T2171.

## Licence

Apache-2.0, like Tenmoku. No third-party assets.
