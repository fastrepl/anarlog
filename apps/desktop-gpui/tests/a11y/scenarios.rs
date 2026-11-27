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

#[test]
fn chat_panel_names_its_toolbar_history_and_composer() {
    use desktop_gpui::chat::state::ChatEvent;

    let app = Harness::start();
    app.chat(|chat, window, cx| chat.event(ChatEvent::Open, window, cx));
    let tree = app.tree();
    let chat = tree.find(Role::Complementary, "Chat");
    for name in ["New chat", "Open in right panel", "Chat history"] {
        chat.find(Role::Button, name);
    }
    let composer = chat.find(Role::MultilineTextInput, "Chat draft");
    assert_eq!(composer.raw().placeholder(), Some("Ask anything"));
    chat.find(Role::Button, "Start voice input");

    app.chat(|chat, _, cx| chat.toggle_history(cx));
    let tree = app.tree();
    assert_eq!(
        tree.find(Role::Button, "Chat history").expanded(),
        Some(true)
    );
    let history = tree.find(Role::Menu, "Recent chats");
    assert!(
        !history.items(Role::MenuItemCheckBox).is_empty(),
        "{}",
        history.outline()
    );

    app.chat(|chat, _, cx| chat.select_group("g-launch".into(), cx));
    let tree = app.tree();
    let messages = tree.find(Role::List, "Messages");
    let items = messages.items(Role::ListItem);
    assert!(items.iter().any(|item| item.label() == "You"));
    assert!(items.iter().any(|item| {
        item.label() == "Anarlog AI"
            && item
                .raw()
                .description()
                .is_some_and(|text| !text.is_empty())
    }));
}

#[test]
fn settings_navigation_and_controls_are_named() {
    let app = Harness::start();
    app.open(Route::Settings("app".into()));
    let tree = app.tree();
    let nav = tree.find(Role::Navigation, "Settings");
    let rows = nav.items(Role::Button);
    assert!(rows.len() > 10, "{}", nav.outline());
    assert_eq!(
        rows.iter()
            .filter(|row| row.selected() == Some(true))
            .count(),
        1
    );
    tree.find(Role::TextInput, "Search settings");
    tree.find(Role::Heading, "General");
    // Every switch is named and reports its state.
    let switches = tree.root().items(Role::Switch);
    assert!(!switches.is_empty(), "{}", tree.outline());
    for switch in &switches {
        assert!(!switch.label().is_empty(), "{}", switch.line());
        assert!(switch.toggled().is_some());
    }
    let language = tree.find(Role::Group, "Main language");
    let select = language.items(Role::ComboBox)[0];
    assert_eq!(select.expanded(), Some(false));
    assert!(select.value().is_some());
    app.press(select.id);
    let tree = app.tree();
    let options = tree.root().items(Role::ListBoxOption);
    assert!(options.len() > 1, "{}", tree.outline());
    assert_eq!(
        options
            .iter()
            .filter(|option| option.selected() == Some(true))
            .count(),
        1
    );
}

/// Controls a screen reader can land on must say what they are.
fn assert_named(tree: &crate::tree::Tree, place: &str) {
    let controls = [
        Role::Button,
        Role::Link,
        Role::CheckBox,
        Role::Switch,
        Role::RadioButton,
        Role::Tab,
        Role::MenuItem,
        Role::MenuItemCheckBox,
        Role::MenuItemRadio,
        Role::ListBoxOption,
        Role::ComboBox,
        Role::Dialog,
        Role::Heading,
    ];
    let unnamed = tree
        .all()
        .into_iter()
        .filter(|node| controls.contains(&node.role()) && node.label().trim().is_empty())
        .map(|node| node.line())
        .collect::<Vec<_>>();
    assert!(unnamed.is_empty(), "{place}: unnamed {unnamed:#?}");
}

#[test]
fn every_page_names_its_controls() {
    let app = Harness::start();
    assert_named(&app.tree(), "main");
    for route in [
        Route::Session(SessionId("s-sync".into())),
        Route::Session(SessionId("s-critique".into())),
        Route::Calendar,
        Route::Templates,
        Route::Folders,
        Route::Automations,
        Route::Contacts,
    ] {
        let place = format!("{route:?}");
        app.open(route);
        assert_named(&app.tree(), &place);
    }
    for page in [
        "app",
        "account",
        "billing",
        "referrals",
        "insights",
        "team",
        "sync",
        "appearance",
        "notifications",
        "transcription",
        "dictation",
        "intelligence",
        "dictionary",
        "meetings",
        "imports",
        "crm",
        "privacy",
        "permissions",
        "developers",
    ] {
        app.open(Route::Settings(page.into()));
        assert_named(&app.tree(), page);
    }
}

#[test]
fn onboarding_steps_have_headings_and_named_actions() {
    use desktop_gpui::product::onboarding::{Access, OnboardingPreview, State, Step};

    let app = Harness::start();
    let preview = |step, state: State| {
        app.app.update(|cx| {
            cx.set_global(OnboardingPreview {
                step: Some(step),
                state,
            })
        });
    };
    let granted = State {
        microphone: Access::Authorized,
        system_audio: Access::Authorized,
        accessibility: Access::Authorized,
        enabled_calendars: 1,
        skipped_login: true,
        ..State::default()
    };
    // Like shipping, only macOS asks for permissions during onboarding.
    if cfg!(target_os = "macos") {
        preview(Step::Permissions, State::default());
        app.open(Route::Onboarding);
        let tree = app.tree();
        tree.find(Role::Heading, "Start with permissions");
        assert!(
            !tree.root().items(Role::Button).is_empty(),
            "{}",
            tree.outline()
        );
        assert_named(&tree, "permissions");
    } else {
        preview(Step::Login, granted.clone());
        app.open(Route::Onboarding);
    }
    for step in [Step::Login, Step::Calendar, Step::Imports, Step::Final] {
        preview(step, granted.clone());
        app.settle();
        let tree = app.tree();
        assert_named(&tree, &format!("{step:?}"));
        assert!(!tree.root().items(Role::Heading).is_empty(), "{step:?}");
    }
    app.tree().find(Role::Button, "Open Anarlog");
    // A freshly connected account's calendars start disabled; onboarding lists them to enable.
    preview(
        Step::Calendar,
        State {
            enabled_calendars: 0,
            calendars: vec![desktop_gpui::workspace::calendar_sidebar::CalendarRow {
                id: "work".into(),
                name: "Work".into(),
                enabled: false,
                provider: "google".into(),
                source: "ada@example.com".into(),
                color: "".into(),
                connection: "google-1".into(),
            }],
            google_connected: true,
            ..granted.clone()
        },
    );
    app.settle();
    app.tree().find(Role::CheckBox, "Work");
}

#[test]
fn calendar_names_its_navigation_days_and_events() {
    let app = Harness::start();
    app.open(Route::Calendar);
    let tree = app.tree();
    for name in ["Previous", "Today", "Next"] {
        tree.find(Role::Button, name);
    }
    assert!(!tree.root().items(Role::Heading).is_empty());
    let days = tree
        .root()
        .items(Role::Group)
        .into_iter()
        .filter(|group| group.label().contains(", "))
        .collect::<Vec<_>>();
    assert!(days.len() >= 28, "{}", tree.outline());
    assert!(days.iter().any(|day| day.label().starts_with("Today, ")));
    let event = days
        .iter()
        .flat_map(|day| day.items(Role::Button))
        .next()
        .expect("a seeded event");
    let label = event.label().to_owned();
    app.press(event.id);
    let tree = app.tree();
    let chip = tree.find(Role::Button, &label);
    assert_eq!(chip.expanded(), Some(true));
    assert!(
        !tree.root().items(Role::Dialog).is_empty(),
        "{}",
        tree.outline()
    );
}

#[test]
fn contacts_list_people_and_open_a_named_profile() {
    let app = Harness::start();
    app.open(Route::Contacts);
    let tree = app.tree();
    tree.find(Role::TextInput, "Search contacts");
    for name in ["Go home", "Add contact"] {
        tree.find(Role::Button, name);
    }
    let rows = tree
        .root()
        .items(Role::Button)
        .into_iter()
        .filter(|row| row.selected().is_some())
        .collect::<Vec<_>>();
    assert!(!rows.is_empty(), "{}", tree.outline());
    let pin_actions: Vec<_> = tree
        .root()
        .items(Role::Button)
        .into_iter()
        .filter(|button| button.label() == "Pin contact")
        .collect();
    assert!(!pin_actions.is_empty(), "{}", tree.outline());
    let unpin_before = tree
        .root()
        .items(Role::Button)
        .into_iter()
        .filter(|button| button.label() == "Unpin contact")
        .count();
    app.press(pin_actions[0].id);
    let after_pin = app.tree();
    let unpin_after = after_pin
        .root()
        .items(Role::Button)
        .into_iter()
        .filter(|button| button.label() == "Unpin contact")
        .count();
    assert_eq!(unpin_after, unpin_before + 1, "{}", after_pin.outline());

    let first = rows[0].id;
    app.press(first);
    let tree = app.tree();
    let selected = tree
        .root()
        .items(Role::Button)
        .into_iter()
        .filter(|row| row.selected() == Some(true))
        .count();
    assert_eq!(selected, 1, "{}", tree.outline());
    assert_named(&tree, "contact");
}

#[test]
fn open_note_dialog_is_a_dialog_whose_highlight_is_read() {
    let app = Harness::start();
    app.keys(&[if cfg!(target_os = "macos") {
        "cmd-k"
    } else {
        "ctrl-k"
    }]);
    let tree = app.tree();
    let dialog = tree.find(Role::Dialog, "Search notes and pages...");
    let input = dialog.find(Role::TextInput, "Search notes and pages");
    assert_eq!(input.id, tree.focus, "the search field takes focus");
    let results = dialog.find(Role::ListBox, "Notes and pages");
    assert!(!results.items(Role::ListBoxOption).is_empty());
    dialog.find(Role::Button, "Close");

    app.keys(&["down"]);
    let tree = app.tree();
    let highlighted = tree
        .root()
        .items(Role::ListBoxOption)
        .into_iter()
        .find(|option| option.selected() == Some(true))
        .expect("a highlighted option")
        .label()
        .to_owned();
    let status = tree
        .root()
        .items(Role::Status)
        .into_iter()
        .find(|status| highlighted.starts_with(status.label()))
        .is_some();
    assert!(status, "{}", tree.outline());
}

#[test]
fn transcript_segments_read_as_speaker_and_text() {
    let app = Harness::start();
    app.open(Route::Session(SessionId("s-critique".into())));
    let tab = app
        .tree()
        .find(Role::TabList, "Session note views")
        .find(Role::Tab, "Transcript")
        .id;
    app.press(tab);
    // The transcript loads on the runtime's own threads, which can take longer than one settle
    // on a busy machine.
    let mut tree = app.tree();
    for _ in 0..25 {
        if tree.try_find(Role::List, "Transcript").is_some() {
            break;
        }
        app.settle();
        tree = app.tree();
    }
    let transcript = tree.find(Role::List, "Transcript");
    let segments = transcript.items(Role::ListItem);
    assert!(!segments.is_empty(), "{}", transcript.outline());
    for segment in &segments {
        let (speaker, text) = segment.label().split_once(": ").expect("speaker: text");
        assert!(
            !speaker.is_empty() && !text.is_empty(),
            "{}",
            segment.line()
        );
    }
    assert_named(&tree, "transcript");
}

#[test]
fn a_standalone_note_window_locks_with_the_app() {
    use desktop_gpui::{application::lock, platform::device_auth, product::settings};

    struct Approve;

    impl device_auth::DeviceAuth for Approve {
        fn available(&self) -> bool {
            true
        }

        fn authenticate(&self, _: &str) -> futures::channel::oneshot::Receiver<bool> {
            let (sender, receiver) = futures::channel::oneshot::channel();
            let _ = sender.send(true);
            receiver
        }
    }

    let app = Harness::start();
    app.app.update(|cx| {
        cx.set_global(device_auth::Installed(std::sync::Arc::new(Approve)));
        lock::refresh_availability(cx);
    });
    let stored = futures::executor::block_on(async {
        let watch = settings::watch(&app.runtime)
            .unwrap()
            .receive()
            .await
            .unwrap();
        let snapshot = settings::decode(&watch.snapshots.borrow().rows).unwrap();
        watch.unsubscribe().await.unwrap();
        snapshot
    });
    let base = stored["lock_app"].clone();
    futures::executor::block_on(
        settings::save(&app.runtime, "lock_app".into(), base, true.into())
            .unwrap()
            .receive(),
    )
    .unwrap();
    // The launch prompt unlocks the app; then the note opens in its own window.
    app.settle();
    app.app.update(|cx| {
        app.window
            .update(cx, |root, _, cx| {
                root.workspace().clone().update(cx, |_, cx| {
                    cx.emit(desktop_gpui::workspace::WorkspaceAction::OpenNoteWindow(
                        SessionId("s-sync".into()),
                    ))
                })
            })
            .unwrap()
    });
    app.settle();
    let main: gpui::AnyWindowHandle = app.window.into();
    let note = app
        .app
        .update(|cx| cx.windows().into_iter().find(|window| *window != main))
        .expect("the note window");
    let title = |tree: &crate::tree::Tree| {
        tree.root()
            .items(Role::TextInput)
            .iter()
            .any(|input| input.value() == Some("Weekly product sync"))
    };
    assert!(
        title(&app.window_tree(note)),
        "{}",
        app.window_tree(note).outline()
    );

    // Closing the main window locks the app, and the note window with it.
    app.app.update(lock::main_hidden);
    app.settle();
    let tree = app.window_tree(note);
    tree.find(Role::Heading, "Anarlog is Locked");
    assert!(!title(&tree), "{}", tree.outline());
}

#[test]
fn notes_stay_read_only_while_a_slow_quit_drain_finishes() {
    let app = Harness::start();
    app.open(Route::Session(SessionId("s-sync".into())));
    let note = |app: &Harness| {
        app.tree()
            .root()
            .items(Role::MultilineTextInput)
            .first()
            .and_then(|editor| editor.value())
            .map(str::to_owned)
    };
    let before = note(&app).expect("the note editor");
    // A shutdown participant that outlasts the 12-second quit deadline.
    let (release, held) = futures::channel::oneshot::channel::<()>();
    futures::executor::block_on(
        app.runtime
            .register_flush(Box::new(move || {
                Box::pin(async move {
                    let _ = held.await;
                    Ok(())
                })
            }))
            .unwrap()
            .receive(),
    )
    .unwrap();
    app.app.update(|cx| {
        app.window
            .update(cx, |root, _, cx| root.request_quit(cx))
            .unwrap()
    });
    app.settle();
    app.platform
        .dispatcher
        .advance_clock(std::time::Duration::from_secs(13));
    app.settle();
    assert!(!app.platform.quit.get());

    app.keys(&["x"]);
    let after = note(&app);
    assert!(
        after.is_none_or(|text| text == before),
        "typing must not reach a note that can no longer save"
    );
    release.send(()).unwrap();
    for _ in 0..50 {
        if app.platform.quit.get() {
            break;
        }
        app.settle();
    }
    assert!(app.platform.quit.get(), "the app quits once the drain ends");
}

/// Notes shipping has yet to move keep the library closed, and the window says so. Closing it
/// quits, as shipping exits on a refused startup. On macOS this platform's window can't be
/// hidden, so close-to-hide falls back to quitting too; only Linux tells the two apart.
#[test]
fn an_unfinished_storage_move_keeps_the_library_closed_and_closing_quits() {
    let app = Harness::refused();
    let tree = app.tree();
    tree.find(Role::Heading, "Anarlog can't open your notes yet");
    assert!(!app.platform.quit.get());

    app.app.update(|cx| {
        app.window
            .update(cx, |root, window, cx| root.close_main(window, cx))
            .unwrap()
    });
    app.settle();
    assert!(app.platform.quit.get());
}

#[test]
fn refused_startup_exposes_support_links_copy_details_and_quit() {
    let app = Harness::refused();
    let tree = app.tree();
    tree.find(Role::Heading, "Anarlog can't open your notes yet");
    assert_named(&tree, "startup-refused");

    app.keys(&["tab"]);
    assert_eq!(app.tree().focused().label(), "Email team@fastrepl.com");
    app.keys(&["tab"]);
    assert_eq!(app.tree().focused().label(), "Discord (anarlog.so/discord)");
    app.keys(&["tab"]);
    assert_eq!(app.tree().focused().label(), "Copy details");
    app.keys(&["tab"]);
    assert_eq!(app.tree().focused().label(), "Quit");
    app.keys(&["shift-tab"]);
    assert_eq!(app.tree().focused().label(), "Copy details");

    let tree = app.tree();
    let email = tree.find(Role::Link, "Email team@fastrepl.com");
    assert!(email.clickable(), "{}", tree.outline());
    app.press(email.id);
    assert_eq!(app.tree().focused().label(), "Email team@fastrepl.com");

    let tree = app.tree();
    let discord = tree.find(Role::Link, "Discord (anarlog.so/discord)");
    assert!(discord.clickable(), "{}", tree.outline());
    app.press(discord.id);
    assert_eq!(app.tree().focused().label(), "Discord (anarlog.so/discord)");

    assert_eq!(
        app.platform.opened_urls.borrow().as_slice(),
        [
            "mailto:team@fastrepl.com?subject=Anarlog%20storage%20move%20help",
            "https://anarlog.so/discord",
        ]
    );

    app.keys(&["tab", "tab"]);
    let tree = app.tree();
    assert_eq!(tree.focused().label(), "Quit", "{}", tree.outline());

    app.press(tree.find(Role::Button, "Copy details").id);
    assert!(!app.platform.quit.get());
    assert_eq!(app.tree().focused().label(), "Copied details");

    app.app
        .update(|cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(String::new())));
    app.key_press("enter");
    assert!(!app.platform.quit.get());

    let copied = app
        .app
        .update(|cx| cx.read_from_clipboard().and_then(|item| item.text()))
        .expect("diagnostic details in clipboard");
    assert!(copied.contains("Previous folder:"), "{copied}");
    assert!(copied.contains("vault"), "{copied}");
    assert!(copied.contains("Current folder:"), "{copied}");
    assert!(copied.contains("base"), "{copied}");
    assert!(copied.contains("Version:"), "{copied}");

    let tree = app.tree();
    assert!(tree.find(Role::Button, "Copied details").clickable());
    tree.find(Role::Status, "Details copied to clipboard");

    app.keys(&["tab"]);
    let tree = app.tree();
    let quit = tree.find(Role::Button, "Quit");
    assert!(quit.clickable(), "{}", tree.outline());
    assert_eq!(tree.focused().label(), "Quit", "{}", tree.outline());
    assert!(!app.platform.quit.get());
    app.key_press("enter");
    assert!(app.platform.quit.get());
}
