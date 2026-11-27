//! `fastedit:editor/panels.reveal` tickets: each of the user's runs opens the one call the host
//! hands its ticket with, whatever order the runs' calls reach the plugin in, and belongs to the
//! load of the plugin that took it. Unloading or reloading the plugin forgets its runs, those
//! that asked a reveal included.

use fe_plugin::HostState;
use fe_plugin::fastedit::editor::panels::Host as _;
use fe_plugin::panels::{PanelViews, RunTicket, State};
use kiln_command::capability::{Capability, Grants};
use kiln_plugin::bindings::Error;

const PLUGIN: &str = "acme";
const PLAY: &str = "acme.play";

/// Plugin `PLUGIN`'s store, a new load of it, with the panels `board` and `scores`.
fn load(views: &PanelViews) -> HostState {
    let grants: Grants = [Capability::parse("panels").unwrap()].into_iter().collect();
    let declared = ["board".to_owned(), "scores".to_owned()];
    HostState {
        panels: State::new(PLUGIN, &grants, &declared, views),
        ..HostState::default()
    }
}

/// The `run-command` call of `PLAY` handed `ticket`, which asks to reveal `panel`: what the
/// plugin is answered.
fn call(store: &mut HostState, ticket: Option<RunTicket>, panel: &str) -> Result<(), Error> {
    store.panels.begin_command(ticket, PLAY, "{}");
    let revealed = store.reveal(panel.to_owned());
    store.panels.end_command();
    revealed
}

#[test]
fn identical_runs_reveal_their_own_panels_in_either_order() {
    let views = PanelViews::new();
    let mut acme = load(&views);
    // Blocks: Play run in one window, then in another, before either call: the host holds a
    // ticket for each window.
    let first = views.user_run(PLUGIN, PLAY, "{}");
    let second = views.user_run(PLUGIN, PLAY, "{}");
    // The second window's call reaches the plugin first, and is over first.
    call(&mut acme, Some(second.ticket()), "scores").unwrap();
    assert_eq!(views.end_user_run(second), Some("scores".to_owned()));
    // The first window's call still has its own ticket.
    call(&mut acme, Some(first.ticket()), "board").unwrap();
    assert_eq!(views.end_user_run(first), Some("board".to_owned()));

    // Both calls made before either run is over, the later one first.
    let first = views.user_run(PLUGIN, PLAY, "{}");
    let second = views.user_run(PLUGIN, PLAY, "{}");
    call(&mut acme, Some(second.ticket()), "board").unwrap();
    call(&mut acme, Some(first.ticket()), "scores").unwrap();
    assert_eq!(views.end_user_run(first), Some("scores".to_owned()));
    assert_eq!(views.end_user_run(second), Some("board".to_owned()));

    // A run whose call asked nothing takes nothing of the other's.
    let first = views.user_run(PLUGIN, PLAY, "{}");
    let second = views.user_run(PLUGIN, PLAY, "{}");
    acme.panels.begin_command(Some(second.ticket()), PLAY, "{}");
    acme.panels.end_command();
    assert_eq!(views.end_user_run(second), None);
    call(&mut acme, Some(first.ticket()), "board").unwrap();
    assert_eq!(views.end_user_run(first), Some("board".to_owned()));
}

#[test]
fn a_reveal_asked_before_an_unload_reveals_nothing() {
    let views = PanelViews::new();
    let mut acme = load(&views);
    let ticket = views.user_run(PLUGIN, PLAY, "{}");
    call(&mut acme, Some(ticket.ticket()), "board").unwrap();
    // Unloaded before the host reads the run.
    views.unload(PLUGIN);
    assert_eq!(views.end_user_run(ticket), None);
}

#[test]
fn a_reveal_asked_before_a_reload_reveals_nothing_of_the_new_load() {
    let views = PanelViews::new();
    let mut old = load(&views);
    // Asked and over, then the plugin reloads before the host reads the run.
    let ticket = views.user_run(PLUGIN, PLAY, "{}");
    call(&mut old, Some(ticket.ticket()), "board").unwrap();
    let mut new = load(&views);
    assert_eq!(views.end_user_run(ticket), None);

    // Asked, and the plugin reloads while the call is under way.
    let ticket = views.user_run(PLUGIN, PLAY, "{}");
    new.panels.begin_command(Some(ticket.ticket()), PLAY, "{}");
    new.reveal("board".to_owned()).unwrap();
    let _newer = load(&views);
    new.panels.end_command();
    assert_eq!(views.end_user_run(ticket), None);
}

#[test]
fn a_ticket_taken_before_a_reload_opens_no_call_of_the_new_load() {
    let views = PanelViews::new();
    let _old = load(&views);
    let ticket = views.user_run(PLUGIN, PLAY, "{}");
    let mut new = load(&views);
    assert!(matches!(
        call(&mut new, Some(ticket.ticket()), "board"),
        Err(Error::Denied(_))
    ));
    assert_eq!(views.end_user_run(ticket), None);

    // Nor of a load attached after an unload.
    let ticket = views.user_run(PLUGIN, PLAY, "{}");
    views.unload(PLUGIN);
    let mut again = load(&views);
    assert!(matches!(
        call(&mut again, Some(ticket.ticket()), "board"),
        Err(Error::Denied(_))
    ));
    assert_eq!(views.end_user_run(ticket), None);

    // A ticket the new load's run takes is its own.
    let ticket = views.user_run(PLUGIN, PLAY, "{}");
    call(&mut again, Some(ticket.ticket()), "board").unwrap();
    assert_eq!(views.end_user_run(ticket), Some("board".to_owned()));
}
