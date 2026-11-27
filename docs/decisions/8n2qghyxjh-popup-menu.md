# T2155: kiln-ui PopupMenu, state the host keeps

The spec asked for a `PopupMenu` entity. It is a plain struct the host keeps
in its own open-menu state, not a gpui `Entity`:

- C2 freezes the four menus' tests. Those tests read the highlight with
  `&self` accessors that get no `App`: `panel.read_with(cx, |panel, _|
  panel.menu_highlighted())`, `new_menu_highlighted`,
  `editor_menu_highlighted`, the branches menu's `highlighted_line` and
  `actions`. State in another entity can't be read that way.
- When a host keeps focus, its own key listener has to hand the menu keys.
  A separate entity would then be updated from inside the host's update, and
  any callback from it into the host would be re-entrant. With the state in
  the host, the menu answers synchronously (`MenuInput`), and each host runs
  the chosen entry inside the same listener, as it did before.

A host gives `PopupMenu::render`/`wire` two things: `MenuOf<V>` (a fn that
finds its menu, `None` once closed) and `OnMenuInput<V>` (a fn that handles
`Handled | Chosen(ix) | Escape | PressedOutside(at)`). `PressedOutside`
carries the press's position, so a host whose opener toggles the menu (the
New menu's `+`, the branch widget, the branch actions beside the rows) can
skip a press there.

Focus is either the host's (the default; `key` is fed by the host's focused
element) or the menu's own (`take_focus`, moved from fe-gpui's `MenuFocus`).
The project panel and Graph menus must keep the panel focused, because their
tests assert it.
