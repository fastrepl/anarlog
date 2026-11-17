use desktop_gpui::workspace::navigation::Route;
use desktop_runtime::SessionId;
use gpui::Role;

use crate::Harness;

#[test]
fn note_editor_and_title_expose_their_text_and_caret() {
    let app = Harness::start();
    app.open(Route::Session(SessionId("s-sync".into())));
    let tree = app.tree();

    let title = tree.root().items(Role::TextInput)[0];
    assert_eq!(title.value(), Some("Weekly product sync"));
    assert_eq!(title.raw().placeholder(), Some("Untitled"));

    let editor = tree.root().items(Role::MultilineTextInput)[0];
    assert_eq!(editor.id, tree.focus, "{}", tree.outline());
    let text = editor.value().unwrap();
    assert!(text.starts_with("Agenda\nRelease status"), "{text}");
    let runs = editor.items(Role::TextRun);
    assert_eq!(runs[0].value(), Some("Agenda\n"));
    // Non-ASCII text keeps one character per grapheme, as VoiceOver moves through it.
    let korean = runs
        .iter()
        .find(|run| run.value().is_some_and(|value| value.contains("한글")))
        .unwrap();
    let characters = korean.raw().character_lengths().len();
    assert_eq!(characters, korean.value().unwrap().chars().count());
    let selection = editor.raw().text_selection().expect("the caret");
    assert!(runs.iter().any(|run| run.id == selection.focus.node));
}
