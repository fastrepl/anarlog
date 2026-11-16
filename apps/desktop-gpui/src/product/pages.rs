//! Settings pages rendered from stored preferences. Copy, order and spacing follow the
//! shipping components named on each method.

use gpui::{
    AnyElement, Context, Div, FontWeight, Hsla, SharedString, Window, div, hsla, prelude::*, px,
};
use serde_json::json;

use crate::ui::theme::{Theme, theme};

use super::{
    controls::{self, setting_row},
    locale,
    preferences_view::{PreferencesView, SelectOption},
};

fn page(title: &'static str, gap: f32) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(gap))
        .child(controls::page_title(title))
}

impl PreferencesView {
    /// `settings/general/index.tsx` (section "app") and `app-settings.tsx`.
    pub(super) fn general(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(window);
        let macos = cfg!(target_os = "macos");
        let main = self.text("ai_language");
        let main_base = locale::base_code(&main);
        let main_value = locale::LANGUAGES
            .iter()
            .find(|(code, _)| *code == main_base)
            .map(|(code, _)| (*code).to_owned())
            .unwrap_or(main);
        let languages = locale::LANGUAGES
            .iter()
            .map(|(code, name)| SelectOption::new(*code, *name))
            .collect();
        let timezone = Some(self.text("timezone"))
            .filter(|zone| !zone.is_empty())
            .unwrap_or_else(locale::system_timezone);
        let timezones = locale::TIMEZONES
            .iter()
            .map(|(zone, label, detail)| SelectOption {
                value: (*zone).into(),
                label: (*label).into(),
                detail: Some((*detail).into()),
            })
            .collect();
        let week_start = Some(self.text("week_start"))
            .filter(|day| !day.is_empty())
            .unwrap_or_else(|| locale::system_week_start().into());
        page("General", 32.)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(self.switch_row(
                        "autostart",
                        "Start Anarlog at login",
                        Some("Have Anarlog ready when you sign in."),
                        true,
                        &colors,
                        cx,
                    ))
                    .child(self.switch_row(
                        "automatic_updates",
                        "Automatically install updates",
                        Some("Stay current with updates installed the next time Anarlog opens."),
                        true,
                        &colors,
                        cx,
                    ))
                    // The native preview has no release version or changelog view yet.
                    .child(div().flex().child(controls::outline_button(
                        "release-notes",
                        "Show release notes",
                        false,
                        &colors,
                        None,
                    )))
                    .when(macos, |view| {
                        view.child(self.switch_row(
                            "show_app_in_dock",
                            "Show app in Dock",
                            Some("Show Anarlog in the Dock and app switcher."),
                            true,
                            &colors,
                            cx,
                        ))
                    })
                    .child(self.switch_row(
                        "show_tray_icon",
                        "Show tray icon",
                        macos.then_some("Open Anarlog from the menu bar."),
                        true,
                        &colors,
                        cx,
                    )),
            )
            .child(
                div()
                    .child(controls::section_title("Language & Region").mb_4())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_6()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_4()
                                    .child(setting_row(
                                        "Main language",
                                        Some(
                                            "Default language for transcription and AI responses."
                                                .into(),
                                        ),
                                        true,
                                        self.dropdown_select(
                                            "ai_language",
                                            languages,
                                            &main_value,
                                            "Select language",
                                            self.writable("ai_language"),
                                            true,
                                            controls::CONTROL_WIDTH,
                                            |this, value, cx| {
                                                this.set("ai_language", json!(value), cx)
                                            },
                                            window,
                                            cx,
                                        ),
                                        &colors,
                                    ))
                                    .child(
                                        controls::nested(&colors)
                                            .pt_2()
                                            .child(self.switch_row(
                                                "summary_use_main_language",
                                                "Use main language for summaries",
                                                Some(
                                                    "When off, summaries follow the detected meeting language.",
                                                ),
                                                true,
                                                &colors,
                                                cx,
                                            ))
                                            .child(self.spoken_languages(&colors, cx)),
                                    ),
                            )
                            .child(setting_row(
                                "Timezone",
                                Some("Show the timeline in your preferred timezone.".into()),
                                true,
                                self.dropdown_select(
                                    "timezone",
                                    timezones,
                                    &timezone,
                                    "Select timezone",
                                    self.writable("timezone"),
                                    true,
                                    288.,
                                    |this, value, cx| {
                                        let value = if value == locale::system_timezone() {
                                            String::new()
                                        } else {
                                            value
                                        };
                                        this.set("timezone", json!(value), cx)
                                    },
                                    window,
                                    cx,
                                ),
                                &colors,
                            ))
                            .child(setting_row(
                                "Week starts on",
                                Some("Choose which day begins your calendar week.".into()),
                                true,
                                self.dropdown_select(
                                    "week_start",
                                    vec![
                                        SelectOption::new("sunday", "Sunday"),
                                        SelectOption::new("monday", "Monday"),
                                    ],
                                    &week_start,
                                    "Select day",
                                    self.writable("week_start"),
                                    false,
                                    controls::CONTROL_WIDTH,
                                    |this, value, cx| {
                                        let value = if value == locale::system_week_start() {
                                            String::new()
                                        } else {
                                            value
                                        };
                                        this.set("week_start", json!(value), cx)
                                    },
                                    window,
                                    cx,
                                ),
                                &colors,
                            )),
                    ),
            )
            .child(
                div()
                    .child(controls::section_title("Storage").mb_4())
                    .child(self.export_location(&colors, cx)),
            )
            .into_any_element()
    }

    /// `settings/general/spoken-languages.tsx`. Removing a chip writes immediately; adding
    /// languages needs the language search field that the native input does not offer yet.
    fn spoken_languages(&self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let main = self.text("ai_language");
        let selected = locale::additional_spoken(&main, &self.text("spoken_languages"));
        let writable = self.writable("spoken_languages");
        div()
            .child(
                div()
                    .mb_1()
                    .text_sm()
                    .line_height(px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .child("Additional spoken languages"),
            )
            .child(
                controls::description(
                    "Used for transcription and automatic summary language detection.",
                    colors,
                )
                .mb_3(),
            )
            .child(
                div()
                    .min_h(px(38.))
                    .w_full()
                    .px_2()
                    .py(px(6.))
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.))
                    .rounded(px(16.))
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.card)
                    .text_sm()
                    .when(selected.is_empty(), |view| {
                        view.child(controls::icon(
                            "Search01Icon.svg",
                            16.,
                            colors.muted_foreground,
                        ))
                        .child(
                            div()
                                .text_color(colors.muted_foreground)
                                .child("Add language"),
                        )
                    })
                    .children(selected.iter().map(|code| {
                        let remaining = selected
                            .iter()
                            .filter(|other| *other != code)
                            .cloned()
                            .collect::<Vec<_>>();
                        div()
                            .px_2()
                            .py(px(2.))
                            .flex()
                            .items_center()
                            .gap_1()
                            .rounded(px(8.))
                            .bg(colors.muted)
                            .text_xs()
                            .child(locale::language_name(code))
                            .child(
                                div()
                                    .id(SharedString::from(format!("remove-language-{code}")))
                                    .ml(px(2.))
                                    .when(writable, |view| {
                                        view.cursor_pointer().on_click(cx.listener(
                                            move |this, _, _, cx| {
                                                this.set(
                                                    "spoken_languages",
                                                    json!(json!(remaining).to_string()),
                                                    cx,
                                                )
                                            },
                                        ))
                                    })
                                    .child(controls::icon(
                                        "product/XIcon.svg",
                                        10.,
                                        colors.foreground,
                                    )),
                            )
                    })),
            )
    }

    /// `settings/general/storage/export-location.tsx`.
    fn export_location(&self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let stored = self.text("export_directory");
        let home = std::env::home_dir();
        let path = if stored.is_empty() {
            home.as_ref()
                .map(|home| home.join("Downloads").to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            stored.clone()
        };
        let shown = home
            .and_then(|home| {
                path.strip_prefix(home.to_string_lossy().as_ref())
                    .map(|rest| format!("~{rest}"))
            })
            .unwrap_or_else(|| path.clone());
        let writable = self.writable("export_directory");
        let open_path = path.clone();
        let hover = colors.muted.opacity(0.4);
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .id("open-export-location")
                            .flex_1()
                            .min_w_0()
                            .px_2()
                            .py_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .rounded(px(8.))
                            .cursor_pointer()
                            .hover(move |style| style.bg(hover))
                            .on_click(move |_, _, cx| {
                                cx.open_with_system(std::path::Path::new(&open_path))
                            })
                            .child(controls::icon(
                                "workspace/Folder01Icon.svg",
                                16.,
                                colors.muted_foreground,
                            ))
                            .child(
                                div()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("Export location"),
                                    )
                                    .child(controls::description(shown, colors).truncate()),
                            ),
                    )
                    .child(
                        div().w(px(144.)).flex().child(
                            controls::outline_button(
                                "choose-export-folder",
                                "Choose folder",
                                writable,
                                colors,
                                Some(Box::new(
                                    cx.listener(|this, _, _, cx| this.choose_export_folder(cx)),
                                )),
                            )
                            .w_full(),
                        ),
                    ),
            )
            .child(
                controls::description(
                    "Save PDF, text, Markdown, and Org exports to this folder.",
                    colors,
                )
                .px_2(),
            )
            .when(!stored.is_empty(), |view| {
                view.child(
                    div()
                        .id("reset-export-folder")
                        .px_2()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors.primary)
                        .cursor_pointer()
                        .child("Reset to Downloads")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.set("export_directory", json!(""), cx)
                        })),
                )
            })
    }

    fn choose_export_folder(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose export folder".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            if let Some(path) = paths.into_iter().next() {
                let _ = this.update(cx, |this, cx| {
                    this.set(
                        "export_directory",
                        json!(path.to_string_lossy().into_owned()),
                        cx,
                    )
                });
            }
        })
        .detach();
    }

    /// `settings/appearance/*`.
    pub(super) fn appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(window);
        let current = match self.text("theme").as_str() {
            "light" => "light",
            "dark" => "dark",
            _ => "system",
        };
        let writable = self.writable("theme");
        page("Appearance", 40.)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div().child(controls::section_title("Theme")).child(
                            div()
                                .mt_1()
                                .text_sm()
                                .text_color(colors.muted_foreground)
                                .child("Choose how Anarlog looks on this device."),
                        ),
                    )
                    .child(
                        div().flex().gap_3().children(
                            [
                                ("light", "Light", "Bright canvas"),
                                ("dark", "Dark", "Low-light canvas"),
                                ("system", "System", "Match your device"),
                            ]
                            .into_iter()
                            .map(|(value, label, detail)| {
                                let selected = value == current;
                                div()
                                    .id(value)
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .rounded(px(16.))
                                    .border_1()
                                    .border_color(if selected {
                                        colors.foreground.opacity(0.5)
                                    } else {
                                        colors.border
                                    })
                                    .bg(if selected {
                                        colors.accent.opacity(0.4)
                                    } else {
                                        colors.background
                                    })
                                    .when(selected, |view| view.shadow_xs())
                                    .when(writable, |view| {
                                        view.cursor_pointer().on_click(cx.listener(
                                            move |this, _, _, cx| {
                                                super::settings::apply_theme(value, cx);
                                                this.set("theme", json!(value), cx);
                                            },
                                        ))
                                    })
                                    .child(theme_preview(value))
                                    .child(
                                        div()
                                            .p_3()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .border_t_1()
                                            .border_color(colors.border)
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .line_height(px(20.))
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .child(label),
                                                    )
                                                    .child(controls::description(detail, &colors)),
                                            )
                                            .when(selected, |view| {
                                                view.child(
                                                    div()
                                                        .size(px(20.))
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .rounded(px(6.))
                                                        .bg(colors.foreground)
                                                        .child(controls::icon(
                                                            "product/CheckIcon.svg",
                                                            12.,
                                                            colors.background,
                                                        )),
                                                )
                                            }),
                                    )
                            }),
                        ),
                    ),
            )
            .child(self.switch_row(
                "use_24_hour_time",
                "Use 24-hour time",
                Some("Show meeting times as 14:00 instead of 2:00 PM."),
                true,
                &colors,
                cx,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div().child(controls::section_title("Notes list")).child(
                            div()
                                .mt_1()
                                .text_sm()
                                .text_color(colors.muted_foreground)
                                .child("Choose extra fields to show on each note."),
                        ),
                    )
                    .child(self.switch_row(
                        "sidebar_show_folder",
                        "Folder",
                        Some("Show the folder above the title."),
                        true,
                        &colors,
                        cx,
                    ))
                    .child(self.switch_row(
                        "sidebar_show_tags",
                        "Tags",
                        Some("Show tags under the date and time."),
                        true,
                        &colors,
                        cx,
                    )),
            )
            .into_any_element()
    }

    /// `settings/general/notification.tsx`.
    pub(super) fn notifications(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = theme(window);
        let enabled = !self.flag("notification_disabled");
        let sound = match self.text("notification_completion_sound_name").as_str() {
            sound @ ("ready" | "success" | "chime" | "sparkle" | "bloom") => sound.to_owned(),
            _ => "ready".to_owned(),
        };
        let bounce = !cfg!(target_os = "macos") || self.flag("show_app_in_dock");
        let detect = self.flag("notification_detect");
        page("Notifications", 24.)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(self.switch_row(
                        "notification_disabled",
                        "Disable all notifications",
                        Some("Hide all notification panels, Dock alerts, and completion sounds."),
                        true,
                        &colors,
                        cx,
                    ))
                    .child(self.switch_row(
                        "notification_summary_complete",
                        "Summary complete",
                        Some("Show when a summary is ready."),
                        enabled,
                        &colors,
                        cx,
                    ))
                    .child(self.switch_row(
                        "notification_cloudsync_complete",
                        "Cloud sync complete",
                        Some("Show when initial cloud sync finishes."),
                        enabled,
                        &colors,
                        cx,
                    ))
                    .child(self.switch_row(
                        "notification_event",
                        "Event notifications",
                        Some("Prepare for events with a 5-minute reminder."),
                        enabled,
                        &colors,
                        cx,
                    ))
                    .child(self.switch_row(
                        "notification_recording",
                        "Recording status prompts",
                        Some(
                            "Ask before stopping when a meeting may have ended. When alerts are off, Anarlog keeps listening.",
                        ),
                        enabled,
                        &colors,
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(self.switch_row(
                                "notification_completion_sound",
                                "Completion sound",
                                Some(
                                    "Play a sound when transcription, summaries, or cloud sync finish.",
                                ),
                                enabled,
                                &colors,
                                cx,
                            ))
                            .when(self.flag("notification_completion_sound"), |view| {
                                view.child(controls::nested(&colors).child(setting_row(
                                    "Sound",
                                    Some("Choose from five completion sounds.".into()),
                                    true,
                                    div()
                                        .w_full()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(div().flex_1().min_w_0().child(
                                            self.dropdown_select(
                                                "notification_completion_sound_name",
                                                vec![
                                                    SelectOption::new("ready", "Ready"),
                                                    SelectOption::new("success", "Success"),
                                                    SelectOption::new("chime", "Chime"),
                                                    SelectOption::new("sparkle", "Sparkle"),
                                                    SelectOption::new("bloom", "Bloom"),
                                                ],
                                                &sound,
                                                "Select sound",
                                                enabled
                                                    && self.writable(
                                                        "notification_completion_sound_name",
                                                    ),
                                                false,
                                                controls::CONTROL_WIDTH - 72.,
                                                |this, value, cx| {
                                                    this.set(
                                                        "notification_completion_sound_name",
                                                        json!(value),
                                                        cx,
                                                    )
                                                },
                                                window,
                                                cx,
                                            ),
                                        ))
                                        // Sound playback is not wired into the native app yet.
                                        .child(
                                            controls::outline_button(
                                                "preview-sound",
                                                "Preview",
                                                false,
                                                &colors,
                                                None,
                                            )
                                            .h(px(28.))
                                            .px_2()
                                            .text_xs(),
                                        ),
                                    &colors,
                                )))
                            }),
                    )
                    .when(bounce, |view| {
                        view.child(self.switch_row(
                            "notification_bounce",
                            "Bounce app icon",
                            Some("Get your attention when a summary is ready in the background."),
                            enabled,
                            &colors,
                            cx,
                        ))
                    })
                    .when(cfg!(target_os = "macos"), |view| {
                        view.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_4()
                                .child(self.switch_row(
                                    "notification_detect",
                                    "Microphone detection",
                                    Some("Detect meetings from microphone activity."),
                                    enabled,
                                    &colors,
                                    cx,
                                ))
                                .when(detect && enabled, |view| {
                                    view.child(self.detection_settings(&colors, window, cx))
                                }),
                        )
                    })
                    .child(self.switch_row(
                        "respect_dnd",
                        "Respect Do-Not-Disturb mode",
                        Some("Don't show notifications when Do-Not-Disturb is enabled."),
                        enabled,
                        &colors,
                        cx,
                    )),
            )
            .into_any_element()
    }

    fn detection_settings(
        &self,
        colors: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let threshold = self
            .value("mic_active_threshold")
            .as_f64()
            .map(|value| (value as i64).to_string())
            .unwrap_or_default();
        controls::nested(colors)
            .pt_2()
            .child(setting_row(
                "Detection delay",
                Some("Wait before treating microphone activity as a meeting.".into()),
                false,
                div().w(px(100.)).child(
                    self.dropdown_select(
                        "mic_active_threshold",
                        [
                            ("5", "5 sec"),
                            ("10", "10 sec"),
                            ("15", "15 sec"),
                            ("30", "30 sec"),
                            ("60", "1 min"),
                            ("120", "2 min"),
                        ]
                        .into_iter()
                        .map(|(value, label)| SelectOption::new(value, label))
                        .collect(),
                        &threshold,
                        "",
                        self.writable("mic_active_threshold"),
                        false,
                        100.,
                        |this, value, cx| {
                            if let Ok(seconds) = value.parse::<u64>() {
                                this.set("mic_active_threshold", json!(seconds), cx)
                            }
                        },
                        window,
                        cx,
                    ),
                ),
                colors,
            ))
            .child(
                div()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Exclude apps from detection"),
                    )
                    .child(controls::description(
                        "Prevent selected apps from triggering meeting detection.",
                        colors,
                    )),
            )
    }

    /// `settings/general/index.tsx` (section "meetings"), `meeting-settings.tsx` and
    /// `audio-settings.tsx`.
    pub(super) fn meetings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(window);
        let pro = super::account::is_pro(cx);
        let macos = cfg!(target_os = "macos");
        let windows = cfg!(target_os = "windows");
        let share = self.text("default_meeting_share_access");
        let retention = self.text("audio_retention");
        page("Meetings", 32.)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(setting_row(
                        "Default sharing",
                        Some("Choose who can access notes from new meetings.".into()),
                        true,
                        self.dropdown_select(
                            "default_meeting_share_access",
                            vec![
                                SelectOption::new("me", "Only me"),
                                SelectOption::new("participants", "People in the meeting"),
                                SelectOption::new("workspace", "Everyone in the workspace"),
                            ],
                            &share,
                            "Only me",
                            self.writable("default_meeting_share_access"),
                            false,
                            controls::CONTROL_WIDTH,
                            |this, value, cx| {
                                this.set("default_meeting_share_access", json!(value), cx)
                            },
                            window,
                            cx,
                        ),
                        &colors,
                    ))
                    // Choosing a primary device needs the signed-in device registry.
                    .child(setting_row(
                        "Primary recording device",
                        Some(
                            if !super::account::current(cx).signed_in() {
                                "Sign in to choose a primary recording device."
                            } else if !pro {
                                "Anarlog Pro coordinates recording across your devices."
                            } else {
                                "Prefer this device when multiple devices record the same meeting. Other devices can record when it is not recording."
                            }
                            .into(),
                        ),
                        true,
                        controls::select_trigger(
                            "primary_recording_device",
                            Some("Ask each meeting".into()),
                            "",
                            false,
                            &colors,
                        ),
                        &colors,
                    ))
                    .child(plan_gate(
                        pro,
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(self.gated_switch_row(
                                "auto_start_scheduled_meetings",
                                "Start when meeting begins",
                                Some("Start listening when a scheduled meeting begins."),
                                pro,
                                true,
                                &colors,
                                cx,
                            ))
                            .child(self.gated_switch_row(
                                "auto_join_scheduled_meetings",
                                "Join scheduled meetings",
                                Some("Open the meeting link when a scheduled meeting begins."),
                                pro,
                                self.flag("auto_start_scheduled_meetings"),
                                &colors,
                                cx,
                            )),
                    ))
                    .when(!windows, |view| {
                        view.child(self.switch_row(
                            "auto_stop_meetings",
                            "Stop when meeting ends",
                            Some("Stop listening when your call ends."),
                            true,
                            &colors,
                            cx,
                        ))
                    })
                    .when(!windows, |view| {
                        view.child(self.switch_row(
                            "consent_auto_send_chat",
                            "Post recording disclosure in meeting chat",
                            Some("Tell participants when listening starts; this does not confirm consent."),
                            true,
                            &colors,
                            cx,
                        ))
                        .child(self.switch_row(
                            "capture_meeting_chat",
                            "Capture meeting chat in Memos",
                            Some("Save visible chat from supported meetings using Accessibility."),
                            true,
                            &colors,
                            cx,
                        ))
                    })
                    .when(macos || windows, |view| {
                        view.child(self.switch_row(
                            "capture_shared_screens",
                            "Capture shared screens in Memos",
                            Some("Save slides and screens others share in meetings. Requires Screen Recording."),
                            true,
                            &colors,
                            cx,
                        ))
                    })
                    .child(self.switch_row(
                        "floating_bar_enabled",
                        "Show floating bar",
                        Some("Control listening without reopening Anarlog."),
                        true,
                        &colors,
                        cx,
                    )),
            )
            .child(
                div()
                    .child(controls::section_title("Audio").mb_4())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(self.microphone_row(window, cx))
                            .child(setting_row(
                                "Audio file retention",
                                Some("Choose how long recordings stay on this device.".into()),
                                true,
                                self.dropdown_select(
                                    "audio_retention",
                                    [
                                        ("none", "Don't save"),
                                        ("oneDay", "1 day"),
                                        ("threeDays", "3 days"),
                                        ("oneWeek", "1 week"),
                                        ("oneMonth", "1 month"),
                                        ("forever", "Forever"),
                                    ]
                                    .into_iter()
                                    .map(|(value, label)| SelectOption::new(value, label))
                                    .collect(),
                                    &retention,
                                    "",
                                    self.writable("audio_retention"),
                                    false,
                                    controls::CONTROL_WIDTH,
                                    |this, value, cx| {
                                        this.set("save_recordings", json!(value != "none"), cx);
                                        this.set("audio_retention", json!(value), cx);
                                    },
                                    window,
                                    cx,
                                ),
                                &colors,
                            ))
                            .child(self.switch_row(
                                "remember_speakers",
                                "Remember speakers",
                                Some("Build voiceprints from meeting audio so speakers you name in a transcript are recognized in later meetings. Voiceprints never leave this device, and unnamed ones are deleted after 45 days."),
                                true,
                                &colors,
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }

    /// `AudioDeviceRow`. Device enumeration lives in the capture service, so only the
    /// current default and the stored device are offered here.
    fn microphone_row(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(window);
        let stored = self.text("microphone_device");
        let mut options = vec![SelectOption::new("", "Current default")];
        if !stored.is_empty() {
            options.push(SelectOption::new(stored.clone(), stored.clone()));
        }
        setting_row(
            "Microphone",
            Some("Choose the microphone that captures your voice.".into()),
            true,
            self.dropdown_select(
                "microphone_device",
                options,
                &stored,
                "Current default",
                self.writable("microphone_device"),
                false,
                controls::CONTROL_WIDTH,
                |this, value, cx| this.set("microphone_device", json!(value), cx),
                window,
                cx,
            ),
            &colors,
        )
        .into_any_element()
    }

    /// `settings/privacy/index.tsx`.
    pub(super) fn privacy(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(window);
        page("Privacy", 32.)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    // Native device authentication is not wired into the native app yet.
                    .child(self.switch_row(
                        "lock_app",
                        "Lock app",
                        Some("Device authentication is not available on this computer."),
                        false,
                        &colors,
                        cx,
                    ))
                    .child(self.switch_row(
                        "telemetry_consent",
                        "Share usage data",
                        Some("Help improve Anarlog with anonymous usage data."),
                        true,
                        &colors,
                        cx,
                    ))
                    .child(self.switch_row(
                        "crash_reporting_consent",
                        "Error",
                        Some("Send sanitized crash and error reports to help improve Anarlog."),
                        true,
                        &colors,
                        cx,
                    )),
            )
            .into_any_element()
    }

    /// `settings/dictation.tsx` and `dictation-shortcut.tsx`.
    pub(super) fn dictation(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(window);
        let pro = super::account::is_pro(cx);
        let hands_free = self.flag("dictation_hands_free");
        let keys = self
            .text("dictation_shortcut")
            .split('+')
            .map(|key| match key {
                "Control" => "⌃".to_owned(),
                "Alt" => "⌥".to_owned(),
                "Meta" | "Command" => "⌘".to_owned(),
                "Shift" => "⇧".to_owned(),
                "Fn" => "Fn / Globe".to_owned(),
                "RightCommand" => "Right Command".to_owned(),
                key => key.to_owned(),
            })
            .collect::<Vec<_>>();
        let chip = |label: String| {
            div()
                .px(px(6.))
                .py(px(2.))
                .rounded(px(4.))
                .border_1()
                .border_color(colors.border)
                .bg(colors.muted)
                .text_xs()
                .child(label)
        };
        page("Dictation", 32.)
            .child(plan_gate(
                pro,
                div()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(
                        div()
                            .text_sm()
                            .text_color(colors.muted_foreground)
                            .child("Speak into the focused text field in your apps. Your transcript appears when you finish."),
                    )
                    .child(self.gated_switch_row(
                        "dictation_enabled",
                        "Enable dictation",
                        Some("Keep Anarlog running to use your shortcut in other apps."),
                        pro,
                        true,
                        &colors,
                        cx,
                    ))
                    // Recording a new shortcut needs the native key-capture service.
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_4()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("Dictation shortcut"),
                                    )
                                    .child(
                                        div()
                                            .h(px(36.))
                                            .px_3()
                                            .flex()
                                            .items_center()
                                            .gap_1()
                                            .rounded(px(8.))
                                            .border_1()
                                            .border_color(colors.input)
                                            .children(keys.into_iter().map(chip)),
                                    ),
                            )
                            .child(controls::description(
                                "Click the shortcut to record a new key combination. Press Escape to cancel.",
                                &colors,
                            ))
                            .child(
                                div().flex().gap_2().children(
                                    ["Fn / Globe", "Right Command"].into_iter().map(|label| {
                                        div()
                                            .h(px(28.))
                                            .px_2()
                                            .flex()
                                            .items_center()
                                            .rounded(px(8.))
                                            .border_1()
                                            .border_color(colors.input)
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(label)
                                    }),
                                ),
                            ),
                    )
                    .child(self.microphone_row(window, cx))
                    .child(self.gated_switch_row(
                        "dictation_live_preview",
                        "Live transcript preview",
                        Some("Show words in the floating panel as you speak. Uses a live connection to your selected transcription provider when supported. Text is inserted when you finish."),
                        pro,
                        true,
                        &colors,
                        cx,
                    ))
                    .child(self.gated_switch_row(
                        "dictation_hands_free",
                        "Hands-free dictation",
                        Some(if hands_free {
                            "Press your shortcut to start, then press it again to finish."
                        } else {
                            "Hold your shortcut while speaking, then release it to finish."
                        }),
                        pro,
                        true,
                        &colors,
                        cx,
                    ))
                    .child(controls::description(
                        "Press Escape to cancel. Dictation stops after five minutes and pauses while Anarlog records a meeting.",
                        &colors,
                    ))
                    .child(div().flex().flex_wrap().gap_2().children(
                        ["Permissions", "Language & transcription", "Dictionary"].map(|label| {
                            controls::outline_button(label, label, true, &colors, None)
                        }),
                    )),
            ))
            .into_any_element()
    }

    /// `settings/dictionary/index.tsx`.
    pub(super) fn dictionary(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(window);
        let terms = stored_terms(&self.text("personalization_dictionary_terms"));
        let enabled =
            super::account::is_pro(cx) && self.writable("personalization_dictionary_terms");
        page("Dictionary", 32.)
            .child(plan_gate(
                super::account::is_pro(cx),
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .h(px(36.))
                            .pl_4()
                            .pr_1()
                            .flex()
                            .items_center()
                            .gap_2()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(colors.border)
                            .bg(colors.card)
                            .text_sm()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .h(px(20.))
                                    .line_height(px(20.))
                                    .child(self.dictionary_input.clone()),
                            )
                            .child(
                                div()
                                    .id("add-dictionary-term")
                                    .h(px(28.))
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .rounded(px(8.))
                                    .bg(colors.foreground)
                                    .text_color(colors.background)
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .when(enabled, |view| {
                                        view.cursor_pointer().on_click(cx.listener(|this, _, _, cx| {
                                            this.add_dictionary_terms(cx)
                                        }))
                                    })
                                    .child(controls::icon("product/PlusIcon.svg", 14., colors.background))
                                    .child("Add"),
                            ),
                    )
                    .when(terms.is_empty(), |view| {
                        view.child(
                            div()
                                .min_h(px(160.))
                                .px_6()
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .rounded(px(16.))
                                .border_1()
                                .border_color(colors.border)
                                .bg(colors.card)
                                .text_center()
                                .child(controls::icon("product/BookOpen01Icon.svg", 20., colors.muted_foreground).mb_3())
                                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Your dictionary is empty"))
                                .child(
                                    div()
                                        .mt_1()
                                        .max_w(px(384.))
                                        .text_xs()
                                        .text_color(colors.muted_foreground)
                                        .child("Tip: Add teammate names, acronyms, company jargon, and product terms."),
                                ),
                        )
                    })
                    .when(!terms.is_empty(), |view| {
                        view.child(
                            div()
                                .overflow_hidden()
                                .rounded(px(16.))
                                .border_1()
                                .border_color(colors.border)
                                .bg(colors.card)
                                .children(terms.iter().enumerate().map(|(index, term)| {
                                    let remaining = terms
                                        .iter()
                                        .filter(|other| *other != term)
                                        .cloned()
                                        .collect::<Vec<_>>();
                                    div()
                                        .min_h(px(48.))
                                        .py_3()
                                        .pl_4()
                                        .pr_3()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap_3()
                                        .when(index > 0, |view| view.border_t_1().border_color(colors.border))
                                        .child(div().text_sm().child(term.clone()))
                                        .child(
                                            div()
                                                .id(SharedString::from(format!("remove-term-{index}")))
                                                .size(px(28.))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .when(enabled, |view| {
                                                    view.cursor_pointer().on_click(cx.listener(
                                                        move |this, _, _, cx| {
                                                            this.set(
                                                                "personalization_dictionary_terms",
                                                                json!(json!(remaining).to_string()),
                                                                cx,
                                                            )
                                                        },
                                                    ))
                                                })
                                                .child(controls::icon("product/XIcon.svg", 16., colors.muted_foreground)),
                                        )
                                })),
                        )
                    }),
            ))
            .into_any_element()
    }

    pub(super) fn add_dictionary_terms(&mut self, cx: &mut Context<Self>) {
        let input = self.dictionary_input.read(cx).buffer.text.clone();
        let stored = stored_terms(&self.text("personalization_dictionary_terms"));
        let terms = normalize_terms(
            stored
                .iter()
                .cloned()
                .chain(input.split(['\n', ',']).map(str::to_owned)),
        );
        if terms.len() > stored.len() {
            self.set(
                "personalization_dictionary_terms",
                json!(json!(terms).to_string()),
                cx,
            );
        }
        self.dictionary_input
            .update(cx, |input, cx| input.set_text(String::new(), cx));
    }
}

/// Shipping's `PlanGate`: content stays visible at 60% opacity and ignores input.
fn plan_gate(allowed: bool, content: Div) -> Div {
    if allowed {
        return content;
    }
    div()
        .relative()
        .opacity(0.6)
        .child(content)
        .child(div().absolute().inset_0().occlude())
}

fn stored_terms(stored: &str) -> Vec<String> {
    normalize_terms(serde_json::from_str::<Vec<String>>(stored).unwrap_or_default())
}

/// `normalizeKeywordList`: trimmed, whitespace-collapsed, at least two characters, and
/// unique ignoring case, in first-seen order.
fn normalize_terms(terms: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    terms
        .into_iter()
        .map(|term| term.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|term| term.chars().count() >= 2 && seen.insert(term.to_lowercase()))
        .collect()
}

/// `ThemePreview` in `settings/appearance/theme.tsx`: a miniature light or dark window, and
/// for System the dark one over the lower-right half.
fn theme_preview(theme: &'static str) -> impl IntoElement {
    let canvas = |dark: bool| {
        let (bg, sidebar, heading, line) = if dark {
            (DARK_BG, DARK_SIDEBAR, DARK_HEADING, DARK_LINE)
        } else {
            (
                hsla(0., 0., 1., 1.),
                hsla(0., 0., 0.96, 1.),
                hsla(0., 0., 0.25, 1.),
                hsla(0., 0., 0.9, 1.),
            )
        };
        preview_canvas(bg, sidebar, heading, line)
    };
    div()
        .relative()
        .h(px(112.))
        .overflow_hidden()
        .child(canvas(theme == "dark"))
        .when(theme == "system", |view| {
            view.child(
                gpui::canvas(
                    |_, _, _| (),
                    |bounds, (), window, _| paint_dark_half(bounds, window),
                )
                .absolute()
                .size_full(),
            )
        })
}

const DARK_BG: Hsla = hsla(0., 0., 0.04, 1.);
const DARK_SIDEBAR: Hsla = hsla(0., 0., 0.15, 1.);
const DARK_HEADING: Hsla = hsla(0., 0., 0.83, 1.);
const DARK_LINE: Hsla = hsla(0., 0., 0.25, 1.);

/// The dark preview clipped to the triangle below the bottom-left to top-right diagonal,
/// which shipping draws with a CSS `clip-path`. Shapes follow `preview_canvas`'s layout.
fn paint_dark_half(bounds: gpui::Bounds<gpui::Pixels>, window: &mut Window) {
    let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let content = width - 24.;
    let sidebar = content * 0.25;
    let text_left = 12. + sidebar + 12.;
    let text_width = width - 12. - text_left;
    let mut shapes = vec![
        (rect(0., 0., width, height), DARK_BG),
        (rect(12., 30., sidebar, height - 42.), DARK_SIDEBAR),
        (rect(text_left, 34., 40., 6.), DARK_HEADING),
    ];
    for (top, fraction) in [(48., 0.8), (60., 0.6), (72., 0.667)] {
        shapes.push((rect(text_left, top, text_width * fraction, 4.), DARK_LINE));
    }
    for (index, color) in [0xf87171, 0xfbbf24, 0x4ade80].into_iter().enumerate() {
        shapes.push((
            rect(12. + index as f32 * 10., 12., 6., 6.),
            gpui::rgb(color).into(),
        ));
    }
    for (polygon, color) in shapes {
        let clipped = clip_below_diagonal(&polygon, width, height);
        let Some((first, rest)) = clipped.split_first() else {
            continue;
        };
        let at = |(x, y): (f32, f32)| gpui::point(bounds.left() + px(x), bounds.top() + px(y));
        let mut path = gpui::PathBuilder::fill();
        path.move_to(at(*first));
        for point in rest {
            path.line_to(at(*point));
        }
        path.close();
        if let Ok(path) = path.build() {
            window.paint_path(path, color);
        }
    }
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Vec<(f32, f32)> {
    vec![
        (x, y),
        (x + width, y),
        (x + width, y + height),
        (x, y + height),
    ]
}

/// Sutherland–Hodgman against the half-plane `x / width + y / height >= 1`.
fn clip_below_diagonal(polygon: &[(f32, f32)], width: f32, height: f32) -> Vec<(f32, f32)> {
    let side = |(x, y): (f32, f32)| x / width + y / height - 1.;
    let mut clipped = Vec::new();
    for (index, &current) in polygon.iter().enumerate() {
        let previous = polygon[(index + polygon.len() - 1) % polygon.len()];
        let (a, b) = (side(previous), side(current));
        if (a >= 0.) != (b >= 0.) {
            let t = a / (a - b);
            clipped.push((
                previous.0 + (current.0 - previous.0) * t,
                previous.1 + (current.1 - previous.1) * t,
            ));
        }
        if b >= 0. {
            clipped.push(current);
        }
    }
    clipped
}

fn preview_canvas(bg: Hsla, sidebar: Hsla, heading: Hsla, line: Hsla) -> Div {
    let bar = |width: gpui::DefiniteLength, height: f32, color: Hsla| {
        div().w(width).h(px(height)).rounded(px(8.)).bg(color)
    };
    div()
        .absolute()
        .inset_0()
        .p_3()
        .flex()
        .flex_col()
        .bg(bg)
        .child(
            div()
                .mb_3()
                .flex()
                .gap_1()
                .child(div().size(px(6.)).rounded_full().bg(gpui::rgb(0xf87171)))
                .child(div().size(px(6.)).rounded_full().bg(gpui::rgb(0xfbbf24)))
                .child(div().size(px(6.)).rounded_full().bg(gpui::rgb(0x4ade80))),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .gap_3()
                .child(div().w(gpui::relative(0.25)).rounded(px(6.)).bg(sidebar))
                .child(
                    div()
                        .flex_1()
                        .py_1()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(bar(px(40.).into(), 6., heading))
                        .child(bar(gpui::relative(0.8), 4., line))
                        .child(bar(gpui::relative(0.6), 4., line))
                        .child(bar(gpui::relative(0.667), 4., line)),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionary_terms_normalize_like_shipping() {
        assert_eq!(
            normalize_terms(
                ["  Anarlog ", "anarlog", "a", "Linear   issue", "GPUI"].map(String::from)
            ),
            ["Anarlog", "Linear issue", "GPUI"]
        );
        assert!(stored_terms("not json").is_empty());
    }
}
