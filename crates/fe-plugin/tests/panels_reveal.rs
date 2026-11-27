//! `fastedit:editor/panels.reveal`: a plugin may ask the host to show and focus one of its own
//! panels, and only while a command the user ran is being handled. The host takes a ticket for
//! the user's run ([`PanelViews::user_run`]), hands its ticket with the `run-command` call of
//! that command and those arguments, whose store opens it ([`State::begin_command`]), and reads
//! the panel asked for once the run is over ([`PanelViews::end_user_run`]).

use fe_plugin::HostState;
use fe_plugin::fastedit::editor::panels::Host as _;
use fe_plugin::panels::{PanelViews, State};
use kiln_command::capability::{Capability, Grants};
use kiln_plugin::bindings::Error;

const PLUGIN: &str = "acme";
const PLAY: &str = "acme.play";

fn grants(granted: &[&str]) -> Grants {
    (granted.iter())
        .map(|capability| Capability::parse(capability).unwrap())
        .collect()
}

/// Plugin `plugin`'s store, holding `granted`, with the panels `declared`, on `views`.
fn store(plugin: &str, granted: &[&str], declared: &[&str], views: &PanelViews) -> HostState {
    let declared: Vec<String> = declared.iter().map(|&id| id.to_owned()).collect();
    HostState {
        panels: State::new(plugin, &grants(granted), &declared, views),
        ..HostState::default()
    }
}

fn reveal(store: &mut HostState, panel: &str) -> Result<(), Error> {
    store.reveal(panel.to_owned())
}

#[test]
fn reveal_needs_the_panels_capability() {
    let views = PanelViews::new();
    let mut acme = store(PLUGIN, &["commands"], &["board"], &views);
    let ticket = views.user_run(PLUGIN, PLAY, "{}");
    acme.panels.begin_command(Some(ticket.ticket()), PLAY, "{}");
    assert!(matches!(reveal(&mut acme, "board"), Err(Error::Denied(_))));
    acme.panels.end_command();
    assert_eq!(views.end_user_run(ticket), None);
    // A store with no panels slice at all has no reveal.
    let mut bare = HostState::default();
    assert!(matches!(
        reveal(&mut bare, "board"),
        Err(Error::Unimplemented(_))
    ));
}

#[test]
fn reveal_in_a_command_the_user_ran_shows_the_panel_once_it_is_over() {
    let views = PanelViews::new();
    let mut acme = store(PLUGIN, &["panels"], &["board", "scores"], &views);
    let ticket = views.user_run(PLUGIN, PLAY, "{}");
    acme.panels.begin_command(Some(ticket.ticket()), PLAY, "{}");
    reveal(&mut acme, "scores").unwrap();
    // Asked twice, the last wins.
    reveal(&mut acme, "board").unwrap();
    acme.panels.end_command();
    assert_eq!(views.end_user_run(ticket), Some("board".to_owned()));

    // A run that asks nothing reveals nothing.
    let ticket = views.user_run(PLUGIN, PLAY, "{}");
    acme.panels.begin_command(Some(ticket.ticket()), PLAY, "{}");
    acme.panels.end_command();
    assert_eq!(views.end_user_run(ticket), None);
}

#[test]
fn reveal_outside_a_command_the_user_ran_is_denied() {
    let views = PanelViews::new();
    let mut acme = store(PLUGIN, &["panels"], &["board"], &views);
    // No command at all: an event, a tick, `init`.
    assert!(matches!(reveal(&mut acme, "board"), Err(Error::Denied(_))));
    // A command the host runs on its own, as a formatter's: no ticket for it.
    acme.panels
        .begin_command(None, "acme.format", "{\"request\":1}");
    assert!(matches!(reveal(&mut acme, "board"), Err(Error::Denied(_))));
    acme.panels.end_command();

    // The user ran the command, but this call is another one: handed no ticket...
    let ticket = views.user_run(PLUGIN, PLAY, "{}");
    acme.panels.begin_command(None, PLAY, "{}");
    assert!(matches!(reveal(&mut acme, "board"), Err(Error::Denied(_))));
    acme.panels.end_command();
    // ...other arguments...
    acme.panels
        .begin_command(Some(ticket.ticket()), PLAY, "{\"request\":2}");
    assert!(matches!(reveal(&mut acme, "board"), Err(Error::Denied(_))));
    acme.panels.end_command();
    // ...or the user's call, then a second of the same command: a ticket opens one call.
    acme.panels.begin_command(Some(ticket.ticket()), PLAY, "{}");
    reveal(&mut acme, "board").unwrap();
    acme.panels.end_command();
    acme.panels.begin_command(Some(ticket.ticket()), PLAY, "{}");
    assert!(matches!(reveal(&mut acme, "board"), Err(Error::Denied(_))));
    acme.panels.end_command();
    // After its call, the run can't be added to.
    assert!(matches!(reveal(&mut acme, "board"), Err(Error::Denied(_))));
    assert_eq!(views.end_user_run(ticket), Some("board".to_owned()));

    // A ticket handed back before its call began reveals nothing, and opens nothing later.
    let ticket = views.user_run(PLUGIN, PLAY, "{}");
    let handed = ticket.ticket();
    assert_eq!(views.end_user_run(ticket), None);
    acme.panels.begin_command(Some(handed), PLAY, "{}");
    assert!(matches!(reveal(&mut acme, "board"), Err(Error::Denied(_))));
    acme.panels.end_command();

    // A job's worker store shares the load but is never the user's run.
    let ticket = views.user_run(PLUGIN, PLAY, "{}");
    let mut worker = HostState {
        panels: State::worker(&acme.panels.shared().unwrap()),
        ..HostState::default()
    };
    assert!(matches!(
        reveal(&mut worker, "board"),
        Err(Error::Denied(_))
    ));
    assert_eq!(views.end_user_run(ticket), None);
}

#[test]
fn reveal_of_a_panel_the_plugin_does_not_declare_is_not_found() {
    let views = PanelViews::new();
    let mut acme = store(PLUGIN, &["panels"], &["board"], &views);
    let mut other = store("other", &["panels"], &["list"], &views);
    let ticket = views.user_run(PLUGIN, PLAY, "{}");
    acme.panels.begin_command(Some(ticket.ticket()), PLAY, "{}");
    // Undeclared, or another plugin's, by its id or by its dock id.
    for panel in ["nope", "list", "other.list", "acme.board"] {
        assert!(
            matches!(reveal(&mut acme, panel), Err(Error::NotFound(_))),
            "{panel}"
        );
    }
    acme.panels.end_command();
    assert_eq!(views.end_user_run(ticket), None);
    // Outside a command too, an undeclared panel is not found rather than denied.
    assert!(matches!(reveal(&mut acme, "list"), Err(Error::NotFound(_))));

    // One plugin's ticket is no other's, even for a command of the same name.
    let ticket = views.user_run(PLUGIN, PLAY, "{}");
    other
        .panels
        .begin_command(Some(ticket.ticket()), PLAY, "{}");
    assert!(matches!(reveal(&mut other, "list"), Err(Error::Denied(_))));
    other.panels.end_command();
    // Still acme's to use.
    acme.panels.begin_command(Some(ticket.ticket()), PLAY, "{}");
    reveal(&mut acme, "board").unwrap();
    acme.panels.end_command();
    assert_eq!(views.end_user_run(ticket), Some("board".to_owned()));
}

#[test]
fn reveal_from_an_unloaded_load_is_denied() {
    let views = PanelViews::new();
    let mut old = store(PLUGIN, &["panels"], &["board"], &views);
    let ticket = views.user_run(PLUGIN, PLAY, "{}");
    old.panels.begin_command(Some(ticket.ticket()), PLAY, "{}");
    // The plugin reloads meanwhile: the old load's store asks for nothing.
    let _new = store(PLUGIN, &["panels"], &["board"], &views);
    assert!(matches!(reveal(&mut old, "board"), Err(Error::Denied(_))));
    old.panels.end_command();
    assert_eq!(views.end_user_run(ticket), None);
}
