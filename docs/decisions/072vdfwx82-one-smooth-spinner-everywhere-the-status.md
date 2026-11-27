# One smooth spinner everywhere (T2123)

Every `Icon::LoaderCircle` now draws through one kiln-ui `Spinner`
(`kiln/crates/kiln-ui/src/spinner.rs`). It turns once a second on the
display's frames, **capped at 60 frames a second**. Capped, a window with one
spinner still uses **22–24% of one core**, well above the "few percent" the
spec set. It used 38.5% before, in the Runs panel.

## What ships

- **One angle.** `spin_angle` moved here from the Runs panel. The angle is the
  time since one process-wide epoch, so every spinner on screen turns
  together.
- **Frames only while seen.** A spinner asks to be drawn again when it is
  painted inside the content mask. Removed, hidden or scrolled out of sight,
  it asks for none.
- **Reduce Motion.** It stands still at angle 0 and asks for no frames.
- **The 60/s cap.** Each window has one schedule, a frame due every 60th of a
  second, and its spinners' views are notified together on the first frame
  that is due, up to 2 ms early. The next is due a period after this one was,
  not after the frame came, so a 75 or 144 Hz display averages 60 too: every
  other frame at 120 Hz, every frame at 60 Hz. A frame a whole period late
  starts the schedule afresh, so a stall is not made up in a burst. The window
  waits once however many spinners and views ask.
- **Sites.** The status bar's operations item and its list, the Runs panel,
  pre-commit checks and chat tool and plan rows use `Spinner` directly. The
  run widget and debug run widget get it from kiln-ui's `Button`, and
  terminal tabs from `IconRef::element`.
- The operations item keeps its 100 ms timer for its label and bar.

## CPU

**Setup:** Apple M3 Max (10 performance and 4 efficiency cores), macOS 27.0,
the built-in 120 Hz display. Release builds with LTO off and 16 codegen units,
of the base (acc816a97) and of this change.

**Probe:** a throwaway binary, not committed, opened a real 1280×820
`Workbench` window on a project of one small file, with no editor open. It
showed nothing running (`idle`), a run in the Runs panel (`runs`) or a
status-bar operation (`status`). The window was a pop-up with no inactive
frame cap, so nothing covered it and it got every display frame whether or
not it had focus. A run is also an operation, so `runs` shows the status
bar's spinner too.

**Method:** after 8 s of warm-up, the app's CPU time over 30 s (`ps`), as a
share of one core, with `top` sampling the same 30 s once a second. Windows
were measured one at a time.

| Build and window | App CPU | `top` mean / max | WindowServer | Spinner paints/s | Load |
|---|---|---|---|---|---|
| Before, idle | 0.6% | 0.6 / 0.9% | 11.3% | — | 6.2 |
| After, idle | 0.6% | 0.5 / 0.9% | 10.2% | — | 3.8 |
| **Before, Runs panel** | **38.5%** | 39.0 / 48.7% | 44.3% | not counted | 3.6 |
| **Before, Runs panel**, again | **38.6%** | 38.9 / 49.0% | 44.1% | not counted | 3.2 |
| **After, status bar** | **23.7%** | 23.9 / 33.3% | 24.8% | 63 | 2.9 |
| **After, status bar**, again | **22.2%** | 22.1 / 31.2% | 24.5% | 63 | 5.2 |
| Before, status bar (10 steps a second) | 4.0% | 4.1 / 5.3% | 4.7% | — | 2.4 |
| After, Runs panel (two spinners) | 26.5% | 27.3 / 34.3% | 24.8% | 124 (two) | 3.5 |

WindowServer is the whole system's, so it includes whatever else was on
screen.

**Why the cap.** Before it, the new status-bar spinner painted 106 times a
second and used 38.6% of a core (load 23, so it didn't reach 120). The Runs
panel at the base measured 33.4% in the same batch of runs (load 32). Both are
far over the spec's bar, so the cap went in.

**After review.** The capped figures were measured with the first cap, a
12.5 ms minimum gap, which let a 75 Hz display redraw 75 times a second and a
144 Hz one 72. The schedule above replaced it. On the 120 Hz display measured
here both draw every other frame, so the CPU was not measured again.

## Known limits

- **60/s is not "a few percent".** The cost is redrawing the window, about
  3.5 ms of CPU a frame here, not the spinner: two spinners (26.5%) cost
  little more than one.
- **A status-bar operation costs more than it did.** The old stepping spinner
  used 4.0%; the smooth one uses 22–24% for as long as an operation runs, such
  as a language server indexing. A lower cap would trade smoothness for CPU.
  30/s was not measured.
- **Not measured:** a window with large editors open, which may cost more per
  frame; an unfocused window, which GPUI by default draws at most about 30
  times a second; a 60 Hz display; and power (`powermetrics` needs root).
