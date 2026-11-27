//! Language servers' inlay hints in editors, as VS Code asks for them: the
//! hints of the rows in view, a viewport above and below included, 250 ms
//! after the buffer or the rows in view last changed, and at once when the
//! server asks for a refresh. `editor.inlay_hints` off shows none and asks
//! for none.
//!
//! A buffer's hints are merged across the servers it's on, in fe-lsp, and
//! each resolves through the server that sent it. An answer lands only if
//! it is for the latest request the editor made; servers none of which
//! provides hints aren't asked again until the buffer's servers change.
//! A server asking for a refresh, or handed a notification
//! ([`fe_lsp::LspEvent::Notified`]), or whose poll comes due
//! ([`fe_lsp::LspManager::repoll_due`]) is asked again, what shows kept
//! until the answer; an answer equal to the hints shown changes nothing.

use std::{
    collections::HashMap,
    ops::Range,
    rc::Rc,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};

use fe_buffer::Version;
use fe_gpui::{EditorView, InlayHint, InlayHintFuture, InlayHintHost, InlayHintKind};
use fe_lsp::LspManager;
use fe_lsp::{IncarnationId, LspError, ServerId, SessionId};
use gpui::{AppContext as _, Context, Entity, EntityId, Task};

use super::{InFlight, Reask};
use crate::Workbench;

/// How long the buffer and the rows in view hold still before hints are
/// asked for.
/// Texts up to this many bytes have their outline found on the main thread.
const INLINE_OUTLINE_BYTES: usize = 256 * 1024;

const REST: Duration = Duration::from_millis(250);

/// What was asked for last: the buffer version and the rows in view.
type Key = (Version, (u32, u32));

/// What a request answers.
type Answer = Result<Option<Vec<fe_lsp::InlayHint>>, LspError>;

/// A server the buffer is on, and the opening of it the server has.
type Served = (ServerId, SessionId, IncarnationId);

struct Slot {
    editor: Entity<EditorView>,
    /// The servers the buffer is on while hints are wanted, each with its
    /// opening: another set asks again.
    asked_of: Vec<Served>,
    /// The version and rows last seen, to ask again when they change.
    key: Option<Key>,
    /// The version and byte range of the latest request.
    asked: Option<(Version, Range<usize>)>,
    /// The servers that answered that none of them provides hints.
    no_provider: Option<Vec<Served>>,
    /// The wait, then the request.
    task: Option<Task<()>>,
    /// Whether `task` is still waiting or asking.
    in_flight: InFlight,
    /// The latest poll of each server the slot has seen.
    polled: HashMap<ServerId, Instant>,
    /// The manager the editor's host resolves through.
    hosted: Option<Weak<Mutex<LspManager>>>,
    /// The server's hints and the version they're of.
    served: Option<(Version, Vec<InlayHint>)>,
    /// The closing-brace hints from the outline and the version they're of.
    closing: Option<(Version, Vec<InlayHint>)>,
    /// What the closing hints were last asked for: the version, whether
    /// they're on and the fewest rows.
    closing_key: Option<(Version, bool, u32)>,
    /// The outline being found.
    closing_task: Option<Task<()>>,
}

/// Every open editor's hints.
#[derive(Default)]
pub(super) struct Slots {
    slots: HashMap<EntityId, Slot>,
}

impl Workbench {
    /// Brings each editor's hints up to date with its buffer and the rows
    /// in view: run by [`Self::sync_lsp`] after each draw.
    pub(super) fn sync_inlay_hints(&mut self, cx: &mut Context<Self>) {
        let editors = self.open_editors(cx);
        let on = self.lsp.inlay_hints_on.get();
        self.lsp
            .inlay_hints
            .slots
            .retain(|id, _| editors.iter().any(|editor| editor.entity_id() == *id));
        for editor in &editors {
            let id = editor.entity_id();
            let (buffer, version) = {
                let model = editor.read(cx).model();
                (model.buffer().id(), model.snapshot().version())
            };
            let served = match (&self.lsp.manager, on) {
                (Some(manager), true) => super::lock(manager).served_identities(buffer),
                _ => Vec::new(),
            };
            let rows = editor.read(cx).viewport_rows();
            let slot = self
                .lsp
                .inlay_hints
                .slots
                .entry(id)
                .or_insert_with(|| Slot {
                    editor: editor.clone(),
                    asked_of: Vec::new(),
                    key: None,
                    asked: None,
                    no_provider: None,
                    task: None,
                    in_flight: InFlight::default(),
                    polled: HashMap::new(),
                    hosted: None,
                    served: None,
                    closing: None,
                    closing_key: None,
                    closing_task: None,
                });
            // A new manager (the root changed) doesn't know the old one's
            // hints: they go, and the host follows.
            if let Some(manager) = &self.lsp.manager {
                let manager = Arc::downgrade(manager);
                if !slot
                    .hosted
                    .as_ref()
                    .is_some_and(|w| Weak::ptr_eq(w, &manager))
                {
                    if slot.hosted.replace(manager.clone()).is_some() {
                        slot.asked_of.clear();
                        slot.polled.clear();
                        slot.key = None;
                        slot.asked = None;
                        slot.no_provider = None;
                        slot.task = None;
                    }
                    let host = Rc::new(Host { manager });
                    slot.served = None;
                    editor.update(cx, |view, _| {
                        view.set_inlay_hint_host(Some(host));
                    });
                    publish(slot, cx);
                }
            }
            if served.is_empty() {
                if !slot.asked_of.is_empty() {
                    slot.asked_of.clear();
                    slot.polled.clear();
                    slot.key = None;
                    slot.asked = None;
                    slot.no_provider = None;
                    slot.task = None;
                    slot.served = None;
                    publish(slot, cx);
                }
                continue;
            }
            if slot.asked_of != served {
                // A server the buffer left takes its hints and the request
                // in flight with it; one joining, or a server's new opening,
                // only asks again.
                let left = (slot.asked_of.iter())
                    .any(|(was, ..)| !served.iter().any(|(server, ..)| server == was));
                slot.polled
                    .retain(|was, _| served.iter().any(|(server, ..)| server == was));
                slot.asked_of = served;
                slot.key = None;
                slot.no_provider = None;
                if left {
                    slot.asked = None;
                    slot.task = None;
                    slot.served = None;
                    publish(slot, cx);
                }
            }
            if slot.no_provider.as_ref() == Some(&slot.asked_of) {
                continue;
            }
            let Some(rows) = rows else { continue };
            let key = (version, rows);
            if slot.key == Some(key) {
                continue;
            }
            slot.key = Some(key);
            self.ask_inlay_hints_in(id, REST, cx);
        }
        self.sync_closing_hints(&editors, cx);
    }

    /// Finds each editor's closing-brace hints from its outline when its
    /// parsed text or the settings changed, and shows them with the server's.
    fn sync_closing_hints(&mut self, editors: &[Entity<EditorView>], cx: &mut Context<Self>) {
        let (on, min_rows) = self.lsp.closing_hints.get();
        for editor in editors {
            let id = editor.entity_id();
            let Some(slot) = self.lsp.inlay_hints.slots.get_mut(&id) else {
                continue;
            };
            let view = editor.read(cx);
            let version = view.model().snapshot().version();
            let key = (version, on, min_rows);
            let len = view.model().snapshot().text().len();
            if slot.closing_key == Some(key) {
                continue;
            }
            let job = if !on {
                None
            } else if let Some(syntax) = view.syntax() {
                match fe_syntax::outline_job(syntax) {
                    Ok(job) => Some(job),
                    // Reparsed soon, and drawn again then.
                    Err(fe_syntax::NoOutline::NotParsed) => continue,
                    Err(_) => None,
                }
            } else if view.is_loading_language() {
                // Its grammar lands later, and the editor is drawn again.
                continue;
            } else {
                None
            };
            slot.closing_key = Some(key);
            let Some(job) = job else {
                slot.closing_task = None;
                if slot.closing.take().is_some() {
                    publish(slot, cx);
                }
                continue;
            };
            // Small texts' outlines take a few milliseconds: found here, so
            // no job is in flight when the window closes. Big ones go to
            // the background.
            let (found, inline) = if len > INLINE_OUTLINE_BYTES {
                (Some(cx.background_spawn(async move { job.run() })), None)
            } else {
                (None, Some(job))
            };
            slot.closing_task = Some(cx.spawn(async move |workbench, cx| {
                let outline = match (found, inline) {
                    (Some(found), _) => found.await,
                    (None, Some(job)) => job.run(),
                    (None, None) => return,
                };
                workbench
                    .update(cx, |workbench, cx| {
                        workbench.closing_hints_landed(id, key, outline, cx)
                    })
                    .ok();
            }));
        }
    }

    /// Shows `outline`'s closing-brace hints if the editor is still at the
    /// version and settings they were asked for at.
    fn closing_hints_landed(
        &mut self,
        id: EntityId,
        key: (Version, bool, u32),
        outline: fe_syntax::Outline,
        cx: &mut Context<Self>,
    ) {
        let Some(slot) = self.lsp.inlay_hints.slots.get_mut(&id) else {
            return;
        };
        if slot.closing_key != Some(key) {
            return;
        }
        let view = slot.editor.read(cx);
        let snapshot = view.model().snapshot();
        if snapshot.version() != key.0 || outline.version != key.0 {
            // Parsed at another version: asked for again at the next frame.
            slot.closing_key = None;
            return;
        }
        let hints = crate::closing_brace_hints::hints(&outline, snapshot.text(), key.2);
        // Nothing to show, and nothing shown: the editor is left alone.
        let unchanged =
            hints.is_empty() && slot.closing.as_ref().is_none_or(|(_, old)| old.is_empty());
        slot.closing = Some((key.0, hints));
        if !unchanged {
            publish(slot, cx);
        }
    }

    /// Asks again, at once, for every editor whose buffer is on `server`.
    pub(super) fn refresh_inlay_hints(&mut self, server: ServerId, cx: &mut Context<Self>) {
        for id in self.inlay_hint_slots_of(server) {
            if let Some(slot) = self.lsp.inlay_hints.slots.get_mut(&id) {
                slot.no_provider = None;
            }
            self.ask_inlay_hints_in(id, Duration::ZERO, cx);
        }
    }

    /// The editors whose hints are asked of `server`.
    fn inlay_hint_slots_of(&self, server: ServerId) -> Vec<EntityId> {
        (self.lsp.inlay_hints.slots.iter())
            .filter(|(_, slot)| slot.asked_of.iter().any(|(of, ..)| *of == server))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Asks the hints of every editor whose buffer is on `server` again,
    /// keeping those shown, for `why` (see [`Reask`]).
    pub(super) fn reask_inlay_hints_of(
        &mut self,
        server: ServerId,
        why: Reask,
        cx: &mut Context<Self>,
    ) {
        for id in self.inlay_hint_slots_of(server) {
            self.reask_inlay_hints(id, why, cx);
        }
    }

    /// Asks editor `id`'s hints again for `why`, if it has asked before:
    /// a poll only while no request of its own is in flight.
    fn reask_inlay_hints(&mut self, id: EntityId, why: Reask, cx: &mut Context<Self>) {
        let Some(slot) = self.lsp.inlay_hints.slots.get(&id) else {
            return;
        };
        if slot.key.is_none() || slot.no_provider.as_ref() == Some(&slot.asked_of) {
            return;
        }
        match why {
            Reask::Notified(rest) => self.ask_inlay_hints_in(id, rest, cx),
            Reask::Poll if !slot.in_flight.busy() => {
                self.ask_inlay_hints_in(id, Duration::ZERO, cx)
            }
            Reask::Poll => {}
        }
    }

    /// Asks again each editor's hints of a server whose poll in `dues`
    /// came since the editor last saw one.
    pub(super) fn repoll_inlay_hints(
        &mut self,
        dues: &HashMap<ServerId, Instant>,
        cx: &mut Context<Self>,
    ) {
        let mut due = Vec::new();
        for (id, slot) in &mut self.lsp.inlay_hints.slots {
            let servers: Vec<ServerId> = slot.asked_of.iter().map(|(server, ..)| *server).collect();
            let mut came = false;
            for server in servers {
                came |= super::poll_came(&mut slot.polled, server, dues);
            }
            if came {
                due.push(*id);
            }
        }
        for id in due {
            self.reask_inlay_hints(id, Reask::Poll, cx);
        }
    }

    /// Has editor `id` ask for its hints after `delay`, in place of any
    /// request waiting or in flight.
    fn ask_inlay_hints_in(&mut self, id: EntityId, delay: Duration, cx: &mut Context<Self>) {
        let Some(asking) =
            (self.lsp.inlay_hints.slots.get(&id)).map(|slot| slot.in_flight.asking())
        else {
            return;
        };
        let task = cx.spawn(async move |workbench, cx| {
            let _asking = asking;
            if !delay.is_zero() {
                cx.background_executor().timer(delay).await;
            }
            let Ok(Some((version, range, reply))) =
                workbench.update(cx, |workbench, cx| workbench.send_inlay_hints(id, cx))
            else {
                return;
            };
            let answer = reply.await;
            workbench
                .update(cx, |workbench, cx| {
                    workbench.inlay_hints_landed(id, version, range, answer, cx)
                })
                .ok();
        });
        if let Some(slot) = self.lsp.inlay_hints.slots.get_mut(&id) {
            slot.task = Some(task);
        }
    }

    /// The request for editor `id`'s rows in view now, sent.
    #[allow(clippy::type_complexity)]
    fn send_inlay_hints(
        &mut self,
        id: EntityId,
        cx: &mut Context<Self>,
    ) -> Option<(
        Version,
        Range<usize>,
        std::pin::Pin<Box<dyn std::future::Future<Output = Answer> + Send>>,
    )> {
        let manager = self.lsp.manager.clone()?;
        let slot = self.lsp.inlay_hints.slots.get_mut(&id)?;
        let view = slot.editor.read(cx);
        let (top, rows) = view.viewport_rows()?;
        let model = view.model();
        let snapshot = model.snapshot();
        let (buffer, version) = (model.buffer().id(), snapshot.version());
        // The rows in view and a viewport more each way.
        let text = snapshot.text();
        let offset = |row: u32| {
            if row >= text.row_count() {
                text.len()
            } else {
                text.row_start(row)
            }
        };
        let range =
            offset(top.saturating_sub(rows))..offset(top.saturating_add(rows.saturating_mul(2)));
        slot.asked = Some((version, range.clone()));
        let reply = super::lock(&manager).inlay_hints(buffer, version, range.clone());
        Some((version, range, Box::pin(reply)))
    }

    /// Shows `answer`, the reply to the request for `range` at `version`
    /// of editor `id`, if it is still the latest.
    fn inlay_hints_landed(
        &mut self,
        id: EntityId,
        version: Version,
        range: Range<usize>,
        answer: Answer,
        cx: &mut Context<Self>,
    ) {
        let Some(slot) = self.lsp.inlay_hints.slots.get_mut(&id) else {
            return;
        };
        if slot.asked.as_ref() != Some(&(version, range)) || !self.lsp.inlay_hints_on.get() {
            return;
        }
        match answer {
            Ok(Some(found)) => {
                // Asked again with nothing changed (a poll, say): nothing to
                // show anew.
                let same = (slot.served.as_ref())
                    .is_some_and(|(at, shown)| *at == version && same_hints(shown, &found));
                if same {
                    return;
                }
                slot.served = Some((version, found.into_iter().map(convert).collect()));
                publish(slot, cx);
            }
            Ok(None) => {
                slot.no_provider = Some(slot.asked_of.clone());
                slot.served = None;
                publish(slot, cx);
            }
            Err(error) => log::debug!("no inlay hints: {error}"),
        }
    }
}

/// Shows the editor's server hints and closing-brace hints together: at
/// the server's version, with the closing hints if they're of it too; a
/// closing hint a server hint duplicates is left out.
fn publish(slot: &Slot, cx: &mut Context<Workbench>) {
    let mut shown: Option<(Version, Vec<InlayHint>)> = slot.served.clone();
    if let Some((version, closing)) = &slot.closing {
        match &mut shown {
            None => shown = Some((*version, closing.clone())),
            Some((served_version, hints)) if served_version == version => {
                let extra = crate::closing_brace_hints::without_served(closing, hints);
                hints.extend(extra);
            }
            Some(_) => {}
        }
    }
    slot.editor.update(cx, |view, cx| {
        match shown {
            Some((version, hints)) => view.set_inlay_hints(version, hints),
            None => view.clear_inlay_hints(),
        }
        cx.notify();
    });
}

/// Resolves an editor's hints through the language servers.
struct Host {
    manager: Weak<Mutex<LspManager>>,
}

impl InlayHintHost for Host {
    fn resolve(&self, hint: InlayHint) -> InlayHintFuture {
        let manager = self.manager.upgrade();
        Box::pin(async move {
            let Some(manager) = manager else { return hint };
            let Some(listed) = (hint.data.as_ref())
                .and_then(|data| data.downcast_ref::<fe_lsp::InlayHint>())
                .cloned()
            else {
                return hint;
            };
            // Sent with the lock held, and awaited without it.
            let reply = super::lock(&manager).resolve_inlay_hint(listed);
            drop(manager);
            match reply.await {
                Ok(resolved) => InlayHint {
                    offset: hint.offset,
                    ..convert(resolved)
                },
                Err(error) => {
                    log::debug!("inlay hint not resolved: {error}");
                    hint
                }
            }
        })
    }
}

/// Whether `shown`, hints as editors draw them, are `found`, as servers
/// answered them.
fn same_hints(shown: &[InlayHint], found: &[fe_lsp::InlayHint]) -> bool {
    shown.len() == found.len()
        && shown.iter().zip(found).all(|(shown, found)| {
            (shown.data.as_ref())
                .and_then(|data| data.downcast_ref::<fe_lsp::InlayHint>())
                .is_some_and(|hint| hint == found)
        })
}

/// `hint` as an editor draws it, keeping the server's in `data`.
fn convert(hint: fe_lsp::InlayHint) -> InlayHint {
    InlayHint {
        offset: hint.offset,
        label: hint.label.clone(),
        kind: hint.kind.map(|kind| match kind {
            fe_lsp::InlayHintKind::Type => InlayHintKind::Type,
            fe_lsp::InlayHintKind::Parameter => InlayHintKind::Parameter,
        }),
        padding_left: hint.padding_left,
        padding_right: hint.padding_right,
        tooltip: hint.tooltip.clone(),
        edits: hint.edits.clone(),
        data: Some(Rc::new(hint)),
    }
}
