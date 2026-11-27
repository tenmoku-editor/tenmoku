//! Language servers' code lenses as clickable rows above their lines, as
//! VS Code shows them: asked for when a buffer opens on its server, 250 ms
//! after its last edit, and at once when the server asks for a refresh. The
//! lenses without a command are resolved, all of an answer's together,
//! before the answer shows; one that resolves to no command is dropped.
//!
//! An answer replaces a buffer's lenses only if it is for the buffer's
//! current version; until one lands the old lenses stay, their rows
//! following the edits. A buffer has one [`LensProvider`], added to each of
//! its editors. A click runs `lsp.run_lens`: the lens's command goes to its
//! server, and one the server doesn't list is said in an info toast; the
//! client commands that list references (`editor.action.showReferences`,
//! `rust-analyzer.showReferences`) open the References panel with them.
//! `editor.code_lens` off removes the providers and asks for nothing.
//!
//! A buffer's lenses are merged across the servers it's on, in fe-lsp, and
//! each resolves and runs through the server that sent it. A server handed
//! a notification ([`fe_lsp::LspEvent::Notified`]), or whose poll comes due
//! ([`fe_lsp::LspManager::repoll_due`]), is asked again, the lenses shown
//! kept until the answer; an answer equal to the last changes nothing.

use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    time::{Duration, Instant},
};

use fe_buffer::{Anchor, BufferId, BufferSnapshot, Version};
use fe_command::{Invocation, Value};
use fe_gpui::{EditorView, Lens, LensItem, LensProvider};
use fe_lsp::{CodeLens, IncarnationId, LspError, LspNavigation, ServerId, SessionId};
use gpui::{AppContext as _, Context, Entity, EntityId, SharedString, Task};
use kiln_l10n::tr;

use super::{InFlight, Reask};
use crate::{
    Workbench,
    workbench::toast::{self, Toast},
};

/// How long a buffer holds still before its lenses are asked for again.
const REST: Duration = Duration::from_millis(250);

/// A server the buffer is on, and the opening of the document it has.
type Served = (ServerId, SessionId, IncarnationId);

/// What a request answers.
type Answer = Result<Option<Vec<CodeLens>>, LspError>;

/// One lens row's items, on the row its anchor is on.
type Row = (Anchor, Vec<(SharedString, u64)>);

/// What a buffer's editors show: the rows, in row order.
#[derive(Default)]
struct Shown(Vec<Row>);

impl LensProvider for Shown {
    fn lenses(&mut self, snapshot: &BufferSnapshot) -> Vec<Lens> {
        let text = snapshot.text();
        (self.0.iter())
            .map(|(anchor, items)| Lens {
                row: text.offset_to_point(snapshot.resolve(anchor)).row,
                items: items
                    .iter()
                    .map(|(title, id)| LensItem {
                        title: title.clone(),
                        warning: false,
                        invocation: Some(invocation(*id)),
                    })
                    .collect(),
            })
            .collect()
    }
}

/// What a toast says of a lens titled `title` whose command can't run.
fn unsupported(title: &str) -> SharedString {
    tr!("workbench-lens-unsupported", title = title)
}

/// The click on lens `id`.
fn invocation(id: u64) -> Invocation {
    Invocation::of("lsp.run_lens")
        .expect("lsp.run_lens is registered")
        .args(Value::Object(
            [("lens".to_owned(), id.into())].into_iter().collect(),
        ))
}

struct Slot {
    /// The editors the provider is added to.
    editors: HashMap<EntityId, Entity<EditorView>>,
    /// The servers the buffer is on while lenses are wanted, each with the
    /// opening it had when lenses were last asked for. A new opening of
    /// one (a restarted server, a document reopened), or a server left,
    /// retires the lenses and requests of the old; a server joining asks
    /// again.
    asked_of: Vec<Served>,
    /// The version last seen, to ask again when it changes.
    key: Option<Version>,
    /// The version of the latest request.
    asked: Option<Version>,
    /// The servers that answered that none of them provides lenses.
    no_provider: Option<Vec<Served>>,
    /// The wait, then the request and its resolves.
    task: Option<Task<()>>,
    /// Whether `task` is still waiting or asking.
    in_flight: InFlight,
    /// The latest poll of each server the slot has seen.
    polled: HashMap<ServerId, Instant>,
    /// The last answer shown, resolved, and its version.
    answered: Option<(Version, Vec<CodeLens>)>,
    shown: Rc<RefCell<Shown>>,
    provider: Rc<RefCell<dyn LensProvider>>,
    /// The ids of the lenses shown, in `Lenses::table`.
    ids: Vec<u64>,
}

impl Slot {
    fn new() -> Self {
        let shown = Rc::new(RefCell::new(Shown::default()));
        Self {
            editors: HashMap::new(),
            asked_of: Vec::new(),
            key: None,
            asked: None,
            no_provider: None,
            task: None,
            in_flight: InFlight::default(),
            polled: HashMap::new(),
            answered: None,
            provider: shown.clone(),
            shown,
            ids: Vec::new(),
        }
    }
}

/// Every open buffer's lenses.
#[derive(Default)]
pub(super) struct Lenses {
    slots: HashMap<BufferId, Slot>,
    /// The lenses shown, by id: what a click runs.
    table: HashMap<u64, CodeLens>,
    /// The symbol of each references lens in `table`, read from the text it
    /// was asked on.
    origins: HashMap<u64, String>,
    next: u64,
}

impl Workbench {
    /// Brings each buffer's lenses up to date with its text and server: run
    /// by [`Self::sync_lsp`] after each draw.
    pub(super) fn sync_lenses(&mut self, cx: &mut Context<Self>) {
        let on = self.lsp.code_lens_on.get();
        let editors = self.open_editors(cx);
        let mut wanted: HashMap<BufferId, (Version, Vec<Entity<EditorView>>)> = HashMap::new();
        for editor in &editors {
            let model = editor.read(cx).model();
            let (buffer, version) = (model.buffer().id(), model.snapshot().version());
            wanted
                .entry(buffer)
                .or_insert_with(|| (version, Vec::new()))
                .1
                .push(editor.clone());
        }
        let gone: Vec<BufferId> = (self.lsp.lenses.slots.keys())
            .filter(|buffer| !wanted.contains_key(buffer))
            .copied()
            .collect();
        for buffer in gone {
            self.drop_lenses(buffer, cx);
        }
        for (buffer, (version, editors)) in wanted {
            let served = (self.lsp.manager.as_ref())
                .filter(|_| on)
                .map(|manager| super::lock(manager))
                .filter(|manager| manager.is_served(buffer))
                .map(|manager| manager.served_identities(buffer));
            let Some(served) = served else {
                self.drop_lenses(buffer, cx);
                continue;
            };
            let slot = self
                .lsp
                .lenses
                .slots
                .entry(buffer)
                .or_insert_with(Slot::new);
            // Another opening of the document on a server, or a server
            // left, retires the old lenses; a server joining asks again.
            if slot.asked_of != served {
                let retired = slot.asked_of.iter().any(|was| !served.contains(was));
                slot.polled
                    .retain(|was, _| served.iter().any(|(server, ..)| server == was));
                slot.asked_of = served.clone();
                slot.key = None;
                slot.no_provider = None;
                if retired {
                    slot.asked = None;
                    slot.task = None;
                    self.clear_lenses(buffer);
                }
            }
            let slot = self.lsp.lenses.slots.get_mut(&buffer).expect("just added");
            // Editors closed leave the provider with them; new ones get it.
            slot.editors
                .retain(|id, _| editors.iter().any(|editor| editor.entity_id() == *id));
            for editor in &editors {
                if slot
                    .editors
                    .insert(editor.entity_id(), editor.clone())
                    .is_none()
                {
                    let provider = slot.provider.clone();
                    editor.update(cx, |view, cx| {
                        view.add_lens_provider(provider);
                        cx.notify();
                    });
                }
            }
            // An answer with no opening's identity is discarded, and the
            // ask is made again once one turns active.
            if served.is_empty()
                || slot.no_provider.as_ref() == Some(&served)
                || slot.key == Some(version)
            {
                continue;
            }
            // The first ask goes at once; later ones wait for the text to
            // hold still.
            let first = slot.key.is_none();
            slot.key = Some(version);
            let delay = if first { Duration::ZERO } else { REST };
            self.ask_lenses_in(buffer, delay, cx);
        }
    }

    /// Asks again, at once, for every buffer on `server`.
    pub(super) fn refresh_lenses(&mut self, server: ServerId, cx: &mut Context<Self>) {
        for buffer in self.lens_slots_of(server) {
            if let Some(slot) = self.lsp.lenses.slots.get_mut(&buffer) {
                slot.no_provider = None;
            }
            self.ask_lenses_in(buffer, Duration::ZERO, cx);
        }
    }

    /// The buffers whose lenses are asked of `server`.
    fn lens_slots_of(&self, server: ServerId) -> Vec<BufferId> {
        (self.lsp.lenses.slots.iter())
            .filter(|(_, slot)| slot.asked_of.iter().any(|(of, ..)| *of == server))
            .map(|(buffer, _)| *buffer)
            .collect()
    }

    /// Asks the lenses of every buffer on `server` again, keeping those
    /// shown, for `why` (see [`Reask`]).
    pub(super) fn reask_lenses_of(&mut self, server: ServerId, why: Reask, cx: &mut Context<Self>) {
        for buffer in self.lens_slots_of(server) {
            self.reask_lenses(buffer, why, cx);
        }
    }

    /// Asks `buffer`'s lenses again for `why`, if it has asked before: a
    /// poll only while no request of its own is in flight.
    fn reask_lenses(&mut self, buffer: BufferId, why: Reask, cx: &mut Context<Self>) {
        let Some(slot) = self.lsp.lenses.slots.get(&buffer) else {
            return;
        };
        if slot.key.is_none() || slot.no_provider.as_ref() == Some(&slot.asked_of) {
            return;
        }
        match why {
            Reask::Notified(rest) => self.ask_lenses_in(buffer, rest, cx),
            Reask::Poll if !slot.in_flight.busy() => self.ask_lenses_in(buffer, Duration::ZERO, cx),
            Reask::Poll => {}
        }
    }

    /// Asks again each buffer's lenses of a server whose poll in `dues`
    /// came since the buffer last saw one.
    pub(super) fn repoll_lenses(
        &mut self,
        dues: &HashMap<ServerId, Instant>,
        cx: &mut Context<Self>,
    ) {
        let mut due = Vec::new();
        for (buffer, slot) in &mut self.lsp.lenses.slots {
            let servers: Vec<ServerId> = slot.asked_of.iter().map(|(server, ..)| *server).collect();
            let mut came = false;
            for server in servers {
                came |= super::poll_came(&mut slot.polled, server, dues);
            }
            if came {
                due.push(*buffer);
            }
        }
        for buffer in due {
            self.reask_lenses(buffer, Reask::Poll, cx);
        }
    }

    /// Removes `buffer`'s lenses and providers.
    fn drop_lenses(&mut self, buffer: BufferId, cx: &mut Context<Self>) {
        self.clear_lenses(buffer);
        let Some(slot) = self.lsp.lenses.slots.remove(&buffer) else {
            return;
        };
        for editor in slot.editors.values() {
            let provider = slot.provider.clone();
            editor.update(cx, |view, cx| {
                view.remove_lens_provider(&provider);
                cx.notify();
            });
        }
    }

    /// Forgets the lenses `buffer` shows.
    fn clear_lenses(&mut self, buffer: BufferId) {
        let lenses = &mut self.lsp.lenses;
        if let Some(slot) = lenses.slots.get_mut(&buffer) {
            slot.answered = None;
            slot.shown.borrow_mut().0.clear();
            for id in slot.ids.drain(..) {
                lenses.table.remove(&id);
                lenses.origins.remove(&id);
            }
        }
    }

    /// Has `buffer` ask for its lenses after `delay`, in place of any
    /// request waiting or in flight.
    fn ask_lenses_in(&mut self, buffer: BufferId, delay: Duration, cx: &mut Context<Self>) {
        let Some(asking) = (self.lsp.lenses.slots.get(&buffer)).map(|slot| slot.in_flight.asking())
        else {
            return;
        };
        let task = cx.spawn(async move |workbench, cx| {
            let _asking = asking;
            if !delay.is_zero() {
                cx.background_executor().timer(delay).await;
            }
            let Ok(Some((version, asked_of, reply))) =
                workbench.update(cx, |workbench, cx| workbench.send_lenses(buffer, cx))
            else {
                return;
            };
            let answer = match reply.await {
                Ok(Some(lenses)) => {
                    // Lenses asked on text the buffer has moved on from are
                    // neither resolved nor shown: the next ask replaces them.
                    let current = workbench
                        .update(cx, |workbench, cx| {
                            workbench.lens_request_current(buffer, version, &asked_of, cx)
                        })
                        .unwrap_or(false);
                    if !current {
                        return;
                    }
                    // The ones without a command resolve together.
                    let resolving: Vec<_> = workbench
                        .update(cx, |workbench, cx| {
                            let manager = workbench.lsp.manager.clone();
                            lenses
                                .into_iter()
                                .map(|lens| match &manager {
                                    Some(manager) if lens.command.is_none() => {
                                        let reply =
                                            super::lock(manager).resolve_code_lens(lens.clone());
                                        (lens, Some(cx.background_spawn(reply)))
                                    }
                                    _ => (lens, None),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let mut done = Vec::new();
                    for (lens, resolve) in resolving {
                        let lens = match resolve {
                            Some(resolve) => resolve.await.unwrap_or(lens),
                            None => lens,
                        };
                        done.push(lens);
                    }
                    Ok(Some(done))
                }
                other => other,
            };
            workbench
                .update(cx, |workbench, cx| {
                    workbench.lenses_landed(buffer, version, &asked_of, answer, cx)
                })
                .ok();
        });
        if let Some(slot) = self.lsp.lenses.slots.get_mut(&buffer) {
            slot.task = Some(task);
        }
    }

    /// The openings `buffer` has on its servers now.
    fn lens_openings(&self, buffer: BufferId) -> Vec<Served> {
        (self.lsp.manager.as_ref())
            .map(|manager| super::lock(manager).served_identities(buffer))
            .unwrap_or_default()
    }

    /// Whether the request for `buffer`'s lenses at `version`, made on
    /// the openings `asked_of`, is still the latest and the text and the
    /// servers' openings are as it was asked on.
    fn lens_request_current(
        &self,
        buffer: BufferId,
        version: Version,
        asked_of: &[Served],
        cx: &Context<Self>,
    ) -> bool {
        let Some(slot) = self.lsp.lenses.slots.get(&buffer) else {
            return false;
        };
        let text_is = slot
            .editors
            .values()
            .next()
            .is_some_and(|editor| editor.read(cx).model().snapshot().version() == version);
        slot.asked == Some(version)
            && !asked_of.is_empty()
            && asked_of == self.lens_openings(buffer)
            && text_is
    }

    /// The request for `buffer`'s lenses at its version now, sent.
    #[allow(clippy::type_complexity)]
    fn send_lenses(
        &mut self,
        buffer: BufferId,
        cx: &mut Context<Self>,
    ) -> Option<(
        Version,
        Vec<Served>,
        std::pin::Pin<Box<dyn std::future::Future<Output = Answer> + Send>>,
    )> {
        let manager = self.lsp.manager.clone()?;
        let slot = self.lsp.lenses.slots.get_mut(&buffer)?;
        let editor = slot.editors.values().next()?;
        let version = editor.read(cx).model().snapshot().version();
        slot.asked = Some(version);
        let manager = super::lock(&manager);
        let asked_of = manager.served_identities(buffer);
        let reply = manager.code_lenses(buffer, version);
        Some((version, asked_of, Box::pin(reply)))
    }

    /// Shows `answer`, the reply to the request for `buffer` at `version`,
    /// if it is still the latest, from the openings its servers still
    /// have, and the text hasn't moved on.
    fn lenses_landed(
        &mut self,
        buffer: BufferId,
        version: Version,
        asked_of: &[Served],
        answer: Answer,
        cx: &mut Context<Self>,
    ) {
        if !self.lsp.code_lens_on.get() {
            return;
        }
        // From the openings the servers have now.
        let current = self.lens_openings(buffer);
        let Some(slot) = self.lsp.lenses.slots.get_mut(&buffer) else {
            return;
        };
        if slot.asked != Some(version) || asked_of.is_empty() || asked_of != current {
            return;
        }
        let snapshot = slot
            .editors
            .values()
            .next()
            .map(|editor| editor.read(cx).model().snapshot());
        let Some(snapshot) = snapshot.filter(|snapshot| snapshot.version() == version) else {
            return;
        };
        let lenses = match answer {
            Ok(Some(lenses)) => lenses,
            Ok(None) => {
                slot.no_provider = Some(asked_of.to_vec());
                Vec::new()
            }
            Err(error) => return log::debug!("no code lenses: {error}"),
        };
        // Asked again with nothing changed (a poll, say): nothing to show
        // anew.
        let answered = Some((version, lenses));
        if slot.answered == answered {
            return;
        }
        slot.answered.clone_from(&answered);
        let lenses = answered.map(|(_, lenses)| lenses).unwrap_or_default();
        // A lens that resolved to no command is dropped.
        let text = snapshot.text();
        let Lenses {
            slots,
            table,
            origins,
            next,
        } = &mut self.lsp.lenses;
        let slot = slots.get_mut(&buffer).expect("looked up above");
        for id in slot.ids.drain(..) {
            table.remove(&id);
            origins.remove(&id);
        }
        let language = slot.editors.values().next().and_then(|editor| {
            editor
                .read(cx)
                .syntax()
                .map(|syntax| syntax.language().name())
        });
        let mut rows: Vec<(u32, Row)> = Vec::new();
        for lens in lenses {
            let Some(title) = lens
                .command
                .as_ref()
                .map(|c| SharedString::from(c.title.clone()))
            else {
                continue;
            };
            let row = text.offset_to_point(lens.range.start.min(text.len())).row;
            let id = *next;
            *next += 1;
            slot.ids.push(id);
            if (lens.command.as_ref()).is_some_and(LspNavigation::is_references_command) {
                let word = crate::references_panel::word_at(text, lens.range.start, language)
                    .unwrap_or_default();
                origins.insert(id, word);
            }
            table.insert(id, lens);
            match rows.last_mut() {
                Some((last, (_, items))) if *last == row => items.push((title, id)),
                _ => {
                    let anchor = snapshot.anchor_before(text.row_start(row));
                    rows.push((row, (anchor, vec![(title, id)])));
                }
            }
        }
        slot.shown.borrow_mut().0 = rows.into_iter().map(|(_, row)| row).collect();
        for editor in slot.editors.values() {
            editor.update(cx, |_, cx| cx.notify());
        }
    }

    /// Runs lens `id`'s command, as a click on it asks: a command its
    /// server lists goes to it, any other is said in a toast.
    pub(crate) fn run_lens(&mut self, id: u64, cx: &mut Context<Self>) {
        let (Some(lens), Some(manager)) = (
            self.lsp.lenses.table.get(&id).cloned(),
            self.lsp.manager.clone(),
        ) else {
            return;
        };
        let Some(title) = lens.command.as_ref().map(|c| c.title.clone()) else {
            return;
        };
        // The client commands that list references open the References
        // panel; nothing goes to the server.
        if (lens.command.as_ref()).is_some_and(LspNavigation::is_references_command) {
            let origin = self.lsp.lenses.origins.get(&id).cloned();
            let context = lens.context().clone();
            let locations = lens
                .command
                .as_ref()
                .and_then(LspNavigation::lens_locations);
            let (Some(word), Some(locations)) = (origin, locations) else {
                log::debug!("lens {title:?}: unusable arguments");
                toast::show(cx, Toast::info(unsupported(&title)));
                return;
            };
            // The panel takes focus in the window the click was in, once this
            // update is over.
            cx.spawn(async move |this, cx| {
                let Some(window) =
                    cx.update(|cx| cx.active_window().or_else(|| cx.windows().last().copied()))
                else {
                    return;
                };
                cx.update_window(window, |_, window, cx| {
                    this.update(cx, |this, cx| {
                        let decode = LspNavigation::decode_references;
                        this.show_references(word, context, locations, decode, window, cx)
                    })
                    .ok();
                })
                .ok();
            })
            .detach();
            return;
        }
        let reply = super::lock(&manager).run_lens_command(&lens);
        cx.spawn(async move |_, cx| match reply.await {
            Ok(()) => {}
            Err(LspError::Unavailable(why)) => {
                log::debug!("lens {title:?} not run: {why}");
                cx.update(|cx| toast::show(cx, Toast::info(unsupported(&title))));
            }
            Err(error) => {
                let failed = tr!(
                    "workbench-lens-failed",
                    title = &title,
                    error = error.to_string()
                );
                cx.update(|cx| toast::show(cx, Toast::info(failed)));
            }
        })
        .detach();
    }
}
