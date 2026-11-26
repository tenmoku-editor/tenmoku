//! A server's `window/showMessage` draws in the tone of its level: an Info
//! message isn't an error.

use gpui::{AppContext as _, TestAppContext};
use kiln_shell::StatusTone;
use lsp_types::{MessageType, ShowMessageParams, notification::ShowMessage};

use crate::tests::lsp::Lsp;

/// The tone of the language server's status item.
fn tone(s: &mut Lsp) -> StatusTone {
    let bar = s.workbench.read_with(&s.cx, |w, _| w.status_bar().clone());
    let items = s.cx.read_entity(&bar, |bar, cx| bar.items(cx));
    items
        .iter()
        .find(|item| item.id == crate::lsp::ITEM)
        .expect("the server's item")
        .tone
}

#[gpui::test]
fn info_message_shows_in_the_default_tone(cx: &mut TestAppContext) {
    let mut s = Lsp::new(cx);
    s.open("src/main.rs");
    s.fake.wait_initialized();
    for (typ, message, expected) in [
        (MessageType::INFO, "Loaded 3 crates", StatusTone::Default),
        (MessageType::WARNING, "Careful now", StatusTone::Warning),
        (MessageType::ERROR, "It broke", StatusTone::Error),
    ] {
        s.fake.notify::<ShowMessage>(ShowMessageParams {
            typ,
            message: message.into(),
        });
        s.wait_item(Some(message));
        assert_eq!(tone(&mut s), expected, "{message}");
    }
}
