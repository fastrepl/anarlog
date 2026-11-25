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
        upload::{Progress, Transcriber, TranscriberGlobal, Transcription, Warn},
    },
    ui::theme::UiFonts,
    workspace::{assets::Assets, navigation::Route},
    zoom::{self, Zoom},
};
use desktop_runtime::{CancellationToken, Profile, RuntimeHandle, ServiceError, SessionId};
use gpui::{
    AnyWindowHandle, AppContext, Focusable, HeadlessAppContext, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput, ScrollDelta, ScrollEvent,
    ScrollSource, TouchPhase, WindowHandle, point, px, size,
};

const WINDOW: (f32, f32) = (910., 600.);

fn main() {
    let Some(out) = std::env::var_os("ANLG_SCREENS_DIR").map(PathBuf::from) else {
        return;
    };
    // Before anything can click: no system dialog, app launch, permission prompt or sound.
    desktop_gpui::platform::os::go_headless();
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
    cx.update(|cx| cx.set_global(Zoom::load(profile.path().join("zoom.json"))));
    // Imports detection, Reminders access and GitHub answer from fixtures, never this Mac;
    // the upload file picker answers as cancelled and its Finder reveal does nothing.
    cx.update(|cx| {
        use desktop_gpui::{meeting::upload::FilesGlobal, product, workspace::task};
        cx.set_global(product::local::imports::InstalledAppsOverride(Arc::new(
            Apps(vec![]),
        )));
        cx.set_global(product::ticket::RemindersOverride(Arc::new(Reminders)));
        cx.set_global(task::GithubOverride(Arc::new(Github)));
        cx.set_global(FilesGlobal(std::rc::Rc::new(NoFiles)));
    });
    let window = {
        let runtime = runtime.clone();
        let profile = profile.path().to_path_buf();
        cx.open_window(size(px(WINDOW.0), px(WINDOW.1)), move |window, cx| {
            cx.new(|cx| {
                ApplicationView::new(
                    runtime,
                    ready,
                    desktop_gpui::launch::Storage {
                        channel: None,
                        root: profile.clone(),
                        database_dir: profile.clone(),
                        pointer: Some(profile.with_extension("storage.json")),
                    },
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
    if std::env::var("ANLG_SCREENS_ONLY").as_deref() == Ok("chat-edits") {
        chat_edits(&mut cx, window, &runtime, &out);
        return;
    }
    if std::env::var("ANLG_SCREENS_ONLY").as_deref() == Ok("ai-features") {
        ai_features(&mut cx, window, &runtime, profile.path(), &out);
        return;
    }
    if std::env::var("ANLG_SCREENS_ONLY").as_deref() == Ok("zoom") {
        zoom(&mut cx, window, &out);
        return;
    }

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
    chat_composer(&mut cx, window, &runtime, &out);
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
    click(&mut cx, window, 349., 23.);
    capture(&mut cx, window, &out, "note-memos");
    click(&mut cx, window, 362., 23.);
    capture(&mut cx, window, &out, "note-transcript");
    // With the sidebar collapsed, the header starts after its toggle.
    click(&mut cx, window, 94., 23.);
    capture(&mut cx, window, &out, "note-sidebar-collapsed");
    click(&mut cx, window, 94., 23.);
    // Typing "/" after the customer note's first line opens the slash menu.
    open(
        &mut cx,
        window,
        Route::Session(SessionId("s-customer".into())),
    );
    click(&mut cx, window, 850., 70.);
    type_text(&mut cx, window, &["space", "/"]);
    capture(&mut cx, window, &out, "note-slash");
    // Later captures (find and replace) show this memo as seeded.
    type_text(&mut cx, window, &["escape", "backspace", "backspace"]);
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
    transcript_files(&mut cx, window, profile.path(), &out);
    dictation(&mut cx, &out);
    contacts(&mut cx, window, &out);
    timeline_details(&mut cx, window, &out);
    calendar_depth(&mut cx, window, &out);
    sharing(&mut cx, window, &out);
    note_depth(&mut cx, window, &out);
    toasts(&mut cx, window, &out);
    meeting_records(&mut cx, window, &runtime, profile.path(), &out);
    locks(&mut cx, window, &runtime, &out);
    note_header(&mut cx, window, &out);
    note_windows(&mut cx, window, &out);
    find_replace(&mut cx, window, &out);
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
        ("automations", Route::Automations),
        ("settings", Route::Settings("app".into())),
    ] {
        open(&mut cx, window, route);
        capture(&mut cx, window, &out, name);
        if name == "folders" {
            templates_and_folders(&mut cx, window, &out);
        }
        if name == "automations" {
            automations(&mut cx, window, &out);
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
        if page == "transcription" {
            provider_menus(&mut cx, window, &runtime, &out);
        }
    }
    developers(&mut cx, window, &runtime, &out);
    chat_edits(&mut cx, window, &runtime, &out);
    account_states(&mut cx, window, &out);
    detection(&mut cx, window, &out);
    instruction(&mut cx, &out);
    polish(&mut cx, window, &out);
    teams_and_sync(&mut cx, window, &out);
    imports_crm_ticket(&mut cx, window, &out);
    app_links_and_transcript_selection(&mut cx, window, &out);
    ai_features(&mut cx, window, &runtime, profile.path(), &out);
    zoom(&mut cx, window, &out);
    memo_templates_narrow(&mut cx, window, &runtime, &out);
    capture_onboarding(&mut cx, window, &out);
}

/// UI zoom. Headless windows skip the platform adapter that zooms real windows
/// (`desktop_gpui::zoom`), so `zoom_window` gives a window the geometry the adapter reports:
/// 1/factor of its real size at factor times the 2x scale. Captures keep the real size's 2x
/// pixels.
fn zoom(cx: &mut HeadlessAppContext, window: WindowHandle<ApplicationView>, out: &std::path::Path) {
    use desktop_gpui::dictation::{
        controller::Phase,
        overlay::{self, DictationBar, Panel},
    };
    let main: AnyWindowHandle = window.into();
    let real = size(px(WINDOW.0), px(WINDOW.1));
    for (factor, route, name) in [
        (
            0.5,
            Route::Session(SessionId("s-sync".into())),
            "zoom-50-note",
        ),
        (
            1.5,
            Route::Session(SessionId("s-sync".into())),
            "zoom-150-note",
        ),
        (
            2.,
            Route::Session(SessionId("s-sync".into())),
            "zoom-200-note",
        ),
        (
            3.,
            Route::Session(SessionId("s-sync".into())),
            "zoom-300-note",
        ),
        (1.5, Route::Calendar, "zoom-150-calendar"),
        (1.5, Route::Settings("app".into()), "zoom-150-settings"),
    ] {
        open(cx, window, route);
        zoom_window(cx, main, real, factor);
        shoot(cx, main, out, name);
    }
    zoom_window(cx, main, real, 1.);

    let before = cx.update(|cx| cx.windows());
    let id = SessionId("s-sync".into());
    cx.update(|cx| {
        window
            .update(cx, |root, _, cx| {
                root.workspace().update(cx, |_, cx| {
                    cx.emit(desktop_gpui::workspace::WorkspaceAction::OpenNoteWindow(id))
                })
            })
            .unwrap()
    });
    settle(cx);
    let note = cx
        .update(|cx| cx.windows())
        .into_iter()
        .find(|candidate| !before.contains(candidate))
        .expect("the note opens in a new window");
    let real = cx
        .update_window(note, |_, window, _| window.bounds().size)
        .unwrap();
    zoom_window(cx, note, real, 1.5);
    shoot(cx, note, out, "zoom-150-note-window");
    cx.update_window(note, |_, window, _| window.remove_window())
        .unwrap();

    // Content-sized windows open at their zoomed size (`zoom::window_size`).
    let panel = Panel {
        owner: "system-dictation:screens".into(),
        phase: Phase::Recording,
        amplitude: 0.45,
        microphone: "Default microphone".into(),
        preview_enabled: true,
        text: "Let's move the launch review to Thursday so design can join, and".into(),
        partial: "send the agenda to".into(),
        expanded: true,
        ..Panel::default()
    };
    let size = cx.update(|cx| zoom::window_size(overlay::size(panel.expanded), cx));
    let bar = cx
        .open_window(size, move |_, cx| cx.new(|_| DictationBar::new(panel)))
        .unwrap();
    zoom_window(cx, bar.into(), size, 1.5);
    shoot(cx, bar.into(), out, "zoom-150-dictation");
    cx.update_window(bar.into(), |_, window, _| window.remove_window())
        .unwrap();

    let mut live = LiveMeeting::new(SessionId("s-standup".into()), SessionMode::Active);
    live.amplitude = 0.35;
    set_live(cx, Some(live));
    let state = cx.update(|cx| LiveState::global(cx).unwrap());
    let shows = cx.update(|cx| state.read(cx).current().is_some_and(shows_expand));
    let size = cx.update(|cx| zoom::window_size(container_size(false, shows), cx));
    let bar = cx
        .open_window(size, move |_, cx| cx.new(|cx| FloatingBar::new(state, cx)))
        .unwrap();
    zoom_window(cx, bar.into(), size, 1.5);
    shoot(cx, bar.into(), out, "zoom-150-floating");
    cx.update_window(bar.into(), |_, window, _| window.remove_window())
        .unwrap();
    set_live(cx, None);
    cx.update(|cx| zoom::set(1., cx));
    settle(cx);
}

/// The first headless frame is unzoomed, and content-sized bars resize themselves to it, so the
/// real size is passed in rather than read back.
fn zoom_window(
    cx: &mut HeadlessAppContext,
    window: AnyWindowHandle,
    real: gpui::Size<gpui::Pixels>,
    factor: f32,
) {
    cx.update(|cx| zoom::set(factor, cx));
    // Flushes what that frame and `zoom::set` deferred before the geometry changes.
    cx.update_window(window, |_, _, _| {}).unwrap();
    cx.update_window(window, |_, window, cx| {
        window.resize(real.map(|length| length / factor));
        window.bounds_changed(cx);
        window.set_scale_factor(2. * factor);
        // The capture target keeps the real size; GPUI keeps the zoomed viewport.
        window.resize(real);
    })
    .unwrap();
    settle(cx);
}

/// App-link chips in a memo (`170-app-links.sql`), revealed at the end of the timeline, and the
/// transcript's selection bar over two adjacent segments, then its speaker picker.
fn app_links_and_transcript_selection(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    // The settings back arrow returns to the workspace.
    click(cx, window, 90., 23.);
    open(cx, window, Route::Session(SessionId("s-links".into())));
    capture(cx, window, out, "note-app-links");
    open(cx, window, Route::Session(SessionId("s-vendor".into())));
    click(cx, window, TRANSCRIPT_TAB_AFTER.0, TRANSCRIPT_TAB_AFTER.1);
    click(cx, window, 330., 23.);
    // The select circles of "It does." (Speaker 2) and the Speaker 3 segment after it.
    click(cx, window, 229., 436.);
    click(cx, window, 229., 505.);
    capture(cx, window, out, "transcript-selection-bar");
    click(cx, window, 497., 521.);
    capture(cx, window, out, "transcript-selection-speaker");
    type_text(cx, window, &["escape"]);
    click(cx, window, 330., 23.);
}

/// Settings › Transcription's provider and model menus, which are at least as wide as their
/// triggers. Picking Deepgram saves it, so its rows are removed again afterwards.
fn provider_menus(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    runtime: &RuntimeHandle,
    out: &std::path::Path,
) {
    click(cx, window, 350., 143.);
    capture(cx, window, out, "settings-transcription-provider-menu");
    type_text(cx, window, &["escape"]);
    click(cx, window, 350., 143.);
    // Deepgram, the fifth provider.
    click(cx, window, 350., 314.);
    click(cx, window, 700., 143.);
    capture(cx, window, out, "settings-transcription-model-menu");
    type_text(cx, window, &["escape"]);
    sql(
        runtime,
        "DELETE FROM app_settings WHERE id IN ('current_stt_provider', 'current_stt_model', 'ai_provider:stt:deepgram')",
    );
}

/// A file picker that never opens: a stray click cannot show an OS dialog.
struct NoFiles;

impl desktop_gpui::meeting::upload::Files for NoFiles {
    fn pick(
        &self,
        _: desktop_gpui::platform::dialogs::Picker,
        _: &mut gpui::App,
    ) -> futures::future::LocalBoxFuture<'static, desktop_runtime::Result<Option<PathBuf>>> {
        Box::pin(async { Ok(None) })
    }

    fn open_folder(&self, _: &std::path::Path, _: &mut gpui::App) {}
}

/// Hold the fake model's brief and summary halfway until their streaming captures are taken.
static BRIEF_GATE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SUMMARY_GATE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn gated(
    first: &'static str,
    rest: &'static str,
    gate: &'static std::sync::atomic::AtomicBool,
) -> futures::stream::BoxStream<
    'static,
    desktop_runtime::Result<desktop_gpui::meeting::ai::ProviderEvent>,
> {
    use desktop_gpui::meeting::ai::ProviderEvent;
    use futures::StreamExt;
    let rest = futures::stream::once(async move {
        while !gate.load(std::sync::atomic::Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Ok(ProviderEvent::Text(rest.into()))
    });
    futures::stream::iter([Ok(ProviderEvent::Text(first.into()))])
        .chain(rest)
        .boxed()
}

/// Replies with a pre-meeting brief to structured requests and a summary otherwise.
struct ScreensModel;

impl desktop_gpui::meeting::ai::ProviderAdapter for ScreensModel {
    fn stream(
        &self,
        request: desktop_gpui::meeting::ai::Request,
    ) -> futures::future::BoxFuture<
        'static,
        desktop_runtime::Result<
            futures::stream::BoxStream<
                'static,
                desktop_runtime::Result<desktop_gpui::meeting::ai::ProviderEvent>,
            >,
        >,
    > {
        let structured = request.structured.is_some();
        Box::pin(async move {
            Ok(if structured {
                gated(
                    r#"{"opener": "Confirm the 15% annual discount holds through Q3", "bullets": ["Maya owns the revised pricing page copy", "#,
                    r#""Legal still has to approve the reseller terms", "Listen for whether Sam needs the discount in writing"]}"#,
                    &BRIEF_GATE,
                )
            } else {
                gated(
                    "## Vendor terms\n\n- Globex offered net-60 payment terms for the annual contract.\n- Support moves to a shared Slack channel next month.\n",
                    "\n## Next steps\n\n- Send the signed order form by Friday.",
                    &SUMMARY_GATE,
                )
            })
        })
    }
}

/// The upcoming meeting's "Create a brief" suggestion and the streamed brief, the
/// participants' "Enhance contact" sparkle, and the summary streaming once an uploaded
/// recording is transcribed, all with fake models. The records are added late and removed
/// after; `ai-features.sqlite` lets the shipping app render the same rows.
fn ai_features(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    runtime: &RuntimeHandle,
    profile: &std::path::Path,
    out: &std::path::Path,
) {
    script(runtime, include_str!("screens/late/ai-features.sql"));
    let copy = out.join("ai-features.sqlite");
    let _ = std::fs::remove_file(&copy);
    let copy = copy.to_string_lossy().into_owned();
    let reply = runtime
        .submit(move |services| async move {
            sqlx::query("VACUUM INTO ?")
                .bind(copy)
                .execute(services.db.pool())
                .await
                .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
            Ok(())
        })
        .unwrap();
    futures::executor::block_on(reply.receive()).unwrap();
    cx.update(|cx| {
        use desktop_gpui::meeting::{ai, ai_view::AiViewServices, context, tools};
        let approvals = tools::Approvals::default();
        cx.set_global(AiViewServices {
            ai: Arc::new(
                ai::AiServices::new(
                    runtime.clone(),
                    Arc::new(|_| {
                        Box::pin(async {
                            Ok(Arc::new(ScreensModel) as Arc<dyn ai::ProviderAdapter>)
                        })
                    }),
                    tools::executor(runtime.clone(), approvals.clone(), None),
                    Arc::new(|_| false),
                )
                .unwrap(),
            ),
            context: context::resolver(runtime.clone()),
            approvals,
        });
    });
    open(
        cx,
        window,
        Route::Session(SessionId("s-ai-followup".into())),
    );
    capture(cx, window, out, "ai-brief-suggestion");
    click(cx, window, 888., 23.);
    click(cx, window, MEETING_INFO.0, MEETING_INFO.1);
    capture(cx, window, out, "ai-meeting-info");
    click(cx, window, ENHANCE_SAM.0, ENHANCE_SAM.1);
    capture(cx, window, out, "ai-contact-enhanced");
    type_text(cx, window, &["escape"]);
    click(cx, window, BRIEF.0, BRIEF.1);
    capture(cx, window, out, "ai-brief-streaming");
    BRIEF_GATE.store(true, std::sync::atomic::Ordering::Release);
    settle(cx);
    capture(cx, window, out, "ai-brief");

    // An uploaded recording of a blank note: its transcript lands, then the summary.
    let recording = profile.join("Vendor call.wav");
    std::fs::write(&recording, wav(8)).unwrap();
    let words = "Globex offered net sixty payment terms for the annual contract and support moves to a shared Slack channel next month so please send the signed order form by Friday";
    let transcriber: Transcriber = Arc::new(
        move |_: SessionId,
              _: PathBuf,
              progress: Progress,
              _: Warn,
              _: CancellationToken|
              -> Pending {
            use owhisper_interface::batch::{Alternatives, Channel, Response, Results, Word};
            let response = Response {
                metadata: serde_json::json!({}),
                results: Results {
                    channels: vec![Channel {
                        alternatives: vec![Alternatives {
                            transcript: words.into(),
                            confidence: 1.,
                            words: words
                                .split(' ')
                                .enumerate()
                                .map(|(index, word)| Word {
                                    word: word.to_lowercase(),
                                    start: index as f64 * 0.3,
                                    end: index as f64 * 0.3 + 0.25,
                                    confidence: 1.,
                                    channel: 0,
                                    speaker: Some(index / 12 % 2),
                                    punctuated_word: Some(word.into()),
                                })
                                .collect(),
                        }],
                    }],
                },
            };
            progress(1.);
            Box::pin(async move {
                Ok(Transcription {
                    response,
                    provider: "deepgram".into(),
                    model: "nova-3".into(),
                })
            })
        },
    );
    cx.update(|cx| cx.set_global(TranscriberGlobal(transcriber)));
    let upload = SessionId("s-blank-upload".into());
    open(cx, window, Route::Session(upload.clone()));
    cx.update(|cx| {
        let uploads = desktop_gpui::meeting::upload::Uploads::global(cx);
        uploads.update(cx, |uploads, cx| {
            uploads.drop_audio(runtime, upload.clone(), recording.clone(), cx)
        });
    });
    settle(cx);
    settle(cx);
    capture(cx, window, out, "ai-upload-summary");
    SUMMARY_GATE.store(true, std::sync::atomic::Ordering::Release);
    settle(cx);

    script(
        runtime,
        "DELETE FROM session_participants WHERE id IN ('sp-ai-maya', 'sp-ai-sam');
         DELETE FROM sessions WHERE id = 's-ai-followup';
         DELETE FROM events WHERE id = 'evt-ai-followup';
         DELETE FROM humans WHERE id = 'h-ai-sam';
         DELETE FROM session_documents WHERE id = 'd-ai-pricing-summary' OR session_id IN ('s-ai-followup', 's-blank-upload');
         DELETE FROM transcripts WHERE session_id = 's-blank-upload';",
    );
}

/// The memo's "Create a brief" row, the note menu's Meeting info row and Sam's sparkle, by
/// window position.
const BRIEF: (f32, f32) = (300., 104.);
const MEETING_INFO: (f32, f32) = (790., 65.);
const ENHANCE_SAM: (f32, f32) = (596., 199.);
/// App toasts, stacked and expanded on hover; a timeline delete's Undo toast and its undo;
/// and the multi-selection's delete confirmation.
fn toasts(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    use desktop_gpui::application::toasts::{self, Toast};
    open(cx, window, Route::Empty);
    // "Welcome to Anarlog" in the timeline: Delete Note is its menu's third item.
    right_click(cx, window, 100., 368.);
    type_text(cx, window, &["down", "down", "enter"]);
    capture(cx, window, out, "toast-undo");
    pointer(cx, window, 734., 560.);
    capture(cx, window, out, "toast-undo-hover");
    click(cx, window, 597., 558.);
    capture(cx, window, out, "toast-undo-restored");
    open(cx, window, Route::Empty);
    modified_click(cx, window, 100., 314.);
    modified_click(cx, window, 100., 368.);
    type_text(cx, window, &["backspace"]);
    capture(cx, window, out, "delete-confirm");
    type_text(cx, window, &["escape"]);
    // An empty spot in the note surface clears the selection.
    click(cx, window, 700., 150.);
    cx.update(|cx| {
        toasts::show(Toast::success("Note copied to clipboard").id("copied"), cx);
        toasts::show(
            Toast::error("Could not delete this note. Please try again.").id("failed"),
            cx,
        );
    });
    pointer(cx, window, 300., 300.);
    capture(cx, window, out, "toast-stack");
    pointer(cx, window, 734., 560.);
    capture(cx, window, out, "toast-stack-hover");
    cx.update(|cx| {
        toasts::dismiss("copied", cx);
        toasts::dismiss("failed", cx);
    });
    pointer(cx, window, 300., 300.);
}

fn modified_click(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    x: f32,
    y: f32,
) {
    let position = point(px(x), px(y));
    let modifiers = Modifiers::command();
    input(
        cx,
        window,
        [
            PlatformInput::MouseMove(MouseMoveEvent {
                position,
                pressed_button: None,
                modifiers,
            }),
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers,
                click_count: 1,
                first_mouse: false,
            }),
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers,
                click_count: 1,
            }),
        ],
    );
}

/// Microphone detection under Notifications, scrolled into view, then the excluded-apps search.
fn detection(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    open(cx, window, Route::Settings("notifications".into()));
    input(
        cx,
        window,
        [PlatformInput::Scroll(ScrollEvent {
            position: point(px(600.), px(300.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-2000.))),
            modifiers: Modifiers::default(),
            touch_phase: TouchPhase::Moved,
            source: ScrollSource::Pointer,
        })],
    );
    capture(cx, window, out, "settings-notifications-detection");
    click(cx, window, 570., 406.);
    capture(cx, window, out, "settings-notifications-exclude");
    type_text(cx, window, &["escape"]);
}

/// The memo's shared screens and meeting chat. The records are added late so no other
/// screen changes; `meeting-records.sqlite` lets the shipping app render the same rows. Its
/// parity server has no attachment files, so the compared tiles are empty on both sides.
fn meeting_records(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    runtime: &RuntimeHandle,
    profile: &std::path::Path,
    out: &std::path::Path,
) {
    sql(runtime, include_str!("screens/late/meeting-records.sql"));
    let copy = out.join("meeting-records.sqlite");
    let _ = std::fs::remove_file(&copy);
    let copy = copy.to_string_lossy().into_owned();
    let reply = runtime
        .submit(move |services| async move {
            sqlx::query("VACUUM INTO ?")
                .bind(copy)
                .execute(services.db.pool())
                .await
                .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
            Ok(())
        })
        .unwrap();
    futures::executor::block_on(reply.receive()).unwrap();
    open(cx, window, Route::Session(SessionId("s-sync".into())));
    capture(cx, window, out, "note-meeting-records");
    // Shipping's capture scrolls the shared screens to the same height.
    wheel(cx, window, (600., 350.), -233.);
    capture(cx, window, out, "note-meeting-records-scrolled");
    sql(
        runtime,
        "UPDATE session_documents SET deleted_at = '2026-11-17T11:00:00Z'
        WHERE kind IN ('meeting_chat', 'meeting_screen')",
    );
    // A captured frame on disk, which only the native capture can show. Attachments live
    // under UUID notes, which the seeded library does not use.
    const NOTE: &str = "6b3f7a52-1c4e-4f0a-9d2b-7e5c8a1f3d40";
    sql(
        runtime,
        "INSERT INTO sessions (id, title, created_at, started_at, ended_at, folder_path) VALUES
            ('6b3f7a52-1c4e-4f0a-9d2b-7e5c8a1f3d40', 'Design review', '2026-11-17T10:00:00Z',
             '2026-11-17T10:00:00Z', '', '')",
    );
    sql(
        runtime,
        "INSERT INTO session_documents (id, session_id, kind, body_format, body) VALUES
            ('6b3f7a52-1c4e-4f0a-9d2b-7e5c8a1f3d40', '6b3f7a52-1c4e-4f0a-9d2b-7e5c8a1f3d40',
             'note', 'prosemirror_json',
             '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\",\"content\":[{\"type\":\"text\",\"text\":\"Walked through the new onboarding flow.\"}]}]}')",
    );
    let attachments =
        desktop_gpui::editor::AttachmentService::new(runtime.clone(), profile.join("vault"));
    let mut frame = image::RgbaImage::from_pixel(640, 360, image::Rgba([241, 245, 249, 255]));
    for (x, y, pixel) in frame.enumerate_pixels_mut() {
        if (40..600).contains(&x) && (40..90).contains(&y) {
            *pixel = image::Rgba([30, 41, 59, 255]);
        } else if (40..380).contains(&x) && (130..320).contains(&y) {
            *pixel = image::Rgba([99, 102, 241, 255]);
        }
    }
    let mut png = Vec::new();
    frame
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let saved = futures::executor::block_on(attachments.import_bytes(
        SessionId(NOTE.into()),
        "shared-screen-2026-11-17T10-04-00-000Z.png".into(),
        png,
    ))
    .unwrap();
    let id = saved.id.clone();
    let reply = runtime
        .submit(move |services| async move {
            services
                .executor
                .execute_transaction(vec![
                    desktop_gpui::meeting::schedule::records::screen_statement(
                        NOTE,
                        &id,
                        Some(&desktop_gpui::meeting::schedule::apps::MeetingPlatform::Zoom),
                        640,
                        360,
                        "2026-11-17T10:04:00.000Z",
                        9,
                    ),
                ])
                .await
                .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
            Ok(())
        })
        .unwrap();
    futures::executor::block_on(reply.receive()).unwrap();
    open(cx, window, Route::Session(SessionId(NOTE.into())));
    capture(cx, window, out, "note-meeting-records-image");
    sql(
        runtime,
        "UPDATE sessions SET deleted_at = '2026-11-17T11:00:00Z'
        WHERE id = '6b3f7a52-1c4e-4f0a-9d2b-7e5c8a1f3d40'",
    );
    open(cx, window, Route::Session(SessionId("s-sync".into())));
}

/// Device authentication that declines, so lock screens stay up.
struct DecliningAuth;

impl desktop_gpui::platform::device_auth::DeviceAuth for DecliningAuth {
    fn available(&self) -> bool {
        true
    }

    fn authenticate(&self, _: &str) -> futures::channel::oneshot::Receiver<bool> {
        let (sender, receiver) = futures::channel::oneshot::channel();
        let _ = sender.send(false);
        receiver
    }
}

/// A locked note in the timeline, its menus and lock screen, then the app lock screen.
fn locks(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    runtime: &RuntimeHandle,
    out: &std::path::Path,
) {
    use desktop_gpui::{application::lock, platform::device_auth};
    cx.update(|cx| {
        cx.set_global(device_auth::Installed(Arc::new(DecliningAuth)));
        lock::refresh_availability(cx);
    });
    sql(
        runtime,
        "UPDATE sessions SET locked = 1 WHERE id = 's-standup'",
    );
    open(cx, window, Route::Session(SessionId("s-sync".into())));
    capture(cx, window, out, "lock-timeline");
    click(cx, window, 888., 23.);
    capture(cx, window, out, "lock-note-menu");
    type_text(cx, window, &["escape"]);
    right_click(cx, window, 100., 270.);
    capture(cx, window, out, "lock-context-menu");
    type_text(cx, window, &["escape"]);
    open(cx, window, Route::Session(SessionId("s-standup".into())));
    capture(cx, window, out, "lock-note");
    sql(
        runtime,
        "INSERT INTO app_settings (id, value_json) VALUES ('lock_app', 'true') ON CONFLICT(id) DO UPDATE SET value_json = 'true'",
    );
    settle(cx);
    capture(cx, window, out, "lock-app");
    sql(
        runtime,
        "UPDATE app_settings SET value_json = 'false' WHERE id = 'lock_app'",
    );
    sql(
        runtime,
        "UPDATE sessions SET locked = 0 WHERE id = 's-standup'",
    );
    cx.update(|cx| {
        cx.remove_global::<device_auth::Installed>();
        lock::refresh_availability(cx);
    });
    settle(cx);
}

/// The browser hand-off card at shipping's 340×500, without moving any window.
fn instruction(cx: &mut HeadlessAppContext, out: &std::path::Path) {
    use desktop_gpui::{
        application::instruction::{CARD, Instruction},
        product::cloud::host::Handoff,
    };
    let url = reqwest::Url::parse("https://anarlog.so/app/checkout").unwrap();
    for (name, handoff) in [
        ("instruction-sign-in", Handoff::SignIn),
        ("instruction-billing", Handoff::Billing),
        (
            "instruction-google-calendar",
            Handoff::Integration("google-calendar".into()),
        ),
        ("instruction-slack", Handoff::Integration("slack".into())),
    ] {
        let url = url.clone();
        let card = cx
            .open_window(size(px(CARD.0), px(CARD.1)), move |window, cx| {
                let main = window.window_handle();
                cx.new(|cx| {
                    let mut card = Instruction::new(main, cx);
                    card.show(handoff, url, None, cx);
                    card
                })
            })
            .unwrap();
        settle(cx);
        capture_window(cx, card.into(), out, name);
        if name == "instruction-sign-in" {
            // The callback-link fallback, then a pasted link.
            click_window(cx, card.into(), 170., 382.);
            capture_window(cx, card.into(), out, "instruction-sign-in-paste");
            cx.update_window(card.into(), |_, window, cx| {
                window.dispatch_keystroke(gpui::Keystroke::parse("a").unwrap(), cx)
            })
            .unwrap();
            settle(cx);
            capture_window(cx, card.into(), out, "instruction-sign-in-pasted");
        }
        cx.update_window(card.into(), |_, window, _| window.remove_window())
            .unwrap();
    }
}

fn capture_window(
    cx: &mut HeadlessAppContext,
    window: AnyWindowHandle,
    out: &std::path::Path,
    name: &str,
) {
    cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    let image = cx.capture_screenshot(window).unwrap();
    let path = out.join(format!("{name}.png"));
    image.save(&path).unwrap();
    println!("{}", path.display());
}

fn click_window(cx: &mut HeadlessAppContext, window: AnyWindowHandle, x: f32, y: f32) {
    let position = point(px(x), px(y));
    for event in [
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
    ] {
        cx.update_window(window, |_, window, cx| {
            window.draw(cx).clear(cx);
            window.dispatch_event(event, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
    settle(cx);
}

/// A model that proposes a summary edit, then answers once the edit is decided.
struct EditingModel;

impl desktop_gpui::meeting::ai::ProviderAdapter for EditingModel {
    fn stream(
        &self,
        request: desktop_gpui::meeting::ai::Request,
    ) -> futures::future::BoxFuture<
        'static,
        desktop_runtime::Result<
            futures::stream::BoxStream<
                'static,
                desktop_runtime::Result<desktop_gpui::meeting::ai::ProviderEvent>,
            >,
        >,
    > {
        use desktop_gpui::meeting::ai::{Part, ProviderEvent};
        use futures::StreamExt;
        let decided = request
            .messages
            .iter()
            .flat_map(|message| &message.parts)
            .any(|part| matches!(part, Part::Tool { .. }));
        Box::pin(async move {
            let event = if decided {
                ProviderEvent::Text("I shortened the **Design critique** summary.".into())
            } else {
                ProviderEvent::Tool {
                    call_id: "call-live-summary".into(),
                    name: "edit_summary".into(),
                    input: serde_json::json!({"content": "## Decisions\n\n- Ship the simplified onboarding next release.\n- The calendar step stays optional.\n\n## Follow-ups\n\nMina shares updated mocks by Friday."}),
                }
            };
            Ok(futures::stream::iter([Ok(event)]).boxed())
        })
    }
}

/// Chat edit review: the note's pending-edits banner, the Edit tab's diff, saved tool cards,
/// and a live summary edit waiting for review, then applied.
fn chat_edits(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    runtime: &RuntimeHandle,
    out: &std::path::Path,
) {
    let fixture = include_str!("screens/chat-edits/fixture.sql");
    let reply = runtime
        .submit(move |services| async move {
            sqlx::raw_sql(sqlx::AssertSqlSafe(fixture))
                .execute(services.db.pool())
                .await
                .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
            Ok(())
        })
        .unwrap();
    futures::executor::block_on(reply.receive()).unwrap();
    open(cx, window, Route::Session(SessionId("s-sync".into())));
    capture(cx, window, out, "note-pending-edits");
    open(cx, window, Route::Edit("p-sync-memo".into()));
    capture(cx, window, out, "edit-review-memo");
    open(cx, window, Route::Edit("p-critique-summary".into()));
    capture(cx, window, out, "edit-review-summary");

    open(cx, window, Route::Empty);
    chat(cx, window, |chat, window, cx| {
        chat.event(ChatEvent::Open, window, cx)
    });
    chat(cx, window, |chat, _, cx| {
        chat.select_group("g-edits".into(), cx)
    });
    capture(cx, window, out, "chat-tools");
    chat(cx, window, |chat, _, cx| {
        chat.toggle_activity("m-edits-2", cx)
    });
    // Shipping's capture clicks "Activity", which scrolls the thread to its top.
    wheel(cx, window, (500., 300.), 5000.);
    capture(cx, window, out, "chat-tools-activity");
    chat(cx, window, |chat, window, cx| {
        chat.new_chat(cx);
        chat.event(ChatEvent::Close, window, cx)
    });

    // A live edit: the review tab opens and the chat card waits with Decline / Apply.
    cx.update(|cx| {
        use desktop_gpui::meeting::{ai, ai_view::AiViewServices, context, tools};
        let approvals = tools::Approvals::default();
        let services = AiViewServices {
            ai: Arc::new(
                ai::AiServices::new(
                    runtime.clone(),
                    Arc::new(|_| {
                        Box::pin(async {
                            Ok(Arc::new(EditingModel) as Arc<dyn ai::ProviderAdapter>)
                        })
                    }),
                    tools::executor(runtime.clone(), approvals.clone(), None),
                    Arc::new(|_| false),
                )
                .unwrap(),
            ),
            context: context::resolver(runtime.clone()),
            approvals,
        };
        cx.set_global(services);
    });
    open(cx, window, Route::Session(SessionId("s-critique".into())));
    chat(cx, window, |chat, window, cx| {
        chat.event(ChatEvent::Open, window, cx)
    });
    write(cx, window, "Make the summary shorter");
    type_text(cx, window, &["enter"]);
    capture(cx, window, out, "chat-edit-pending");
    cx.update(|cx| {
        let workspace = window
            .update(cx, |root, _, _| root.workspace().downgrade())
            .unwrap();
        desktop_gpui::workspace::edit_review::decide(
            runtime.clone(),
            workspace,
            "call-live-summary".into(),
            true,
            cx,
        )
        .detach();
    });
    settle(cx);
    capture(cx, window, out, "note-edit-applied");
    // Returning to the note closed chat, as in shipping; reopened, the card shows the result.
    chat(cx, window, |chat, window, cx| {
        chat.event(ChatEvent::Open, window, cx)
    });
    capture(cx, window, out, "chat-edit-applied");
    chat(cx, window, |chat, window, cx| {
        chat.event(ChatEvent::Close, window, cx)
    });
}

/// ⌘F over the customer memo, then ⌘H's replace row; Esc closes the bar.
fn find_replace(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    open(cx, window, Route::Session(SessionId("s-customer".into())));
    click(cx, window, 850., 70.);
    type_text(cx, window, &["cmd-f"]);
    write(cx, window, "an");
    capture(cx, window, out, "note-find");
    type_text(cx, window, &["enter"]);
    capture(cx, window, out, "note-find-next");
    type_text(cx, window, &["cmd-h"]);
    capture(cx, window, out, "note-find-replace");
    type_text(cx, window, &["escape"]);
    // With focus outside the note (the workspace, as after a sidebar click), ⌘H still opens
    // replace, as shipping's window-wide listener does, instead of reaching Hide Anarlog.
    let window_handle: AnyWindowHandle = window.into();
    let handled = cx
        .update_window(window_handle, |root, window, cx| {
            let root = root.downcast::<ApplicationView>().unwrap();
            let focus = root.read(cx).workspace().focus_handle(cx);
            window.focus(&focus, cx);
            window.draw(cx).clear(cx);
            window.dispatch_keystroke(gpui::Keystroke::parse("cmd-h").unwrap(), cx)
        })
        .unwrap();
    assert!(handled, "⌘H over a note fell through to the app menu");
    settle(cx);
    capture(cx, window, out, "note-replace-from-workspace");
    type_text(cx, window, &["escape"]);
}

/// Insights badges, the changelog tab, the trial reminder's referral nudge and the timeline
/// revealing the opened note.
fn polish(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    use desktop_gpui::product::account::{self, Account, Claims, User};
    let draw = |cx: &mut HeadlessAppContext| {
        let window: AnyWindowHandle = window.into();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        settle(cx);
    };
    open(cx, window, Route::Settings("insights".into()));
    pointer(cx, window, 600., 400.);
    input(
        cx,
        window,
        [PlatformInput::Scroll(ScrollEvent {
            position: point(px(600.), px(400.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-1400.))),
            modifiers: Modifiers::default(),
            touch_phase: TouchPhase::Moved,
            source: ScrollSource::Pointer,
        })],
    );
    capture(cx, window, out, "settings-insights-badges");
    let badge = std::env::var("ANLG_SCREENS_BADGE")
        .ok()
        .and_then(|at| {
            at.split_once(',')
                .map(|(x, y)| (x.parse().unwrap(), y.parse().unwrap()))
        })
        .unwrap_or((470., 300.));
    click(cx, window, badge.0, badge.1);
    capture(cx, window, out, "badge-dialog");
    type_text(cx, window, &["escape"]);
    // Settings' back arrow returns to the workspace.
    click(cx, window, 94., 23.);
    open(cx, window, Route::Changelog);
    capture(cx, window, out, "changelog");
    // A fresh trial account: the started dialog, then the reminder with the referral nudge.
    let now = account::now();
    cx.update(|cx| {
        account::set(
            Account {
                user: Some(User {
                    id: "polish-trial-user".into(),
                    email: Some("sam@example.com".into()),
                }),
                claims: Some(Claims {
                    subscription_status: Some("trialing".into()),
                    trial_end: Some(now + 5 * 86_400 - 60),
                    has_payment_method: Some(false),
                    ..Claims::default()
                }),
                workspaces: Some(vec![].into()),
                trial_eligible: true,
                devices: None,
            },
            cx,
        )
    });
    cx.update(|cx| {
        window
            .update(cx, |root, _, cx| {
                root.trial_dialogs()
                    .clone()
                    .update(cx, |dialogs, cx| dialogs.set_can_refer(true, cx))
            })
            .unwrap()
    });
    open(cx, window, Route::Settings("billing".into()));
    click(cx, window, 455., 371.);
    capture(cx, window, out, "trial-reminder-referral");
    click(cx, window, 383., 310.);
    cx.update(|cx| account::set(Account::default(), cx));
    click(cx, window, 94., 23.);
    open(cx, window, Route::Empty);
    // The timeline scrolled away from the top, then a recent note opens and comes into view.
    pointer(cx, window, 100., 400.);
    scroll(cx, window, -2000.);
    open(cx, window, Route::Session(SessionId("s-sync".into())));
    draw(cx);
    capture(cx, window, out, "timeline-reveal");
}

struct Apps(Vec<&'static str>);

impl desktop_gpui::product::local::imports::InstalledApps for Apps {
    fn list(
        &self,
    ) -> desktop_runtime::Result<Vec<desktop_gpui::product::local::imports::InstalledApp>> {
        Ok(self
            .0
            .iter()
            .map(|name| desktop_gpui::product::local::imports::InstalledApp {
                id: (*name).into(),
                name: (*name).into(),
            })
            .collect())
    }
}

struct Reminders;

impl desktop_gpui::product::ticket::Reminders for Reminders {
    fn status(&self) -> desktop_gpui::product::ticket::Access {
        desktop_gpui::product::ticket::Access::NotDetermined
    }

    fn request(&self) -> desktop_gpui::product::ticket::Access {
        self.status()
    }
}

struct Github;

impl desktop_gpui::workspace::task::Github for Github {
    fn get(
        &self,
        path: String,
    ) -> futures::future::BoxFuture<'static, desktop_runtime::Result<serde_json::Value>> {
        let value = match path.as_str() {
            "/repos/acme/app/issues/42" => serde_json::json!({
                "title": "Calendar sync drops recurring events", "number": 42, "state": "open",
                "user": {"login": "maya"}, "created_at": "2026-11-02T10:00:00Z", "comments": 2,
                "labels": [{"name": "bug", "color": "d73a4a"}, {"name": "calendar", "color": "0e8a16"}],
                "assignees": [{"login": "jordan"}],
                "body": "Recurring events created in **Outlook** disappear after the second sync.\n\n- Happens on every account\n- Started after 1.4.10",
                "pull_request": null,
            }),
            "/repos/acme/app/issues/42/comments" => serde_json::json!([
                {"user": {"login": "jordan"}, "created_at": "2026-11-03T08:30:00Z", "body": "I can reproduce this with a weekly series."},
                {"user": {"login": "maya"}, "created_at": "2026-11-04T16:10:00Z", "body": "Fix is in #43."},
            ]),
            "/repos/acme/app/issues/43" => serde_json::json!({
                "title": "Keep recurrence rules when syncing", "number": 43, "state": "closed",
                "user": {"login": "maya"}, "created_at": "2026-11-04T15:00:00Z", "comments": 0,
                "labels": [], "assignees": [], "body": null,
                "pull_request": {"merged_at": "2026-11-05T09:00:00Z"},
            }),
            "/repos/acme/app/issues/43/comments" => serde_json::json!([]),
            _ => {
                return Box::pin(async {
                    Err(ServiceError::Failed(
                        "GitHub API returned 404 Not Found".into(),
                    ))
                });
            }
        };
        Box::pin(async move { Ok(value) })
    }
}

/// Imports with detected assistants and a connected Fathom, CRM rows, the Ticket page and a
/// Task tab, all against fixtures.
fn imports_crm_ticket(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    use desktop_gpui::product::{
        account::{self, Account, Claims, User},
        integrations::{self, fake::FakeIntegrations},
        local::imports,
    };
    let fake = Arc::new(FakeIntegrations::default());
    *fake.connections.lock().unwrap() = vec![
        FakeIntegrations::connection("fathom", "c-fathom", None),
        FakeIntegrations::connection("hubspot", "c-hubspot", None),
        FakeIntegrations::connection("salesforce", "c-salesforce", Some("reconnect_required")),
        FakeIntegrations::connection("attio", "c-attio", None),
    ];
    fake.responses.lock().unwrap().extend([
        (
            "/fathom/import-meetings".to_owned(),
            Ok(serde_json::json!({"files": [], "warnings": []})),
        ),
        (
            "/crm/search-contacts".to_owned(),
            Ok(serde_json::json!({"contacts": [{"name": "Maya Chen", "jobTitle": "Head of Product", "phone": "+1 555 0100", "url": "https://app.attio.com/maya"}]})),
        ),
    ]);
    cx.update(|cx| {
        cx.set_global(integrations::Installed(fake.clone()));
        cx.set_global(imports::InstalledAppsOverride(Arc::new(Apps(vec![
            "Granola", "Fathom", "Notion", "Krisp", "Otter", "Slack",
        ]))));
    });
    open(cx, window, Route::Settings("imports".into()));
    capture(cx, window, out, "settings-imports-detected-signed-out");
    let pro = Account {
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
        workspaces: Some(vec![].into()),
        ..Account::default()
    };
    cx.update(|cx| account::set(pro, cx));
    open(cx, window, Route::Settings("app".into()));
    open(cx, window, Route::Settings("imports".into()));
    click(cx, window, 300., 126.);
    capture(cx, window, out, "settings-imports-detected");
    click(cx, window, 858., 126.);
    capture(cx, window, out, "settings-imports-menu");
    click(cx, window, 500., 580.);
    open(cx, window, Route::Settings("crm".into()));
    capture(cx, window, out, "settings-crm-connected");
    open(cx, window, Route::Settings("todo".into()));
    capture(cx, window, out, "settings-ticket");
    click(cx, window, 300., 108.);
    capture(cx, window, out, "settings-ticket-github");
    // The settings back arrow returns to the workspace.
    click(cx, window, 90., 23.);
    open(
        cx,
        window,
        Route::Task("github_issue:acme/app#42 github_pr:acme/app#43".into()),
    );
    capture(cx, window, out, "task");
    open(cx, window, Route::Human("h-taylor".into()));
    capture(cx, window, out, "contacts-enrich");
    click(cx, window, 836., 24.);
    capture(cx, window, out, "contacts-enrich-result");
    cx.update(|cx| account::set(Account::default(), cx));
    open(cx, window, Route::Settings("todo".into()));
    click(cx, window, 300., 108.);
    capture(cx, window, out, "settings-ticket-signed-out");
}
/// The month grid's cell for `day`, top-left in window pixels (Sunday-first, as captured).
fn calendar_cell(day: chrono::NaiveDate) -> Option<(f32, f32)> {
    use chrono::Datelike;
    let today = chrono::Local::now().date_naive();
    let days = desktop_gpui::workspace::calendar::month_days(today.with_day(1)?, 0);
    let index = days.iter().position(|candidate| *candidate == day)?;
    let width = (WINDOW.0 - CALENDAR_LEFT) / 7.;
    let height = (WINDOW.1 - CALENDAR_TOP) / (days.len() / 7) as f32;
    Some((
        CALENDAR_LEFT + width * (index % 7) as f32,
        CALENDAR_TOP + height * (index / 7) as f32,
    ))
}

const CALENDAR_LEFT: f32 = 209.;
const CALENDAR_TOP: f32 = 81.;

/// Where an anchored popover of `size` lands for a chip whose top-left is `chip`: below it,
/// moved back inside the window's 16px margin.
fn popover_origin(chip: (f32, f32), size: (f32, f32)) -> (f32, f32) {
    (
        chip.0.min(WINDOW.0 - 16. - size.0),
        (chip.1 + 19.).min(WINDOW.1 - 16. - size.1),
    )
}

/// The event popover, its folder picker, the auto-add prompt it leads to, the manage view,
/// Escape, and a note's right-click menu.
fn calendar_depth(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    let today = chrono::Local::now().date_naive();
    // The seed places next week's critique at 06:00 UTC, seven UTC days from now.
    let critique = (chrono::Utc::now().date_naive() + chrono::Days::new(7))
        .and_hms_opt(6, 0, 0)
        .unwrap()
        .and_utc()
        .with_timezone(&chrono::Local)
        .date_naive();
    let Some(cell) = calendar_cell(critique) else {
        return;
    };
    open(cx, window, Route::Calendar);
    let chip = (cell.0 + 6., cell.1 + 38.);
    click(cx, window, chip.0 + 40., chip.1 + 7.);
    capture(cx, window, out, "calendar-event-popover");
    let main = popover_origin(chip, (320., 197.));
    click(cx, window, main.0 + 160., main.1 + 117.);
    type_text(cx, window, &["R", "e", "s", "e", "a", "r", "c", "h"]);
    capture(cx, window, out, "calendar-folder-picker");
    let picker = popover_origin(chip, (320., 163.));
    click(cx, window, picker.0 + 160., picker.1 + 98.);
    capture(cx, window, out, "calendar-auto-add");
    let prompt = popover_origin(chip, (320., 156.));
    click(cx, window, prompt.0 + 88., prompt.1 + 120.);
    click(cx, window, main.0 + 160., main.1 + 117.);
    capture(cx, window, out, "calendar-manage");
    type_text(cx, window, &["escape"]);
    capture(cx, window, out, "calendar-escape");
    if let Some(cell) = calendar_cell(today) {
        right_click(cx, window, cell.0 + 46., cell.1 + 45.5 + 2. * 17.);
        capture(cx, window, out, "calendar-note-menu");
        type_text(cx, window, &["escape"]);
    }
}

fn right_click(cx: &mut HeadlessAppContext, window: WindowHandle<ApplicationView>, x: f32, y: f32) {
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
                button: MouseButton::Right,
                position,
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            }),
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Right,
                position,
                modifiers: Modifiers::default(),
                click_count: 1,
            }),
        ],
    );
}

/// The empty state's key lift on hover and the live note's stop control on hover.
fn timeline_details(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    open(cx, window, Route::Empty);
    pointer(cx, window, 520., 253.);
    capture(cx, window, out, "empty-hover");
    let mut live = LiveMeeting::new(SessionId("s-standup".into()), SessionMode::Active);
    live.amplitude = 0.35;
    set_live(cx, Some(live));
    pointer(cx, window, 100., 322.);
    capture(cx, window, out, "timeline-live-hover");
    set_live(cx, None);
}

/// The note share popover in each account state, and a share with people, by window position.
fn sharing(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    use desktop_gpui::product::{
        account::{self, Account, Claims, User},
        sharing::{self, AccessEntry, Capability, EntryKind, GeneralAccess, ManagedShare},
    };
    let critique = || SessionId("s-critique".into());
    let signed_in = |claims: Claims| Account {
        user: Some(User {
            id: "7f1c2a9e-3b4d-4e5f-8a6b-9c0d1e2f3a4b".into(),
            email: Some("alex@example.com".into()),
        }),
        claims: Some(claims),
        workspaces: Some(Arc::from([])),
        ..Account::default()
    };
    let pro = || {
        signed_in(Claims {
            entitlements: vec!["hyprnote_pro".into()],
            subscription_status: Some("active".into()),
            current_period_end: Some(account::now() + 86_400),
            ..Claims::default()
        })
    };
    open(cx, window, Route::Session(critique()));
    click(cx, window, 832., 23.);
    capture(cx, window, out, "share-signed-out");
    type_text(cx, window, &["escape"]);
    cx.update(|cx| account::set(signed_in(Claims::default()), cx));
    click(cx, window, 832., 23.);
    capture(cx, window, out, "share-upgrade");
    type_text(cx, window, &["escape"]);
    cx.update(|cx| account::set(pro(), cx));
    click(cx, window, 832., 23.);
    capture(cx, window, out, "share-draft");
    type_text(cx, window, &["escape"]);
    let entry = |kind, id: &str, email: Option<&str>, capability| AccessEntry {
        kind,
        id: id.into(),
        user_id: None,
        email: email.map(Into::into),
        capability,
    };
    cx.update(|cx| {
        sharing::remember(
            critique(),
            Some(ManagedShare {
                share_id: "73d1e15a-9117-4260-8c78-5081a0c5012e".into(),
                general: GeneralAccess::Workspace("5d8f2c1a-7b3e-4f6a-9c2d-1e0b7a6f5c4d".into()),
                has_active_link: false,
                access: Arc::from([
                    entry(
                        EntryKind::Grant,
                        "g-maya",
                        Some("maya@northwind.example"),
                        Capability::Commenter,
                    ),
                    entry(
                        EntryKind::Invitation,
                        "i-jordan",
                        Some("jordan@contoso.example"),
                        Capability::Viewer,
                    ),
                    entry(
                        EntryKind::Request,
                        "r-sam",
                        Some("sam@contoso.example"),
                        Capability::Editor,
                    ),
                ]),
                conflict: false,
            }),
            cx,
        )
    });
    click(cx, window, 832., 23.);
    capture(cx, window, out, "share-managed");
    type_text(cx, window, &["escape"]);
    cx.update(|cx| {
        sharing::remember(critique(), None, cx);
        account::set(Account::default(), cx);
    });
    // Shared notes render from the account's local cache.
    open(
        cx,
        window,
        Route::SharedSession("2b6e4f1c-8a3d-4c7e-9f0a-5d1b3e7c9a2f".into()),
    );
    capture(cx, window, out, "shared-note-signed-out");
    cx.update(|cx| account::set(pro(), cx));
    // Every account-aware view reloads after sign-in; let the cache read land.
    settle(cx);
    settle(cx);
    capture(cx, window, out, "shared-note");
    open(
        cx,
        window,
        Route::SharedSession("9c3a7e2b-1d4f-4a6c-8e5b-0f2d6a9c3e1b".into()),
    );
    capture(cx, window, out, "shared-note-revoked");
    // The Research folder's Default access row and its share dialog, signed in.
    open(cx, window, Route::Folders);
    click(cx, window, 100., 218.);
    capture(cx, window, out, "folder-default-access");
    click(cx, window, 840., 24.);
    capture(cx, window, out, "folder-share-pro");
    click(cx, window, 10., 590.);
    cx.update(|cx| account::set(signed_in(Claims::default()), cx));
    click(cx, window, 840., 24.);
    capture(cx, window, out, "folder-share-free");
    click(cx, window, 10., 590.);
    cx.update(|cx| account::set(Account::default(), cx));
}

/// The note's summary states, memo templates and transcript editing (`120-editor.sql`).
fn note_depth(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    // The active summary tab opens the template picker.
    open(cx, window, Route::Session(SessionId("s-critique".into())));
    click(cx, window, 270., 23.);
    capture(cx, window, out, "summary-picker");
    type_text(cx, window, &["escape"]);
    // A transcript without a summary offers to generate one.
    open(cx, window, Route::Session(SessionId("s-vendor".into())));
    capture(cx, window, out, "summary-generate");
    // Its transcript scrolls; the active Transcript tab toggles editing, and a speaker label
    // opens the speaker picker.
    click(cx, window, TRANSCRIPT_TAB_AFTER.0, TRANSCRIPT_TAB_AFTER.1);
    capture(cx, window, out, "transcript-vendor");
    click(cx, window, 330., 23.);
    capture(cx, window, out, "transcript-edit");
    click(cx, window, 290., 68.);
    capture(cx, window, out, "transcript-speaker");
    type_text(cx, window, &["escape"]);
    click(cx, window, 330., 23.);
    // A note without a memo document: the empty editor with suggested templates.
    open(cx, window, Route::Session(SessionId("s-intro".into())));
    capture(cx, window, out, "memo-templates");
}

/// The memo's templates in a narrow window: short titles still show in full, and a long one
/// ends in an ellipsis. Late, because the narrow window collapses the sidebar.
fn memo_templates_narrow(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    runtime: &RuntimeHandle,
    out: &std::path::Path,
) {
    let resize = |cx: &mut HeadlessAppContext, width: f32| {
        cx.update_window(window.into(), |_, window, cx| {
            window.resize(size(px(width), px(WINDOW.1)));
            window.bounds_changed(cx);
        })
        .unwrap();
        settle(cx);
    };
    // Zoom ends in Settings; its back arrow returns to the workspace.
    click(cx, window, 94., 23.);
    open(cx, window, Route::Session(SessionId("s-intro".into())));
    resize(cx, 420.);
    capture(cx, window, out, "memo-templates-narrow");
    sql(
        runtime,
        "UPDATE templates SET title = 'Weekly team sync with design, research, platform and customer success' WHERE id = 't-1-sync'",
    );
    settle(cx);
    capture(cx, window, out, "memo-templates-narrow-long");
    sql(
        runtime,
        "UPDATE templates SET title = 'Weekly team sync' WHERE id = 't-1-sync'",
    );
    resize(cx, WINDOW.0);
    // Reopens the sidebar the narrow window collapsed.
    click(cx, window, 94., 23.);
}

/// The note header's folder picker, Meeting info, Export modal and the summary tab's menu.
fn note_header(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    open(cx, window, Route::Session(SessionId("s-sync".into())));
    click(cx, window, 260., 23.);
    capture(cx, window, out, "folder-picker");
    type_text(cx, window, &["n", "e", "w"]);
    capture(cx, window, out, "folder-picker-create");
    type_text(cx, window, &["escape"]);
    // The more menu's Meeting info submenu: the date and participants of a note without an event.
    click(cx, window, 888., 23.);
    click(cx, window, 800., 66.);
    capture(cx, window, out, "meeting-info");
    click(cx, window, 560., 139.);
    type_text(cx, window, &["p", "r"]);
    capture(cx, window, out, "meeting-info-search");
    type_text(cx, window, &["escape", "escape"]);
    // A calendar event's details with its Join link.
    open(cx, window, Route::Session(SessionId("s-hiring".into())));
    click(cx, window, 888., 23.);
    click(cx, window, 800., 66.);
    capture(cx, window, out, "meeting-info-event");
    type_text(cx, window, &["escape"]);
    open(cx, window, Route::Session(SessionId("s-sync".into())));
    click(cx, window, 888., 23.);
    click(cx, window, 800., 180.);
    capture(cx, window, out, "export-modal");
    type_text(cx, window, &["escape"]);
    // The active summary tab's right-click menu.
    open(cx, window, Route::Session(SessionId("s-critique".into())));
    right_click(cx, window, 270., 23.);
    capture(cx, window, out, "summary-tab-menu");
    click(cx, window, 700., 400.);
}

/// Account, plan and seat states, seeded through the same app-wide value the cloud service
/// publishes after sign-in.
fn account_states(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    use desktop_gpui::product::account::{
        self, Account, Claims, Devices, Recorder, User, Workspace, WorkspaceTier,
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
        Some(Devices {
            used: 1,
            max: 3,
            recorder: None,
        }),
    );
    let pro = signed_in(
        Claims {
            entitlements: vec!["hyprnote_pro".into()],
            subscription_status: Some("active".into()),
            current_period_end: Some(now + 20 * day),
            ..Claims::default()
        },
        vec![],
        Some(Devices {
            used: 2,
            max: 3,
            recorder: Some(Recorder {
                fingerprint: "fp-macbook".into(),
                name: "Alex's MacBook Pro".into(),
            }),
        }),
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
        ("dictation-pro", pro.clone(), "dictation"),
        ("meetings-pro", pro.clone(), "meetings"),
        ("billing-team-seat", seat.clone(), "billing"),
        ("meetings-team-seat", seat, "meetings"),
    ] {
        cx.update(|cx| account::set(state, cx));
        open(cx, window, Route::Settings(page.into()));
        // A new trial opens the app-wide trial dialogs: started, then the payment reminder.
        if name == "billing-trial" {
            capture(cx, window, out, "trial-started");
            click(cx, window, 455., 371.);
            capture(cx, window, out, "trial-reminder");
            click(cx, window, 383., 371.);
        }
        capture(cx, window, out, &format!("settings-{name}"));
    }
    // Recording a dictation shortcut, then a press on Pro-gated content.
    cx.update(|cx| account::set(pro, cx));
    open(cx, window, Route::Settings("dictation".into()));
    click(cx, window, 817., 222.);
    capture(cx, window, out, "settings-dictation-recording");
    type_text(cx, window, &["escape"]);
    cx.update(|cx| account::set(free.clone(), cx));
    open(cx, window, Route::Settings("dictation".into()));
    click(cx, window, 600., 180.);
    capture(cx, window, out, "settings-plan-required");
    cx.update(|cx| account::set(free, cx));
    open(cx, window, Route::Settings("account".into()));
    click(cx, window, 850., 100.);
    capture(cx, window, out, "settings-account-sign-out");
    cx.update(|cx| account::set(Account::default(), cx));
}

/// Settings › Developers over a temporary home: the installed CLI, the skill menu, a new
/// webhook, and the Cloud API section signed out, on Free and on Pro.
fn developers(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    runtime: &RuntimeHandle,
    out: &std::path::Path,
) {
    use desktop_gpui::product::{
        account::{self, Account, Claims, User},
        local::{developers::DeveloperTools, developers_view::Developers},
    };
    let home = tempfile::tempdir().unwrap();
    let bundled = home.path().join("resources/anarlog");
    std::fs::create_dir_all(bundled.parent().unwrap()).unwrap();
    std::fs::write(&bundled, b"fixture-cli").unwrap();
    std::fs::create_dir_all(home.path().join(".claude")).unwrap();
    std::fs::create_dir_all(home.path().join(".codex/skills/anarlog")).unwrap();
    std::fs::write(home.path().join(".codex/skills/anarlog/SKILL.md"), "skill").unwrap();
    let tools = DeveloperTools {
        bundled_cli: bundled,
        home: home.path().into(),
        command_name: "anarlog",
        version: "0.1.0",
    };
    tools.install_cli().unwrap();
    cx.update(|cx| {
        cx.set_global(Developers {
            runtime: runtime.clone(),
            tools,
        })
    });
    open(cx, window, Route::Settings("developers".into()));
    capture(cx, window, out, "developers-installed");
    click(cx, window, 828., 245.);
    capture(cx, window, out, "developers-skill-menu");
    click(cx, window, 400., 120.);
    click(cx, window, 400., 490.);
    type_text(
        cx,
        window,
        &[
            "h", "t", "t", "p", "s", ":", "/", "/", "h", "o", "o", "k", "s", ".", "t", "e", "s",
            "t",
        ],
    );
    click(cx, window, 733., 488.);
    capture(cx, window, out, "developers-webhook");
    let user = Some(User {
        id: "7f1c2a9e-3b4d-4e5f-8a6b-9c0d1e2f3a4b".into(),
        email: Some("alex@example.com".into()),
    });
    let free = Account {
        user: user.clone(),
        claims: Some(Claims::default()),
        workspaces: Some(vec![].into()),
        ..Account::default()
    };
    let pro = Account {
        claims: Some(Claims {
            entitlements: vec!["hyprnote_pro".into()],
            subscription_status: Some("active".into()),
            current_period_end: Some(account::now() + 86_400),
            ..Claims::default()
        }),
        ..free.clone()
    };
    for (name, state) in [("developers-free", free), ("developers-pro", pro)] {
        cx.update(|cx| account::set(state, cx));
        open(cx, window, Route::Settings("developers".into()));
        capture(cx, window, out, name);
    }
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

/// Transcript and audio file flows: saved audio without a transcript, a running and a failed
/// re-transcription (with fake providers), the note menu's recording rows, the Transcript tab
/// and timeline menus, segment selection, and the audio drop overlay.
fn transcript_files(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    profile: &std::path::Path,
    out: &std::path::Path,
) {
    let audio = profile.join("vault/sessions/s-voice-memo");
    std::fs::create_dir_all(&audio).unwrap();
    std::fs::write(audio.join("audio.wav"), wav(20)).unwrap();
    open(cx, window, Route::Session(SessionId("s-voice-memo".into())));
    click(cx, window, TRANSCRIPT_TAB_LIVE.0, TRANSCRIPT_TAB_LIVE.1);
    capture(cx, window, out, "transcript-audio-empty");
    click(cx, window, 888., 23.);
    capture(cx, window, out, "note-menu-audio");
    type_text(cx, window, &["escape"]);

    let waiting: Transcriber = Arc::new(
        |_: SessionId,
         _: PathBuf,
         progress: Progress,
         _: Warn,
         cancellation: CancellationToken|
         -> Pending {
            Box::pin(async move {
                progress(0.42);
                cancellation.cancelled().await;
                Err(ServiceError::Cancelled)
            })
        },
    );
    cx.update(|cx| cx.set_global(TranscriberGlobal(waiting)));
    click(cx, window, RETRANSCRIBE.0, RETRANSCRIBE.1);
    capture(cx, window, out, "transcript-batching");
    click(cx, window, STOP_TRANSCRIPTION.0, STOP_TRANSCRIPTION.1);
    let failing: Transcriber = Arc::new(
        |_: SessionId, _: PathBuf, _: Progress, _: Warn, _: CancellationToken| -> Pending {
            Box::pin(async {
                Err(ServiceError::Failed(
                    "Deepgram rejected the API key (401).".into(),
                ))
            })
        },
    );
    cx.update(|cx| cx.set_global(TranscriberGlobal(failing)));
    click(cx, window, RETRANSCRIBE.0, RETRANSCRIBE.1);
    capture(cx, window, out, "transcript-failed");

    open(
        cx,
        window,
        Route::Session(SessionId("s-blank-upload".into())),
    );
    click(cx, window, 888., 23.);
    capture(cx, window, out, "note-menu-upload");
    type_text(cx, window, &["escape"]);
    // One audio file dragged over the memo shows the drop overlay; it leaves without dropping.
    let position = point(px(600.), px(300.));
    let paths = gpui::ExternalPaths(vec![profile.join("Interview.m4a")].into());
    input(
        cx,
        window,
        [
            PlatformInput::FileDrop(gpui::FileDropEvent::Entered { position, paths }),
            PlatformInput::FileDrop(gpui::FileDropEvent::Pending { position }),
        ],
    );
    capture(cx, window, out, "note-audio-drop");
    input(
        cx,
        window,
        [PlatformInput::FileDrop(gpui::FileDropEvent::Exited)],
    );

    open(cx, window, Route::Session(SessionId("s-retro".into())));
    click(cx, window, TRANSCRIPT_TAB_AFTER.0, TRANSCRIPT_TAB_AFTER.1);
    right_click(cx, window, 340., 23.);
    capture(cx, window, out, "transcript-tab-menu");
    right_click(cx, window, 600., 73.);
    capture(cx, window, out, "timeline-menu");
    click(cx, window, 600., 400.);
    // In edit mode a click selects the second segment; ⌘⇧↓ extends to the last one.
    click(cx, window, 340., 23.);
    click(cx, window, 230., 191.);
    type_text(cx, window, &["cmd-shift-down"]);
    capture(cx, window, out, "transcript-selection");
    type_text(cx, window, &["escape"]);
    click(cx, window, 340., 23.);
}

type Pending = futures::future::BoxFuture<'static, desktop_runtime::Result<Transcription>>;

/// The empty transcript's Re-transcribe and the batching screen's Stop transcription.
const RETRANSCRIBE: (f32, f32) = (495., 422.);
const STOP_TRANSCRIPTION: (f32, f32) = (559., 425.);

/// The transcript tab of a finished meeting (Summary, Memos, Transcript) and of a live one
/// (Memos, Transcript), by window position.
const TRANSCRIPT_TAB_AFTER: (f32, f32) = (382., 23.);
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

/// The dictation panel (shipping's floating bar in dictation mode) in each of its states,
/// rendered from the same panel state the dictation host publishes; nothing records.
fn dictation(cx: &mut HeadlessAppContext, out: &std::path::Path) {
    use desktop_gpui::dictation::{
        controller::Phase,
        overlay::{self, DictationBar, Panel},
    };
    let listening = Panel {
        owner: "system-dictation:screens".into(),
        phase: Phase::Recording,
        amplitude: 0.45,
        microphone: "Default microphone".into(),
        preview_enabled: true,
        ..Panel::default()
    };
    let words = Panel {
        text: "Let's move the launch review to Thursday so design can join, and".into(),
        partial: "send the agenda to".into(),
        expanded: true,
        ..listening.clone()
    };
    let states = [
        (
            "dictation-starting",
            Panel {
                phase: Phase::Starting,
                ..listening.clone()
            },
            false,
        ),
        ("dictation-listening", listening.clone(), false),
        ("dictation-hover", listening.clone(), true),
        (
            "dictation-finishing",
            Panel {
                phase: Phase::Transcribing,
                ..listening.clone()
            },
            false,
        ),
        (
            "dictation-preview-listening",
            Panel {
                expanded: true,
                ..listening.clone()
            },
            false,
        ),
        ("dictation-preview-words", words.clone(), false),
        (
            "dictation-preview-off",
            Panel {
                preview_enabled: false,
                expanded: true,
                ..listening.clone()
            },
            false,
        ),
        (
            "dictation-preview-unavailable",
            Panel {
                preview_unavailable: true,
                expanded: true,
                ..listening.clone()
            },
            false,
        ),
        (
            "dictation-preview-finishing",
            Panel {
                phase: Phase::Transcribing,
                ..words
            },
            false,
        ),
    ];
    for (name, panel, hover) in states {
        let size = overlay::size(panel.expanded);
        let bar = cx
            .open_window(size, move |_, cx| cx.new(|_| DictationBar::new(panel)))
            .unwrap();
        settle(cx);
        let window: AnyWindowHandle = bar.into();
        // The Done control's center when hovered, otherwise outside the panel.
        let position = if hover {
            point(px(39.), size.height - px(23.))
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

/// Starters, workflows and the chat automation, by their sidebar rows.
fn automations(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    for (name, y) in [
        ("automations-drive", 134.),
        ("automations-slack", 170.),
        ("automations-markdown", 278.),
        ("automations-workflow", 355.),
        ("automations-archive", 409.),
        ("automations-blank", 463.),
    ] {
        click(cx, window, 100., y);
        pointer(cx, window, 500., 590.);
        capture(cx, window, out, name);
        // The config below the steps, scrolled into view.
        if matches!(name, "automations-drive" | "automations-archive") {
            input(
                cx,
                window,
                [PlatformInput::Scroll(ScrollEvent {
                    position: point(px(500.), px(300.)),
                    delta: ScrollDelta::Pixels(point(px(0.), px(-1000.))),
                    modifiers: Modifiers::default(),
                    touch_phase: TouchPhase::Moved,
                    source: ScrollSource::Pointer,
                })],
            );
            capture(cx, window, out, &format!("{name}-end"));
        }
    }
    // The workflow's trigger select and actions menu, then a row's context menu.
    click(cx, window, 100., 355.);
    click(cx, window, 466., 266.);
    capture(cx, window, out, "automations-trigger-menu");
    click(cx, window, 466., 266.);
    click(cx, window, 891., 24.);
    capture(cx, window, out, "automations-actions-menu");
    click(cx, window, 891., 24.);
    input(
        cx,
        window,
        [PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Right,
            position: point(px(100.), px(409.)),
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        })],
    );
    capture(cx, window, out, "automations-context");
    click(cx, window, 10., 590.);
    // With Pro, the enabled workflow disables and enables again from its header.
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
    click(cx, window, 100., 355.);
    click(cx, window, 840., 24.);
    pointer(cx, window, 500., 590.);
    capture(cx, window, out, "automations-pro-disabled");
    click(cx, window, 840., 24.);
    pointer(cx, window, 500., 590.);
    capture(cx, window, out, "automations-pro-enabled");
    cx.update(|cx| desktop_gpui::product::account::set(Default::default(), cx));
    // Last: opening the chat automation stores its workflow at the top of the list.
    click(cx, window, 100., 517.);
    pointer(cx, window, 500., 590.);
    capture(cx, window, out, "automations-chat");
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

/// The composer, mentions, message interactions and reply states, by window position for the
/// floating panel over the empty tab.
fn chat_composer(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    runtime: &RuntimeHandle,
    out: &std::path::Path,
) {
    // A transcription provider hides the "provider needed" toast that covers the composer.
    sql(
        runtime,
        "INSERT INTO app_settings (id, value_json) VALUES ('current_stt_provider', '\"deepgram\"'), ('current_stt_model', '\"nova-3\"')",
    );
    open(cx, window, Route::Empty);
    chat(cx, window, |chat, window, cx| {
        chat.new_chat(cx);
        chat.event(ChatEvent::Open, window, cx)
    });
    // Voice input: without a dictation service it fails like shipping without a microphone;
    // a stand-in service shows the recording row. The app's own service is removed first, so
    // no audio device is opened and no provider is called.
    cx.update(|cx| {
        if cx.has_global::<desktop_gpui::chat::voice::Voice>() {
            cx.remove_global::<desktop_gpui::chat::voice::Voice>();
        }
    });
    click(cx, window, 858., 561.);
    capture(cx, window, out, "chat-voice-unavailable");
    cx.update(|cx| {
        cx.set_global(desktop_gpui::chat::voice::Voice(std::sync::Arc::new(
            StandInVoice,
        )))
    });
    // The toast sits over the microphone; the first click dismisses it.
    click(cx, window, 858., 561.);
    click(cx, window, 858., 561.);
    capture(cx, window, out, "chat-voice-recording");
    chat(cx, window, |chat, window, cx| {
        chat.event(ChatEvent::Close, window, cx);
        chat.event(ChatEvent::Open, window, cx)
    });
    write(cx, window, "Summarize the weekly sync");
    capture(cx, window, out, "chat-composer-typed");
    type_text(cx, window, &["shift-enter"]);
    write(cx, window, "and list the owners");
    capture(cx, window, out, "chat-composer-multiline");
    type_text(cx, window, &["cmd-a", "backspace"]);
    write(cx, window, "Prep for @");
    capture(cx, window, out, "chat-mention-empty");
    type_text(cx, window, &["enter"]);
    write(cx, window, "and @");
    type_text(cx, window, &["down", "enter"]);
    capture(cx, window, out, "chat-mention-chips");
    type_text(cx, window, &["cmd-a", "backspace"]);
    write(cx, window, "@wee");
    capture(cx, window, out, "chat-mention-search");
    type_text(cx, window, &["cmd-a", "backspace"]);
    // Files dropped on the composer become chips: an image thumbnail, then a file icon.
    let files = tempfile::tempdir().unwrap();
    let photo = files.path().join("whiteboard.png");
    image::RgbImage::from_pixel(16, 16, image::Rgb([59, 130, 246]))
        .save(&photo)
        .unwrap();
    let notes = files.path().join("Q3 planning notes and owners.pdf");
    std::fs::write(&notes, b"%PDF-1.4").unwrap();
    write(cx, window, "Review ");
    // GPUI keeps hover suppressed after typing until the pointer moves, which it does on its
    // way to the Finder window the files come from.
    pointer(cx, window, 300., 562.);
    file_drop(cx, window, (300., 562.), vec![photo, notes]);
    capture(cx, window, out, "chat-attachments");
    type_text(cx, window, &["cmd-a", "backspace"]);
    write(cx, window, "Prep for @");
    type_text(cx, window, &["enter"]);
    write(cx, window, "and @");
    type_text(cx, window, &["down", "enter"]);
    chat(cx, window, |chat, window, cx| {
        chat.event(ChatEvent::OpenRightPanel, window, cx)
    });
    capture(cx, window, out, "chat-right-typed");
    chat(cx, window, |chat, window, cx| {
        chat.composer()
            .update(cx, |composer, cx| composer.clear(cx));
        chat.select_group("g-launch".into(), cx);
        chat.event(ChatEvent::Open, window, cx)
    });
    pointer(cx, window, 455., 300.);
    capture(cx, window, out, "chat-thread-hover");
    // The table's controls where shipping's capture scrolls them: copy, download, fullscreen.
    wheel(cx, window, (500., 300.), 134.);
    capture(cx, window, out, "chat-thread-table");
    click(cx, window, 789., 332.);
    capture(cx, window, out, "chat-table-copy");
    click(cx, window, 815., 332.);
    capture(cx, window, out, "chat-table-download");
    click(cx, window, 815., 332.);
    click(cx, window, 841., 332.);
    capture(cx, window, out, "chat-table-fullscreen");
    type_text(cx, window, &["escape"]);
    drag(cx, window, (258., 146.), (545., 198.));
    capture(cx, window, out, "chat-selection");
    chat(cx, window, |chat, _, cx| {
        chat.select_group("g-hiring".into(), cx);
    });
    capture(cx, window, out, "chat-code");
    chat(cx, window, |chat, _, cx| {
        chat.preview_reply("", "", None, cx);
    });
    capture(cx, window, out, "chat-thinking");
    chat(cx, window, |chat, _, cx| {
        chat.preview_reply(
            "Checking the debrief notes for owners",
            "The debrief raised **two** questions about",
            None,
            cx,
        );
    });
    capture(cx, window, out, "chat-streaming");
    chat(cx, window, |chat, _, cx| {
        chat.preview_reply(
            "",
            "The debrief raised **two** questions about",
            Some(Ok(())),
            cx,
        );
    });
    capture(cx, window, out, "chat-stopped");
    chat(cx, window, |chat, _, cx| {
        chat.preview_reply(
            "",
            "",
            Some(Err(
                "Request failed: the prompt (5120 tokens) exceeds the context length of 4096"
                    .into(),
            )),
            cx,
        );
    });
    pointer(cx, window, 300., 520.);
    capture(cx, window, out, "chat-error");
    chat(cx, window, |chat, window, cx| {
        chat.new_chat(cx);
        chat.event(ChatEvent::Close, window, cx)
    });
    // A summary shown in the note adds shipping's rewrite suggestions; a live meeting swaps
    // in its own.
    open(cx, window, Route::Session(SessionId("s-critique".into())));
    chat(cx, window, |chat, window, cx| {
        chat.event(ChatEvent::Open, window, cx)
    });
    capture(cx, window, out, "chat-summary-suggestions");
    chat(cx, window, |chat, window, cx| {
        chat.event(ChatEvent::Close, window, cx)
    });
    let standup = SessionId("s-standup".into());
    set_live(
        cx,
        Some(LiveMeeting::new(standup.clone(), SessionMode::Active)),
    );
    open(cx, window, Route::Session(standup));
    chat(cx, window, |chat, window, cx| {
        chat.event(ChatEvent::Open, window, cx)
    });
    capture(cx, window, out, "chat-live-suggestions");
    chat(cx, window, |chat, window, cx| {
        chat.event(ChatEvent::Close, window, cx)
    });
    set_live(cx, None);
    // A provider that needs a key it does not have shows the setup prompt, not the composer.
    sql(
        runtime,
        "UPDATE app_settings SET value_json = '\"openai\"' WHERE id = 'current_llm_provider'",
    );
    open(cx, window, Route::Empty);
    chat(cx, window, |chat, window, cx| {
        chat.event(ChatEvent::Open, window, cx)
    });
    capture(cx, window, out, "chat-missing-key");
    chat(cx, window, |chat, window, cx| {
        chat.event(ChatEvent::Close, window, cx)
    });
    sql(
        runtime,
        "UPDATE app_settings SET value_json = '\"ollama\"' WHERE id = 'current_llm_provider'",
    );
    // The reveal halfway through its 180ms.
    cx.update(|cx| {
        window
            .update(cx, |root, window, cx| {
                let chat = root.workspace().read(cx).chat().clone();
                chat.update(cx, |chat, cx| chat.event(ChatEvent::Open, window, cx))
            })
            .unwrap()
    });
    cx.advance_clock(Duration::from_millis(60));
    cx.run_until_parked();
    capture(cx, window, out, "chat-reveal");
    settle(cx);
    chat(cx, window, |chat, window, cx| {
        chat.event(ChatEvent::Close, window, cx)
    });
    sql(
        runtime,
        "DELETE FROM app_settings WHERE id IN ('current_stt_provider', 'current_stt_model')",
    );
}

fn wheel(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    at: (f32, f32),
    y: f32,
) {
    input(
        cx,
        window,
        [PlatformInput::Scroll(ScrollEvent {
            position: point(px(at.0), px(at.1)),
            delta: ScrollDelta::Pixels(point(px(0.), px(y))),
            modifiers: Modifiers::default(),
            touch_phase: TouchPhase::Moved,
            source: ScrollSource::Pointer,
        })],
    );
}

struct StandInVoice;

impl desktop_gpui::chat::voice::VoiceInput for StandInVoice {
    fn start(
        &self,
    ) -> (
        Box<dyn desktop_gpui::chat::voice::VoiceRecording>,
        futures::future::BoxFuture<'static, desktop_runtime::Result<()>>,
    ) {
        (Box::new(StandInVoice), Box::pin(async { Ok(()) }))
    }
}

impl desktop_gpui::chat::voice::VoiceRecording for StandInVoice {
    fn stop(
        self: Box<Self>,
    ) -> futures::future::BoxFuture<'static, desktop_runtime::Result<String>> {
        Box::pin(async { Ok("and the release dates".into()) })
    }
}

/// Runs a statement through the executor, so reactive watches see the change.
/// Runs a multi-statement fixture script.
fn script(runtime: &RuntimeHandle, script: &'static str) {
    let reply = runtime
        .submit(move |services| async move {
            sqlx::raw_sql(sqlx::AssertSqlSafe(script))
                .execute(services.db.pool())
                .await
                .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
            Ok(())
        })
        .unwrap();
    futures::executor::block_on(reply.receive()).unwrap();
}

fn sql(runtime: &RuntimeHandle, statement: &'static str) {
    let reply = runtime
        .submit(move |services| async move {
            services
                .executor
                .execute(statement.into(), vec![])
                .await
                .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
            Ok(())
        })
        .unwrap();
    futures::executor::block_on(reply.receive()).unwrap();
}

/// Types text through the focused element's input handler, as an input method would.
fn write(cx: &mut HeadlessAppContext, window: WindowHandle<ApplicationView>, text: &str) {
    let window: AnyWindowHandle = window.into();
    for character in text.chars() {
        let keystroke = gpui::Keystroke {
            modifiers: Modifiers::default(),
            key: character.to_lowercase().to_string(),
            key_char: Some(character.to_string()),
        };
        cx.update_window(window, |_, window, cx| {
            window.draw(cx).clear(cx);
            window.dispatch_keystroke(keystroke, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
    settle(cx);
}

fn drag(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    from: (f32, f32),
    to: (f32, f32),
) {
    let (from, to) = (point(px(from.0), px(from.1)), point(px(to.0), px(to.1)));
    input(
        cx,
        window,
        [
            PlatformInput::MouseMove(MouseMoveEvent {
                position: from,
                pressed_button: None,
                modifiers: Modifiers::default(),
            }),
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: from,
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            }),
            PlatformInput::MouseMove(MouseMoveEvent {
                position: to,
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::default(),
            }),
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position: to,
                modifiers: Modifiers::default(),
                click_count: 1,
            }),
        ],
    );
}

fn file_drop(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    at: (f32, f32),
    paths: Vec<PathBuf>,
) {
    let position = point(px(at.0), px(at.1));
    input(
        cx,
        window,
        [
            PlatformInput::FileDrop(gpui::FileDropEvent::Entered {
                position,
                paths: gpui::ExternalPaths(paths.into()),
            }),
            PlatformInput::FileDrop(gpui::FileDropEvent::Pending { position }),
            PlatformInput::FileDrop(gpui::FileDropEvent::Submit { position }),
        ],
    );
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

/// Standalone note windows, and the sync conflict banner with its version history
/// (`165-versions.sql`). Note windows open from the workspace's own request; captures and
/// clicks address them by handle.
fn note_windows(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    open(cx, window, Route::Session(SessionId("s-offsite".into())));
    capture(cx, window, out, "note-conflict");
    click(cx, window, 888., 23.);
    capture(cx, window, out, "note-menu-history");
    click(cx, window, 800., 96.);
    capture(cx, window, out, "version-history");
    type_text(cx, window, &["escape"]);
    let open_window = |cx: &mut HeadlessAppContext, id: &str| {
        let before = cx.update(|cx| cx.windows());
        let id = SessionId(id.into());
        cx.update(|cx| {
            window
                .update(cx, |root, _, cx| {
                    root.workspace().update(cx, |_, cx| {
                        cx.emit(desktop_gpui::workspace::WorkspaceAction::OpenNoteWindow(id))
                    })
                })
                .unwrap()
        });
        settle(cx);
        cx.update(|cx| cx.windows())
            .into_iter()
            .find(|candidate| !before.contains(candidate))
            .expect("the note opens in a new window")
    };
    let note = open_window(cx, "s-sync");
    shoot(cx, note, out, "note-window");
    press(cx, note, 697., 24.);
    shoot(cx, note, out, "note-window-menu");
    press(cx, note, 360., 790.);
    shoot(cx, note, out, "note-window-chat");
    press(cx, note, 654., 592.);
    shoot(cx, note, out, "note-window-chat-right");
    let conflict = open_window(cx, "s-offsite");
    shoot(cx, conflict, out, "note-window-conflict");
    for handle in [note, conflict] {
        cx.update_window(handle, |_, window, _| window.remove_window())
            .unwrap();
    }
    settle(cx);
}

fn shoot(cx: &mut HeadlessAppContext, window: AnyWindowHandle, out: &std::path::Path, name: &str) {
    cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    let image = cx.capture_screenshot(window).unwrap();
    let path = out.join(format!("{name}.png"));
    image.save(&path).unwrap();
    println!("{}", path.display());
}

fn press(cx: &mut HeadlessAppContext, window: AnyWindowHandle, x: f32, y: f32) {
    let position = point(px(x), px(y));
    for event in [
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
    ] {
        cx.update_window(window, |_, window, cx| {
            window.draw(cx).clear(cx);
            window.dispatch_event(event, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
    settle(cx);
}

/// Signed-in Teams and Sync pages, answered by an in-memory cloud: an Enterprise workspace the
/// viewer owns, a Team workspace they belong to, a pending invitation, and synced devices.
fn teams_and_sync(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
) {
    use desktop_gpui::product::{
        account::{self, Account, Claims, User, Workspace, WorkspaceTier},
        cloud::calls::{AccountCloud, SyncBlock, SyncHealth, SyncState, SyncStatus},
    };
    let viewer = "7f1c2a9e-3b4d-4e5f-8a6b-9c0d1e2f3a4b";
    let fake = Arc::new(FakeCloud::default());
    let northwind = "5d8f2c1a-7b3e-4f6a-9c2d-1e0b7a6f5c4d";
    let contoso = "2c4e6a8b-1d3f-4a5b-8c7d-9e0f1a2b3c4d";
    let workspace = |id: &str, name: &str, owner: &str, role: &str| {
        serde_json::json!({"id": id, "name": name, "kind": "shared", "owner_user_id": owner,
            "share_slug": null, "logo_data": null, "workspace_memberships": [{"role": role}]})
    };
    let member = |id: &str, email: &str, name: &str, role: &str| serde_json::json!({"user_id": id, "user_email": email, "user_name": name, "role": role, "deleted_at": null});
    fake.set(
        "workspaces",
        serde_json::json!([
            workspace(northwind, "Northwind", viewer, "owner"),
            workspace(
                contoso,
                "Contoso Design",
                "4b2d6f8a-0c1e-4d3f-9a5b-7c8d9e0f1a2b",
                "member"
            ),
        ]),
    );
    fake.set("list_my_workspace_invitations", serde_json::json!([]));
    fake.set(
        &format!("get_workspace_access:{northwind}"),
        serde_json::json!([{"workspace_role": "owner", "workspace_tier": "enterprise", "used_seats": 3, "seat_limit": 10,
            "capabilities": ["team.manage_workspace", "team.manage_members", "team.manage_policies", "team.view_usage",
                "team.custom_subdomain", "enterprise.sso", "enterprise.scim", "enterprise.retention"]}]),
    );
    fake.set(
        &format!("get_workspace_access:{contoso}"),
        serde_json::json!([{"workspace_role": "member", "workspace_tier": "team", "used_seats": 4, "seat_limit": 5,
            "capabilities": ["team.shared_notes"]}]),
    );
    fake.set(
        &format!("list_workspace_members_with_profiles:{northwind}"),
        serde_json::json!([
            member(viewer, "alex@example.com", "Alex Rivera", "owner"),
            member(
                "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d",
                "maya@northwind.example",
                "Maya Chen",
                "admin"
            ),
            member(
                "1b2c3d4e-5f6a-4b7c-9d8e-0f1a2b3c4d5e",
                "sam@northwind.example",
                "Sam Patel",
                "member"
            ),
        ]),
    );
    fake.set(
        &format!("list_workspace_members_with_profiles:{contoso}"),
        serde_json::json!([
            member(
                "4b2d6f8a-0c1e-4d3f-9a5b-7c8d9e0f1a2b",
                "jordan@contoso.example",
                "Jordan Lee",
                "owner"
            ),
            member(viewer, "alex@example.com", "Alex Rivera", "member"),
        ]),
    );
    fake.set(
        &format!("list_workspace_invitations:{northwind}"),
        serde_json::json!([{"invitation_id": "6e7f8a9b-0c1d-4e2f-8a3b-4c5d6e7f8a9b", "invitee_email": "priya@fabrikam.example",
            "expires_at": "2026-12-01T00:00:00Z", "accepted_at": null, "revoked_at": null}]),
    );
    fake.set(
        &format!("get_workspace_usage_overview:{northwind}"),
        serde_json::json!([{"member_count": 3, "enrolled_devices": 5, "shares_created_30d": 12, "seat_limit": 10, "used_seats": 3}]),
    );
    fake.set(
        &format!("get_workspace_policy:{northwind}"),
        serde_json::json!([{"allowed_share_scopes": ["restricted", "workspace", "link"], "default_share_scope": "workspace",
            "retention_days": 365, "model_training_opt_out": true, "consent_notification_enabled": true, "require_sso": false}]),
    );
    fake.set(
        &format!("get_workspace_email_auto_join:{northwind}"),
        serde_json::json!([{"domain": "northwind.example", "enabled": true}]),
    );
    let now = chrono::Utc::now();
    let ago = |minutes: i64| (now - chrono::Duration::minutes(minutes)).to_rfc3339();
    fake.set(
        "GET /sync/devices",
        serde_json::json!({"maxDevices": 3,
            "devices": [
                {"deviceFingerprint": "fp-macbook-0001", "deviceName": "Alex's MacBook Pro", "lastSeenAt": ago(1)},
                {"deviceFingerprint": "fp-iphone-0002", "deviceName": "Alex's iPhone", "lastSeenAt": ago(125)},
            ],
            "pendingDevices": [
                {"requestId": "9a8b7c6d-5e4f-4a3b-8c2d-1e0f9a8b7c6d", "deviceFingerprint": "fp-studio-0003",
                    "deviceName": "Studio Mac mini", "status": "pending"},
            ]}),
    );
    let synced = SyncState {
        enabled: true,
        block: None,
        status: SyncStatus {
            configured: true,
            running: true,
            last_sync_at_ms: Some((now.timestamp_millis() - 3 * 60_000) as u64),
            ..SyncStatus::default()
        },
        health: SyncHealth::default(),
        fingerprint: "fp-macbook-0001".into(),
        log: vec![
            format!("{} INFO cloudsync: manual sync requested", ago(3)),
            format!("{} INFO cloudsync: encrypted sync started", ago(42)),
        ],
    };
    *fake.sync.lock().unwrap() = synced.clone();
    cx.update(|cx| cx.set_global(AccountCloud(fake.clone())));

    let account = |claims: Claims, workspaces: Vec<Workspace>| Account {
        user: Some(User {
            id: viewer.into(),
            email: Some("alex@example.com".into()),
        }),
        claims: Some(claims),
        workspaces: Some(workspaces.into()),
        trial_eligible: false,
        devices: None,
    };
    let pro = || Claims {
        entitlements: vec!["hyprnote_pro".into()],
        subscription_status: Some("active".into()),
        current_period_end: Some(account::now() + 20 * 86_400),
        ..Claims::default()
    };
    let seat = |id: &str, name: &str, tier| Workspace {
        id: id.into(),
        name: name.into(),
        tier,
        seat_limit: Some(10),
        used_seats: 3,
    };
    cx.update(|cx| {
        account::set(
            account(
                pro(),
                vec![
                    seat(northwind, "Northwind", WorkspaceTier::Enterprise),
                    seat(contoso, "Contoso Design", WorkspaceTier::Team),
                ],
            ),
            cx,
        )
    });
    let team = |cx: &mut HeadlessAppContext| {
        open(cx, window, Route::Settings("app".into()));
        open(cx, window, Route::Settings("team".into()));
    };
    team(cx);
    capture(cx, window, out, "settings-team-owner");
    for (index, y) in [560., 1120., 1680.].into_iter().enumerate() {
        scroll_content(cx, window, -y);
        capture(
            cx,
            window,
            out,
            &format!("settings-team-owner-{}", index + 1),
        );
        scroll_content(cx, window, y);
    }
    // Each interaction starts from a fresh page; `scroll` moves the content first.
    for (name, scroll, (x, y), then) in TEAMS_CLICKS {
        team(cx);
        scroll_content(cx, window, -scroll);
        click(cx, window, x, y);
        if let Some((x, y)) = then {
            click(cx, window, x, y);
        }
        capture(cx, window, out, &format!("settings-team-{name}"));
    }

    // Another workspace where the viewer is a member: read-only roles and Leave.
    team(cx);
    click(cx, window, TEAMS_CONTOSO_TAB.0, TEAMS_CONTOSO_TAB.1);
    capture(cx, window, out, "settings-team-member");

    // An invited Pro account without workspaces: the invitation above the create form.
    fake.set("workspaces", serde_json::json!([]));
    fake.set(
        "list_my_workspace_invitations",
        serde_json::json!([{"invitation_id": "3f4e5d6c-7b8a-4c9d-8e0f-1a2b3c4d5e6f", "workspace_id": contoso,
            "workspace_name": "Contoso Design", "workspace_logo_data": null, "invited_by_email": "jordan@contoso.example",
            "expires_at": "2026-12-01T00:00:00Z"}]),
    );
    cx.update(|cx| account::set(account(pro(), vec![]), cx));
    open(cx, window, Route::Settings("app".into()));
    open(cx, window, Route::Settings("team".into()));
    capture(cx, window, out, "settings-team-invited");
    // Free and invited: the create form stays behind the Pro plan gate.
    cx.update(|cx| account::set(account(Claims::default(), vec![]), cx));
    capture(cx, window, out, "settings-team-invited-free");

    cx.update(|cx| account::set(account(pro(), vec![]), cx));
    let sync = |cx: &mut HeadlessAppContext| {
        open(cx, window, Route::Settings("app".into()));
        open(cx, window, Route::Settings("sync".into()));
    };
    sync(cx);
    capture(cx, window, out, "settings-sync-pro");
    for (name, (x, y)) in SYNC_CLICKS {
        sync(cx);
        click(cx, window, x, y);
        capture(cx, window, out, &format!("settings-sync-{name}"));
    }

    // Attention states: a device limit with health counters, then a missing recovery key.
    *fake.sync.lock().unwrap() = SyncState {
        block: Some(SyncBlock::DeviceLimit),
        health: SyncHealth {
            conflicted_notes: 2,
            pending_update: 1,
            too_large: 0,
        },
        ..synced.clone()
    };
    open(cx, window, Route::Settings("app".into()));
    open(cx, window, Route::Settings("sync".into()));
    capture(cx, window, out, "settings-sync-device-limit");
    *fake.sync.lock().unwrap() = SyncState {
        enabled: false,
        block: Some(SyncBlock::SetupRequired),
        ..synced
    };
    open(cx, window, Route::Settings("app".into()));
    open(cx, window, Route::Settings("sync".into()));
    capture(cx, window, out, "settings-sync-paused");
    // Turning sync on without a recovery key opens the setup dialog; Create shows the key.
    click(cx, window, SYNC_SWITCH.0, SYNC_SWITCH.1);
    capture(cx, window, out, "settings-sync-setup");
    click(cx, window, SYNC_SETUP_CREATE.0, SYNC_SETUP_CREATE.1);
    capture(cx, window, out, "settings-sync-setup-key");

    cx.update(|cx| {
        cx.remove_global::<AccountCloud>();
        account::set(Account::default(), cx);
    });
    open(cx, window, Route::Settings("app".into()));
}

type Point = (f32, f32);

/// Teams interactions: capture name, content scroll, click, and an optional second click.
const TEAMS_CLICKS: [(&str, f32, Point, Option<Point>); 6] = [
    ("invite", 0., (826., 413.), None),
    ("role-menu", 560., (725., 118.), None),
    ("transfer", 560., (725., 118.), Some((725., 158.))),
    ("member-menu", 560., (853., 118.), None),
    ("remove", 560., (853., 118.), Some((773., 158.))),
    ("cancel-invitation", 560., (853., 232.), Some((773., 304.))),
];
const TEAMS_CONTOSO_TAB: Point = (435., 110.);
/// Sync interactions from the synced page: capture name and click.
const SYNC_CLICKS: [(&str, Point); 3] = [
    ("log", (550., 227.)),
    ("rename", (489., 358.)),
    ("add-device", (833., 303.)),
];
const SYNC_SWITCH: Point = (864., 105.);
const SYNC_SETUP_CREATE: Point = (455., 340.);

/// Scrolls the settings content; negative deltas move the content up.
fn scroll_content(cx: &mut HeadlessAppContext, window: WindowHandle<ApplicationView>, y: f32) {
    input(
        cx,
        window,
        [PlatformInput::Scroll(ScrollEvent {
            position: point(px(560.), px(400.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(y))),
            modifiers: Modifiers::default(),
            touch_phase: TouchPhase::Moved,
            source: ScrollSource::Pointer,
        })],
    );
}

/// Answers the Teams and Sync pages' cloud calls from memory. RPC answers are keyed by name,
/// or by `name:workspace` for workspace-scoped calls.
#[derive(Default)]
struct FakeCloud {
    answers: std::sync::Mutex<std::collections::HashMap<String, serde_json::Value>>,
    sync: std::sync::Mutex<desktop_gpui::product::cloud::calls::SyncState>,
}

impl FakeCloud {
    fn set(&self, key: &str, value: serde_json::Value) {
        self.answers.lock().unwrap().insert(key.into(), value);
    }

    fn answer(&self, key: &str, workspace: Option<&str>) -> serde_json::Value {
        let answers = self.answers.lock().unwrap();
        workspace
            .and_then(|workspace| answers.get(&format!("{key}:{workspace}")))
            .or_else(|| answers.get(key))
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]))
    }
}

impl desktop_gpui::product::cloud::calls::CloudCalls for FakeCloud {
    fn rpc(
        &self,
        name: &'static str,
        body: serde_json::Value,
    ) -> futures::future::BoxFuture<'static, desktop_runtime::Result<serde_json::Value>> {
        let value = self.answer(name, body["p_workspace_id"].as_str());
        Box::pin(async move { Ok(value) })
    }
    fn api(
        &self,
        method: reqwest::Method,
        path: String,
        _: Option<serde_json::Value>,
    ) -> futures::future::BoxFuture<'static, desktop_runtime::Result<serde_json::Value>> {
        let value = self.answer(&format!("{method} {path}"), None);
        Box::pin(async move { Ok(value) })
    }
    fn select(
        &self,
        table: &'static str,
        _: Vec<(&'static str, String)>,
    ) -> futures::future::BoxFuture<'static, desktop_runtime::Result<serde_json::Value>> {
        let value = self.answer(table, None);
        Box::pin(async move { Ok(value) })
    }
    fn open_web(
        &self,
        _: &'static str,
        _: Vec<(&'static str, String)>,
        _: Option<&'static str>,
    ) -> futures::future::BoxFuture<'static, desktop_runtime::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn copy_text(
        &self,
        _: zeroize::Zeroizing<String>,
    ) -> futures::future::BoxFuture<'static, desktop_runtime::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn invitation_url(&self, invitation: &str, token: &str) -> desktop_runtime::Result<String> {
        Ok(format!(
            "https://anarlog.so/team/invite/{invitation}/#token={token}"
        ))
    }
    fn sync_state(
        &self,
    ) -> futures::future::BoxFuture<
        'static,
        desktop_runtime::Result<desktop_gpui::product::cloud::calls::SyncState>,
    > {
        let state = self.sync.lock().unwrap().clone();
        Box::pin(async move { Ok(state) })
    }
    fn sync(
        &self,
        command: desktop_gpui::product::cloud::calls::SyncCommand,
    ) -> futures::future::BoxFuture<
        'static,
        desktop_runtime::Result<Option<zeroize::Zeroizing<String>>>,
    > {
        use desktop_gpui::product::cloud::calls::{SyncBlock, SyncCommand};
        let mut state = self.sync.lock().unwrap();
        let key = match command {
            SyncCommand::CreateRecoveryKey => Some(zeroize::Zeroizing::new(
                "anarlog-e2ee-v1:Q2xvdWRTeW5jRml4dHVyZVJlY292ZXJ5S2V5Rm9yU2NyZWVuc2hvdHM"
                    .to_owned(),
            )),
            SyncCommand::SetEnabled(true) if state.block == Some(SyncBlock::SetupRequired) => None,
            SyncCommand::SetEnabled(enabled) => {
                state.enabled = enabled;
                None
            }
            _ => None,
        };
        Box::pin(async move { Ok(key) })
    }
    fn refresh_account(&self, _: &mut gpui::App) {}
}
