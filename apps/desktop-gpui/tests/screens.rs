//! Renders the app's main screens to PNGs, without opening a window, for side-by-side
//! comparison with the shipping desktop app. The seeded library is copied next to the
//! screenshots so the shipping app can render the same data.
//!
//! ANLG_SCREENS_DIR=/tmp/screens cargo test --test screens
//!
//! Seed data is every `tests/screens/*.sql`, applied in name order.
//!
//! Without ANLG_SCREENS_DIR it does nothing, so `cargo test` and CI skip it. It runs as a
//! plain `main` because the macOS platform must be created on the main thread.

use std::{path::PathBuf, sync::Arc, time::Duration};

use anlg_listener_core::LiveTranscriptSegment;
use anlg_transcript::{ChannelProfile, SegmentKey, SegmentWord};
use desktop_gpui::{
    application::ApplicationView,
    chat::{ChatView, state::ChatEvent},
    meeting::{
        floating::{FloatingBar, container_size, shows_expand},
        live::{CaptureMode, LiveMeeting, LiveState, SessionMode},
    },
    ui::theme::UiFonts,
    workspace::{assets::Assets, navigation::Route},
};
use desktop_runtime::{Profile, RuntimeHandle, ServiceError, SessionId};
use gpui::{
    AnyWindowHandle, AppContext, HeadlessAppContext, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PlatformInput, ScrollDelta, ScrollEvent, ScrollSource,
    TouchPhase, WindowHandle, point, px, size,
};

const WINDOW: (f32, f32) = (910., 600.);

fn main() {
    let Some(out) = std::env::var_os("ANLG_SCREENS_DIR").map(PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&out).unwrap();
    let profile = tempfile::tempdir().unwrap();
    let (runtime, ready) = desktop_gpui::runtime_bridge::start(Profile {
        database: profile.path().join("library.sqlite"),
    })
    .unwrap();
    finish_onboarding(&runtime);

    let text_system = gpui_platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(
        text_system,
        Arc::new(Assets),
        gpui_platform::current_headless_renderer,
    );
    // The runtime answers from its own threads, so the dispatcher must wait for real I/O.
    cx.allow_parking();
    cx.update(UiFonts::init);
    let window = {
        let runtime = runtime.clone();
        let profile = profile.path().to_path_buf();
        cx.open_window(size(px(WINDOW.0), px(WINDOW.1)), move |window, cx| {
            cx.new(|cx| {
                ApplicationView::new(
                    runtime,
                    ready,
                    profile.clone(),
                    profile.with_extension("storage.json"),
                    window,
                    cx,
                )
            })
        })
        .unwrap()
    };
    settle(&mut cx);
    // Before seeding there is no language model, so chat shows its setup prompt.
    chat(&mut cx, window, |chat, window, cx| {
        chat.event(ChatEvent::Open, window, cx)
    });
    capture(&mut cx, window, &out, "chat-setup");
    chat(&mut cx, window, |chat, window, cx| {
        chat.event(ChatEvent::Close, window, cx)
    });
    seed(&runtime, &out);
    settle(&mut cx);

    capture(&mut cx, window, &out, "main");
    // The open-note dialog before any note was opened: fresh, filtered, and without a match.
    type_text(&mut cx, window, &["cmd-k"]);
    capture(&mut cx, window, &out, "search");
    type_text(&mut cx, window, &["s", "y", "n", "c"]);
    capture(&mut cx, window, &out, "search-query");
    type_text(&mut cx, window, &["x"]);
    capture(&mut cx, window, &out, "search-none");
    type_text(&mut cx, window, &["escape"]);
    // Sidebar chrome, by window position: sort menu, its Grouping submenu, then the toggle.
    click(&mut cx, window, 178., 23.);
    pointer(&mut cx, window, 260., 67.);
    capture(&mut cx, window, &out, "sort-menu");
    click(&mut cx, window, 450., 108.);
    capture(&mut cx, window, &out, "folder-grouping");
    click(&mut cx, window, 178., 23.);
    pointer(&mut cx, window, 260., 67.);
    click(&mut cx, window, 450., 76.);
    click(&mut cx, window, 94., 23.);
    capture(&mut cx, window, &out, "sidebar-collapsed");
    click(&mut cx, window, 94., 23.);
    pointer(&mut cx, window, 700., 590.);
    scroll(&mut cx, window, -260.);
    capture(&mut cx, window, &out, "timeline-scrolled");
    scroll(&mut cx, window, 1000.);
    chat(&mut cx, window, |chat, window, cx| {
        chat.event(ChatEvent::Open, window, cx)
    });
    capture(&mut cx, window, &out, "chat-empty");
    chat(&mut cx, window, |chat, _, cx| chat.toggle_history(cx));
    capture(&mut cx, window, &out, "chat-history");
    chat(&mut cx, window, |chat, _, cx| {
        chat.select_group("g-launch".into(), cx)
    });
    capture(&mut cx, window, &out, "chat-thread");
    chat(&mut cx, window, |chat, _, cx| {
        chat.select_group("g-hiring".into(), cx)
    });
    capture(&mut cx, window, &out, "chat-activity");
    chat(&mut cx, window, |chat, _, cx| {
        chat.select_group("g-launch".into(), cx)
    });
    chat(&mut cx, window, |chat, window, cx| {
        chat.event(ChatEvent::OpenRightPanel, window, cx)
    });
    capture(&mut cx, window, &out, "chat-right");
    chat(&mut cx, window, |chat, window, cx| {
        chat.event(ChatEvent::Close, window, cx)
    });
    // The note's more button and its tabs, by window position.
    open(&mut cx, window, Route::Session(SessionId("s-sync".into())));
    click(&mut cx, window, 888., 23.);
    capture(&mut cx, window, &out, "note-menu");
    open(
        &mut cx,
        window,
        Route::Session(SessionId("s-critique".into())),
    );
    capture(&mut cx, window, &out, "note-summary");
    click(&mut cx, window, 329., 23.);
    capture(&mut cx, window, &out, "note-memos");
    click(&mut cx, window, 362., 23.);
    capture(&mut cx, window, &out, "note-transcript");
    // Typing "/" after the customer note's first line opens the slash menu.
    open(
        &mut cx,
        window,
        Route::Session(SessionId("s-customer".into())),
    );
    click(&mut cx, window, 850., 70.);
    type_text(&mut cx, window, &["space", "/"]);
    capture(&mut cx, window, &out, "note-slash");
    // Recently opened notes lead the dialog, opened from the sidebar's search button; the
    // arrows move the selection.
    for id in ["s-hiring", "s-pricing", "s-offsite", "s-customer", "s-sync"] {
        open(&mut cx, window, Route::Session(SessionId(id.into())));
    }
    click(&mut cx, window, 122., 23.);
    capture(&mut cx, window, &out, "search-recent");
    type_text(&mut cx, window, &["down", "down"]);
    capture(&mut cx, window, &out, "search-keyboard");
    type_text(&mut cx, window, &["escape"]);
    meeting(&mut cx, window, profile.path(), &out);
    contacts(&mut cx, window, &out);
    for (name, route) in [
        ("note", Route::Session(SessionId("s-sync".into()))),
        (
            "note-markdown",
            Route::Session(SessionId("s-korean".into())),
        ),
        (
            "note-typography",
            Route::Session(SessionId("s-typography".into())),
        ),
        ("note-empty", Route::Session(SessionId("s-welcome".into()))),
        ("calendar", Route::Calendar),
        ("templates", Route::Templates),
        ("folders", Route::Folders),
        ("settings", Route::Settings("app".into())),
    ] {
        open(&mut cx, window, route);
        capture(&mut cx, window, &out, name);
        if name == "folders" {
            templates_and_folders(&mut cx, window, &out);
        }
    }
    for page in [
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
        open(&mut cx, window, Route::Settings(page.into()));
        capture(&mut cx, window, &out, &format!("settings-{page}"));
    }
    account_states(&mut cx, window, &out);
    capture_onboarding(&mut cx, window, &out);
}

/// Account, plan and seat states, seeded through the same app-wide value the cloud service
/// publishes after sign-in.
fn account_states(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    use desktop_gpui::product::account::{
        self, Account, Claims, Devices, User, Workspace, WorkspaceTier,
    };
    let now = account::now();
    let day = 86_400;
    let user = || {
        Some(User {
            id: "7f1c2a9e-3b4d-4e5f-8a6b-9c0d1e2f3a4b".into(),
            email: Some("alex@example.com".into()),
        })
    };
    let signed_in =
        |claims: Claims, workspaces: Vec<Workspace>, devices: Option<Devices>| Account {
            user: user(),
            claims: Some(claims),
            workspaces: Some(workspaces.into()),
            trial_eligible: true,
            devices,
        };
    let free = signed_in(Claims::default(), vec![], None);
    let trial = signed_in(
        Claims {
            subscription_status: Some("trialing".into()),
            trial_end: Some(now + 5 * day - 60),
            has_payment_method: Some(false),
            ..Claims::default()
        },
        vec![],
        Some(Devices { used: 1, max: 3 }),
    );
    let pro = signed_in(
        Claims {
            entitlements: vec!["hyprnote_pro".into()],
            subscription_status: Some("active".into()),
            current_period_end: Some(now + 20 * day),
            ..Claims::default()
        },
        vec![],
        Some(Devices { used: 2, max: 3 }),
    );
    let seat = signed_in(
        Claims::default(),
        vec![Workspace {
            id: "ws-acme".into(),
            name: "Acme".into(),
            tier: WorkspaceTier::Team,
            seat_limit: Some(10),
            used_seats: 4,
        }],
        None,
    );
    for (name, state, page) in [
        ("account-signed-in", free.clone(), "account"),
        ("billing-free", free.clone(), "billing"),
        ("sync-free", free.clone(), "sync"),
        ("team-free", free.clone(), "team"),
        ("dictation-free", free.clone(), "dictation"),
        ("billing-trial", trial, "billing"),
        ("billing-pro", pro.clone(), "billing"),
        ("dictation-pro", pro, "dictation"),
        ("billing-team-seat", seat.clone(), "billing"),
        ("meetings-team-seat", seat, "meetings"),
    ] {
        cx.update(|cx| account::set(state, cx));
        open(cx, window, Route::Settings(page.into()));
        capture(cx, window, out, &format!("settings-{name}"));
    }
    cx.update(|cx| account::set(free, cx));
    open(cx, window, Route::Settings("account".into()));
    click(cx, window, 850., 100.);
    capture(cx, window, out, "settings-account-sign-out");
    cx.update(|cx| account::set(Account::default(), cx));
}

/// The live meeting, its floating bar and the note after recording. Live states come from the
/// same `LiveState` the capture poll feeds; no microphone or provider is involved.
fn meeting(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    profile: &std::path::Path,
    out: &std::path::Path,
) {
    let audio = profile.join("vault/sessions/s-retro");
    std::fs::create_dir_all(&audio).unwrap();
    std::fs::write(audio.join("audio.wav"), wav(60)).unwrap();
    open(cx, window, Route::Session(SessionId("s-retro".into())));
    capture(cx, window, out, "meeting-after");
    click(cx, window, TRANSCRIPT_TAB_AFTER.0, TRANSCRIPT_TAB_AFTER.1);
    capture(cx, window, out, "meeting-after-transcript");

    let standup = SessionId("s-standup".into());
    let mut live = LiveMeeting::new(standup.clone(), SessionMode::Active);
    live.amplitude = 0.35;
    set_live(cx, Some(live.clone()));
    open(cx, window, Route::Session(standup.clone()));
    capture(cx, window, out, "meeting-live-memos");
    click(cx, window, TRANSCRIPT_TAB_LIVE.0, TRANSCRIPT_TAB_LIVE.1);
    capture(cx, window, out, "meeting-listening");
    live.segments = live_segments().into();
    set_live(cx, Some(live.clone()));
    capture(cx, window, out, "meeting-live-transcript");
    floating(cx, out, "floating-compact", false, false);
    floating(cx, out, "floating-hover", false, true);
    floating(cx, out, "floating-expanded", true, false);

    let mut paused = live.clone();
    paused.capture = CaptureMode::FallbackRecordOnly;
    paused.paused = Some(
        "Live transcript paused because Deepgram is having an outage. Your audio is still being saved, and the missing part will be filled in from the recording."
            .into(),
    );
    paused.warning = Some("Microphone unavailable. System audio continues".into());
    set_live(cx, Some(paused));
    capture(cx, window, out, "meeting-paused");
    floating(cx, out, "floating-warning", true, false);

    let mut batch = LiveMeeting::new(standup.clone(), SessionMode::Active);
    batch.capture = CaptureMode::RecordOnly;
    batch.amplitude = 0.35;
    set_live(cx, Some(batch));
    capture(cx, window, out, "meeting-batch");
    floating(cx, out, "floating-batch", false, false);

    set_live(
        cx,
        Some(LiveMeeting::new(standup.clone(), SessionMode::Finalizing)),
    );
    capture(cx, window, out, "meeting-finalizing");
    set_live(cx, None);
    capture(cx, window, out, "meeting-stopped");
}

/// The transcript tab of a finished meeting (Summary, Memos, Transcript) and of a live one
/// (Memos, Transcript), by window position.
const TRANSCRIPT_TAB_AFTER: (f32, f32) = (362., 23.);
const TRANSCRIPT_TAB_LIVE: (f32, f32) = (315., 23.);

fn set_live(cx: &mut HeadlessAppContext, live: Option<LiveMeeting>) {
    cx.update(|cx| {
        LiveState::global(cx)
            .unwrap()
            .update(cx, |state, cx| state.set(live, cx))
    });
    settle(cx);
}

/// Captures the floating bar in its own headless window sized like the panel. The application
/// must already show its bar for the live meeting; headless windows cannot resize their layout
/// viewport, so each state gets a window of its exact size. `hover` places the pointer on the
/// bar, otherwise it sits outside.
fn floating(
    cx: &mut HeadlessAppContext,
    out: &std::path::Path,
    name: &str,
    expanded: bool,
    hover: bool,
) {
    assert!(
        cx.update(|cx| cx
            .windows()
            .into_iter()
            .any(|window| window.downcast::<FloatingBar>().is_some())),
        "the application shows a floating bar while listening"
    );
    let live = cx.update(|cx| LiveState::global(cx).unwrap());
    let shows = cx.update(|cx| live.read(cx).current().is_some_and(shows_expand));
    let size = container_size(expanded && shows, shows);
    let bar = cx
        .open_window(size, move |_, cx| {
            cx.new(|cx| {
                let mut bar = FloatingBar::new(live, cx);
                bar.set_expanded(expanded, cx);
                bar
            })
        })
        .unwrap();
    settle(cx);
    let window: AnyWindowHandle = bar.into();
    let position = if hover {
        point(size.width / 2., size.height - px(10.))
    } else {
        point(px(-20.), px(-20.))
    };
    cx.update_window(window, |_, window, cx| {
        window.draw(cx).clear(cx);
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position,
                pressed_button: None,
                modifiers: Modifiers::default(),
            }),
            cx,
        );
    })
    .unwrap();
    settle(cx);
    cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    let image = cx.capture_screenshot(window).unwrap();
    let path = out.join(format!("{name}.png"));
    image.save(&path).unwrap();
    println!("{}", path.display());
    cx.update_window(window, |_, window, _| window.remove_window())
        .unwrap();
}

fn segment(
    id: &str,
    channel: ChannelProfile,
    speaker: Option<i32>,
    start_ms: i64,
    words: &[(&str, bool)],
) -> Arc<LiveTranscriptSegment> {
    let words: Vec<SegmentWord> = words
        .iter()
        .enumerate()
        .map(|(index, (text, is_final))| SegmentWord {
            text: (*text).into(),
            start_ms: start_ms + index as i64 * 300,
            end_ms: start_ms + index as i64 * 300 + 280,
            channel,
            is_final: *is_final,
            id: is_final.then(|| format!("{id}-{index}")),
        })
        .collect();
    Arc::new(LiveTranscriptSegment {
        id: id.into(),
        key: SegmentKey {
            channel,
            speaker_index: speaker,
            speaker_human_id: None,
        },
        start_ms,
        end_ms: words.last().map_or(start_ms, |word| word.end_ms),
        text: String::new(),
        words,
    })
}

fn live_segments() -> Vec<Arc<LiveTranscriptSegment>> {
    fn words(text: &'static str) -> Vec<(&'static str, bool)> {
        text.split(' ').map(|word| (word, true)).collect()
    }
    vec![
        segment(
            "l1",
            ChannelProfile::DirectMic,
            None,
            0,
            &words("Okay, let's get started with the launch checklist."),
        ),
        segment(
            "l2",
            ChannelProfile::RemoteParty,
            Some(0),
            4000,
            &words("Marketing assets are ready and the press note goes out Thursday."),
        ),
        segment(
            "l3",
            ChannelProfile::DirectMic,
            None,
            9000,
            &words("Great. What is still blocking the beta invites?"),
        ),
        segment(
            "l4",
            ChannelProfile::RemoteParty,
            Some(1),
            13000,
            &[
                ("We", true),
                ("still", true),
                ("need", true),
                ("the", true),
                ("updated", false),
                ("pricing", false),
                ("page", false),
            ],
        ),
    ]
}

/// A stereo 16 kHz WAV whose two channels pulse like alternating speakers.
fn wav(seconds: u32) -> Vec<u8> {
    const RATE: u32 = 16_000;
    let frames = RATE * seconds;
    let mut bytes = Vec::with_capacity(44 + frames as usize * 4);
    let data = frames * 4;
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&RATE.to_le_bytes());
    bytes.extend_from_slice(&(RATE * 4).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    for frame in 0..frames {
        let time = frame as f32 / RATE as f32;
        let tone = (time * 220. * std::f32::consts::TAU).sin();
        let envelope = |offset: f32| {
            let phase = ((time + offset) * 0.7).sin() * ((time + offset) * 2.3).sin();
            phase.max(0.)
        };
        for offset in [0., 1.9] {
            let sample = (tone * envelope(offset) * 12_000.) as i16;
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
    }
    bytes
}

/// A fresh profile opens first-run onboarding; the library captures start past it.
fn finish_onboarding(runtime: &RuntimeHandle) {
    let mut state = runtime.state();
    while !matches!(
        *state.borrow_and_update(),
        desktop_runtime::RuntimeState::Ready
    ) {
        futures::executor::block_on(state.changed()).unwrap();
    }
    let reply = runtime
        .submit(|services| async move {
            sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, 'false')")
                .bind(desktop_gpui::product::onboarding::NEEDED_KEY)
                .execute(services.db.pool())
                .await
                .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
            Ok(())
        })
        .unwrap();
    futures::executor::block_on(reply.receive()).unwrap();
}

/// Each onboarding step from seeded state, so no permission, account or audio is touched.
fn capture_onboarding(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    use desktop_gpui::product::onboarding::{Access, OnboardingPreview, State, Step, TrialPhase};
    let granted = State {
        microphone: Access::Authorized,
        system_audio: Access::Authorized,
        accessibility: Access::Authorized,
        enabled_calendars: 1,
        ..State::default()
    };
    let skipped = State {
        skipped_login: true,
        ..granted.clone()
    };
    let signed_in = State {
        signed_in: true,
        ..granted.clone()
    };
    let preview = |cx: &mut HeadlessAppContext, step, state: State| {
        cx.update(|cx| {
            cx.set_global(OnboardingPreview {
                step: Some(step),
                state,
            })
        });
        // An empty corner: drawing once starts loading the step's images before the capture.
        pointer(cx, window, 905., 595.);
    };
    preview(cx, Step::Permissions, State::default());
    open(cx, window, Route::Onboarding);
    pointer(cx, window, 905., 595.);
    capture(cx, window, out, "onboarding-permissions");
    preview(
        cx,
        Step::Permissions,
        State {
            microphone: Access::Authorized,
            system_audio: Access::Denied,
            ..State::default()
        },
    );
    capture(cx, window, out, "onboarding-permissions-partial");
    for (name, step, state) in [
        ("onboarding-login", Step::Login, granted.clone()),
        (
            "onboarding-login-checking",
            Step::Login,
            State {
                trial: Some(TrialPhase::Checking),
                ..signed_in.clone()
            },
        ),
        (
            "onboarding-login-trial",
            Step::Login,
            State {
                trial: Some(TrialPhase::Started { days: Some(30) }),
                ..signed_in.clone()
            },
        ),
        ("onboarding-calendar", Step::Calendar, skipped.clone()),
        ("onboarding-calendar-signed-in", Step::Calendar, signed_in),
        ("onboarding-imports", Step::Imports, skipped.clone()),
        ("onboarding-final", Step::Final, skipped),
    ] {
        preview(cx, step, state);
        capture(cx, window, out, name);
    }
    cx.update(|cx| cx.remove_global::<OnboardingPreview>());
}

/// Templates and folders, by window position: list rows are 36px tall from y=110.
fn templates_and_folders(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    open(cx, window, Route::Templates);
    click(cx, window, 100., 146.);
    capture(cx, window, out, "templates-favorite");
    click(cx, window, 100., 110.);
    capture(cx, window, out, "templates-auto");
    click(cx, window, 100., 254.);
    click(cx, window, 154., 23.);
    capture(cx, window, out, "templates-sort");
    click(cx, window, 154., 23.);
    click(cx, window, 182., 23.);
    capture(cx, window, out, "templates-pro-required");
    // Pro comes from the app-wide account; reset it so later captures stay signed out.
    cx.update(|cx| {
        use desktop_gpui::product::account::{self, Account, Claims, User};
        account::set(
            Account {
                user: Some(User {
                    id: "7f1c2a9e-3b4d-4e5f-8a6b-9c0d1e2f3a4b".into(),
                    email: Some("alex@example.com".into()),
                }),
                claims: Some(Claims {
                    entitlements: vec!["hyprnote_pro".into()],
                    subscription_status: Some("active".into()),
                    current_period_end: Some(account::now() + 86_400),
                    ..Claims::default()
                }),
                ..Account::default()
            },
            cx,
        )
    });
    click(cx, window, 100., 146.);
    pointer(cx, window, 500., 200.);
    capture(cx, window, out, "templates-pro");
    cx.update(|cx| desktop_gpui::product::account::set(Default::default(), cx));
    open(cx, window, Route::Folders);
    click(cx, window, 182., 23.);
    capture(cx, window, out, "folders-new");
    click(cx, window, 10., 590.);
    click(cx, window, 100., 218.);
    capture(cx, window, out, "folders-research");
    click(cx, window, 231., 24.);
    capture(cx, window, out, "folders-picker");
    click(cx, window, 231., 24.);
    click(cx, window, 840., 24.);
    capture(cx, window, out, "folders-share");
    click(cx, window, 10., 590.);
    click(cx, window, 892., 24.);
    click(cx, window, 840., 66.);
    capture(cx, window, out, "folders-delete");
    click(cx, window, 10., 590.);
}

/// People, organizations and their menus, by window position.
fn contacts(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    for (name, route) in [
        ("contacts", Route::Contacts),
        ("contacts-person", Route::Human("h-maya".into())),
        ("contacts-duplicate", Route::Human("h-jordan".into())),
        ("contacts-photo", Route::Human("h-priya".into())),
        (
            "contacts-organization",
            Route::Organization("org-northwind".into()),
        ),
        ("contacts-team", Route::Organization("org-team".into())),
    ] {
        open(cx, window, route);
        capture(cx, window, out, name);
    }
    open(cx, window, Route::Human("h-maya".into()));
    click(cx, window, 892., 24.);
    capture(cx, window, out, "contacts-options");
    click(cx, window, 892., 24.);
    pointer(cx, window, 847., 24.);
    capture(cx, window, out, "contacts-enrich");
    open(cx, window, Route::Human("h-taylor".into()));
    click(cx, window, 393., 293.);
    capture(cx, window, out, "contacts-company");
    click(cx, window, 700., 580.);
    click(cx, window, 182., 23.);
    capture(cx, window, out, "contacts-new-person");
    type_text(cx, window, &["escape"]);
    open(cx, window, Route::Human("h-maya".into()));
    pointer(cx, window, 600., 400.);
    input(
        cx,
        window,
        [PlatformInput::Scroll(ScrollEvent {
            position: point(px(600.), px(400.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-700.))),
            modifiers: Modifiers::default(),
            touch_phase: TouchPhase::Moved,
            source: ScrollSource::Pointer,
        })],
    );
    capture(cx, window, out, "contacts-scrolled");
}

fn chat(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    action: impl FnOnce(&mut ChatView, &mut gpui::Window, &mut gpui::Context<ChatView>),
) {
    cx.update(|cx| {
        window
            .update(cx, |root, window, cx| {
                let chat = root.workspace().read(cx).chat().clone();
                chat.update(cx, |chat, cx| action(chat, window, cx))
            })
            .unwrap()
    });
    settle(cx);
}

fn seed(runtime: &RuntimeHandle, out: &std::path::Path) {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/screens");
    let mut files = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "sql"))
        .collect::<Vec<_>>();
    files.sort();
    let scripts = files
        .iter()
        .map(|path| std::fs::read_to_string(path).unwrap())
        .collect::<Vec<_>>();
    let copy = out.join("library.sqlite");
    let _ = std::fs::remove_file(&copy);
    let copy = copy.to_string_lossy().into_owned();
    let reply = runtime
        .submit(move |services| async move {
            let failed = |error: sqlx::Error| ServiceError::Failed(error.to_string().into());
            for script in scripts {
                sqlx::raw_sql(sqlx::AssertSqlSafe(script))
                    .execute(services.db.pool())
                    .await
                    .map_err(failed)?;
            }
            sqlx::query("VACUUM INTO ?")
                .bind(copy)
                .execute(services.db.pool())
                .await
                .map_err(failed)?;
            Ok(())
        })
        .unwrap();
    futures::executor::block_on(reply.receive()).unwrap();
}

fn open(cx: &mut HeadlessAppContext, window: WindowHandle<ApplicationView>, route: Route) {
    cx.update(|cx| {
        window
            .update(cx, |root, _, cx| {
                root.workspace()
                    .clone()
                    .update(cx, |workspace, cx| workspace.open_route(route, false, cx))
            })
            .unwrap()
    });
    settle(cx);
}

fn capture(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
    name: &str,
) {
    let window: AnyWindowHandle = window.into();
    // Captures read the last rendered scene, so draw the current state first.
    cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    let image = cx.capture_screenshot(window).unwrap();
    let path = out.join(format!("{name}.png"));
    image.save(&path).unwrap();
    println!("{}", path.display());
}

fn pointer(cx: &mut HeadlessAppContext, window: WindowHandle<ApplicationView>, x: f32, y: f32) {
    input(
        cx,
        window,
        [PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(x), px(y)),
            pressed_button: None,
            modifiers: Modifiers::default(),
        })],
    );
}

/// Scrolls the sidebar timeline; negative deltas move the content up.
fn scroll(cx: &mut HeadlessAppContext, window: WindowHandle<ApplicationView>, y: f32) {
    input(
        cx,
        window,
        [PlatformInput::Scroll(ScrollEvent {
            position: point(px(100.), px(400.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(y))),
            modifiers: Modifiers::default(),
            touch_phase: TouchPhase::Moved,
            source: ScrollSource::Pointer,
        })],
    );
}

fn click(cx: &mut HeadlessAppContext, window: WindowHandle<ApplicationView>, x: f32, y: f32) {
    let position = point(px(x), px(y));
    input(
        cx,
        window,
        [
            PlatformInput::MouseMove(MouseMoveEvent {
                position,
                pressed_button: None,
                modifiers: Modifiers::default(),
            }),
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            }),
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers: Modifiers::default(),
                click_count: 1,
            }),
        ],
    );
}

/// Dispatches pointer input against the last drawn frame's hitboxes.
/// Types keystrokes (`Keystroke::parse` syntax) through the focused view's input handler.
fn type_text(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    keys: &[&'static str],
) {
    let window: AnyWindowHandle = window.into();
    for key in keys {
        cx.update_window(window, |_, window, cx| {
            window.draw(cx).clear(cx);
            window.dispatch_keystroke(gpui::Keystroke::parse(key).unwrap(), cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
    settle(cx);
}

fn input<const N: usize>(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    events: [PlatformInput; N],
) {
    let window: AnyWindowHandle = window.into();
    for event in events {
        cx.update_window(window, |_, window, cx| {
            window.draw(cx).clear(cx);
            window.dispatch_event(event, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
    settle(cx);
}

/// Lets runtime replies and simulated timers land before a frame is captured.
fn settle(cx: &mut HeadlessAppContext) {
    for _ in 0..40 {
        cx.advance_clock(Duration::from_millis(50));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(25));
    }
}
