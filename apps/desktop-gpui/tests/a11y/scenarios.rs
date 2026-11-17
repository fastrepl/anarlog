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

#[test]
fn sidebar_names_its_controls_and_lists_the_timeline() {
    let app = Harness::start();
    let tree = app.tree();
    for name in ["Hide sidebar", "Search", "New note", "Sort notes"] {
        assert!(tree.find(Role::Button, name).clickable(), "{name}");
    }
    let timeline = tree.find(Role::List, "Timeline");
    let rows = timeline.items(Role::Button);
    assert!(
        rows.iter()
            .any(|row| row.label().contains("Weekly product sync")),
        "{}",
        timeline.outline()
    );
    assert!(timeline.items(Role::Heading).len() > 1);

    // Opening a note from its row marks the row selected and the note's title appears.
    let row = rows
        .iter()
        .find(|row| row.label().contains("Weekly product sync"))
        .unwrap()
        .id;
    app.press(row);
    let tree = app.tree();
    let timeline = tree.find(Role::List, "Timeline");
    let row = timeline
        .items(Role::Button)
        .into_iter()
        .find(|row| row.label().contains("Weekly product sync"))
        .unwrap();
    assert_eq!(row.selected(), Some(true));
    assert_eq!(
        tree.root().items(Role::TextInput)[0].value(),
        Some("Weekly product sync")
    );
}

#[test]
fn sort_menu_is_a_menu_with_checked_choices() {
    let app = Harness::start();
    let sort = app.tree().find(Role::Button, "Sort notes").id;
    app.press(sort);
    let tree = app.tree();
    assert_eq!(tree.find(Role::Button, "Sort notes").expanded(), Some(true));
    let menu = tree.find(Role::Menu, "Sort notes");
    let grouping = menu.find(Role::MenuItem, "Grouping, Date").id;
    menu.find(Role::MenuItem, "Ordering, Newest");
    app.press(grouping);
    let tree = app.tree();
    let submenu = tree.find(Role::Menu, "Grouping");
    assert_eq!(
        submenu.find(Role::MenuItemRadio, "Date").toggled(),
        Some(gpui::Toggled::True)
    );
    let folder = submenu.find(Role::MenuItemRadio, "Folder").id;
    app.press(folder);
    let tree = app.tree();
    assert!(tree.try_find(Role::Menu, "Sort notes").is_none());
    app.press(tree.find(Role::Button, "Sort notes").id);
    app.tree().find(Role::MenuItem, "Grouping, Folder");
}

#[test]
fn note_header_has_view_tabs_folder_and_a_more_menu() {
    let app = Harness::start();
    app.open(Route::Session(SessionId("s-critique".into())));
    let tree = app.tree();
    let views = tree.find(Role::TabList, "Session note views");
    let tabs = views.items(Role::Tab);
    assert!(tabs.len() > 1, "{}", views.outline());
    assert_eq!(
        tabs.iter()
            .filter(|tab| tab.selected() == Some(true))
            .count(),
        1
    );
    let memos = views.find(Role::Tab, "Memos").id;
    app.press(memos);
    let tree = app.tree();
    assert_eq!(
        tree.find(Role::TabList, "Session note views")
            .find(Role::Tab, "Memos")
            .selected(),
        Some(true)
    );
    assert!(
        tree.root()
            .items(Role::ComboBox)
            .iter()
            .any(|combo| combo.label().starts_with("Folder: ") || combo.label() == "Select folder")
    );

    app.press(tree.find(Role::Button, "More").id);
    let tree = app.tree();
    assert_eq!(tree.find(Role::Button, "More").expanded(), Some(true));
    let menu = tree.find(Role::Menu, "More");
    assert_eq!(menu.id, tree.focus, "the open menu takes focus");
    for item in ["Meeting info", "Version history", "Export", "Delete"] {
        menu.find(Role::MenuItem, item);
    }
    menu.find(Role::MenuItemCheckBox, "Always on Top");
    // Arrow keys move the highlight, and VoiceOver follows it as the active descendant.
    app.keys(&["down"]);
    let tree = app.tree();
    assert_eq!(tree.focused().role(), Role::MenuItem, "{}", tree.outline());
    assert_eq!(tree.focused().label(), "Meeting info");
}

#[test]
fn note_tasks_are_checkboxes_and_slash_commands_a_listbox() {
    let app = Harness::start();
    app.open(Route::Session(SessionId("s-sync".into())));
    let tree = app.tree();
    let open = tree.find(Role::CheckBox, "Draft the pricing FAQ");
    assert_eq!(open.toggled(), Some(gpui::Toggled::False));
    app.press(open.id);
    assert_eq!(
        app.tree()
            .find(Role::CheckBox, "Draft the pricing FAQ")
            .toggled(),
        Some(gpui::Toggled::True)
    );

    app.keys(&["end", "enter", "/"]);
    let tree = app.tree();
    let commands = tree.find(Role::ListBox, "Commands");
    let options = commands.items(Role::ListBoxOption);
    assert!(options.len() > 3, "{}", commands.outline());
    assert_eq!(
        options
            .iter()
            .filter(|option| option.selected() == Some(true))
            .count(),
        1
    );
}
