//! Settings pages rendered from stored preferences. Copy, order and spacing follow the
//! shipping components named on each method.

use gpui::{
    AnyElement, Context, Div, Focusable, Hsla, KeyDownEvent, SharedString, Window, deferred, div,
    hsla, prelude::*, px,
};
use serde_json::json;

use crate::ui::theme::{Theme, theme};

use super::{
    controls::{self, setting_row},
    locale,
    preferences_view::{PreferencesView, SelectOption},
};
use crate::ui::a11y::Accessible;

fn page(title: &'static str, gap: f32) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(gap))
        .child(controls::page_title(title))
}

fn tildify_path(path: &str) -> String {
    tildify_path_in(path, std::env::home_dir().as_deref())
}

fn tildify_path_in(path: &str, home: Option<&std::path::Path>) -> String {
    let Some(home) = home else {
        return path.to_owned();
    };
    let home = home.to_string_lossy();
    let home = home.trim_end_matches(['/', '\\']);
    if home.is_empty() {
        return path.to_owned();
    }
    match path.strip_prefix(home) {
        Some("") => "~".to_owned(),
        Some(rest) if rest.starts_with('/') || rest.starts_with('\\') => format!("~{rest}"),
        _ => path.to_owned(),
    }
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
                        None,
                        true,
                        &colors,
                        cx,
                    ))
                    .child(self.switch_row(
                        "automatic_updates",
                        "Automatically install updates",
                        None,
                        true,
                        &colors,
                        cx,
                    ))
                    .child(div().flex().child(controls::outline_button(
                        "release-notes",
                        "Show release notes",
                        true,
                        &colors,
                        Some(Box::new(cx.listener(|_, _, _, cx| {
                            cx.emit(super::OpenSettingsPage("changelog"))
                        }))),
                    )))
                    .when(macos, |view| {
                        view.child(self.switch_row(
                            "show_app_in_dock",
                            "Show app in Dock",
                            None,
                            true,
                            &colors,
                            cx,
                        ))
                    })
                    .child(self.switch_row(
                        "show_tray_icon",
                        "Show tray icon",
                        None,
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
                                        None,
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
                                                None,
                                                true,
                                                &colors,
                                                cx,
                                            ))
                                            .child(self.spoken_languages(&colors, window, cx)),
                                    ),
                            )
                            .child(setting_row(
                                "Timezone",
                                None,
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
                                None,
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
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .children(self.data_location(&colors))
                            .child(self.export_location(&colors, cx)),
                    ),
            )
            .into_any_element()
    }

    /// `settings/general/spoken-languages.tsx`: chips plus a search field that suggests
    /// languages as you type; Backspace on an empty field removes the last chip.
    fn spoken_languages(&self, colors: &Theme, window: &Window, cx: &mut Context<Self>) -> Div {
        let main = self.text("ai_language");
        let selected = locale::additional_spoken(&main, &self.text("spoken_languages"));
        let writable = self.writable("spoken_languages");
        let query = self.spoken_input.read(cx).buffer.text.clone();
        let focused = self.spoken_input.focus_handle(cx).is_focused(window);
        let matches = locale::spoken_matches(&query, &main, &selected);
        let open = focused && !query.trim().is_empty();
        let field = div()
            .id("spoken-languages")
            .min_h(px(38.))
            .w_full()
            .px_2()
            .py(px(6.))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(6.))
            .border_1()
            .border_color(colors.border)
            .bg(colors.card)
            .text_sm()
            .cursor_text()
            .on_click(cx.listener(|this, _, window, cx| {
                this.spoken_input
                    .update(cx, |input, cx| input.focus_handle(cx).focus(window, cx))
            }))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                this.spoken_key(event.keystroke.key.as_str(), cx)
            }))
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
                    .bg(colors.muted)
                    .text_xs()
                    .child(locale::language_name(code))
                    .child(
                        div()
                            .id(SharedString::from(format!("remove-language-{code}")))
                            .a11y_button(format!("Remove {}", locale::language_name(code)))
                            .a11y_disabled(!writable)
                            .ml(px(2.))
                            .when(writable, |view| {
                                view.cursor_pointer().on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.set_spoken(remaining.clone(), cx)
                                    },
                                ))
                            })
                            .child(controls::icon("product/XIcon.svg", 10., colors.foreground)),
                    )
            }))
            .when(selected.is_empty(), |view| {
                view.child(controls::icon(
                    "Search01Icon.svg",
                    16.,
                    colors.muted_foreground,
                ))
            })
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w(px(120.))
                    .h(px(20.))
                    .line_height(px(20.))
                    .when(selected.is_empty() && query.is_empty(), |view| {
                        view.child(
                            div()
                                .absolute()
                                .inset_0()
                                .text_color(colors.muted_foreground)
                                .child("Add language"),
                        )
                    })
                    .child(self.spoken_input.clone()),
            );
        let options = open.then(|| {
            deferred(
                div()
                    .id("language-options")
                    .a11y_group(gpui::Role::ListBox, "Languages")
                    .absolute()
                    .top_full()
                    .left_0()
                    .right_0()
                    .mt_1()
                    .max_h(px(240.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.card)
                    .shadow_md()
                    .occlude()
                    .when(matches.is_empty(), |view| {
                        view.child(
                            div()
                                .id("no-languages")
                                .a11y_text("No matching languages found")
                                .px_3()
                                .py_2()
                                .text_center()
                                .text_sm()
                                .text_color(colors.muted_foreground)
                                .child("No matching languages found"),
                        )
                    })
                    .children(matches.iter().enumerate().map(|(index, code)| {
                        let code = *code;
                        let hover = colors.accent;
                        div()
                            .id(SharedString::from(format!("language-option-{code}")))
                            .a11y_option(
                                locale::language_name(code),
                                self.spoken_index == Some(index),
                            )
                            .w_full()
                            .px_3()
                            .py_2()
                            .text_sm()
                            .font_weight(crate::ui::theme::MEDIUM)
                            .truncate()
                            .cursor_pointer()
                            .when(self.spoken_index == Some(index), |view| view.bg(hover))
                            .hover(move |style| style.bg(hover))
                            .on_click(cx.listener(move |this, _, _, cx| this.push_spoken(code, cx)))
                            .child(locale::language_name(code))
                    })),
            )
            .with_priority(1)
        });
        div()
            .child(
                div()
                    .mb_3()
                    .text_sm()
                    .line_height(px(20.))
                    .font_weight(crate::ui::theme::MEDIUM)
                    .child("Additional spoken languages"),
            )
            .child(div().relative().child(field).children(options))
    }

    fn spoken_key(&mut self, key: &str, cx: &mut Context<Self>) {
        let main = self.text("ai_language");
        let selected = locale::additional_spoken(&main, &self.text("spoken_languages"));
        let query = self.spoken_input.read(cx).buffer.text.clone();
        let matches = locale::spoken_matches(&query, &main, &selected);
        match key {
            "backspace" if query.is_empty() && !selected.is_empty() => {
                self.set_spoken(selected[..selected.len() - 1].to_vec(), cx);
            }
            "down" if !matches.is_empty() => {
                self.spoken_index = Some(
                    self.spoken_index
                        .map_or(0, |index| (index + 1).min(matches.len() - 1)),
                );
            }
            "up" if !matches.is_empty() => {
                self.spoken_index =
                    Some(self.spoken_index.map_or(0, |index| index.saturating_sub(1)));
            }
            "escape" if !query.is_empty() => {
                self.spoken_input
                    .update(cx, |input, cx| input.set_text(String::new(), cx));
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn add_spoken_language(&mut self, cx: &mut Context<Self>) {
        let main = self.text("ai_language");
        let selected = locale::additional_spoken(&main, &self.text("spoken_languages"));
        let query = self.spoken_input.read(cx).buffer.text.clone();
        let matches = locale::spoken_matches(&query, &main, &selected);
        if let Some(code) = self.spoken_index.and_then(|index| matches.get(index)) {
            self.push_spoken(code, cx);
        }
    }

    fn push_spoken(&mut self, code: &str, cx: &mut Context<Self>) {
        let main = self.text("ai_language");
        let mut selected = locale::additional_spoken(&main, &self.text("spoken_languages"));
        selected.push(code.to_owned());
        self.set_spoken(selected, cx);
        self.spoken_index = None;
        self.spoken_input
            .update(cx, |input, cx| input.set_text(String::new(), cx));
    }

    fn set_spoken(&mut self, codes: Vec<String>, cx: &mut Context<Self>) {
        if self.writable("spoken_languages") {
            self.set("spoken_languages", json!(json!(codes).to_string()), cx);
        }
    }

    fn folder_location_row(
        &self,
        row_id: &'static str,
        button_id: &'static str,
        title: &'static str,
        location: &std::path::Path,
        colors: &Theme,
    ) -> Div {
        let path = location.to_string_lossy().into_owned();
        let shown = tildify_path(&path);
        let row_path = location.to_path_buf();
        let button_path = location.to_path_buf();
        let hover = colors.muted.opacity(0.4);
        div()
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .id(row_id)
                    .a11y_button(format!("{title}, {shown}"))
                    .flex_1()
                    .min_w_0()
                    .px_2()
                    .py_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .on_click(move |_, _, cx| cx.open_with_system(&row_path))
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
                                    .font_weight(crate::ui::theme::MEDIUM)
                                    .child(title),
                            )
                            .child(controls::description(shown, colors).truncate()),
                    ),
            )
            .child(
                div().w(px(144.)).flex().child(
                    controls::outline_button(
                        button_id,
                        "Open folder",
                        true,
                        colors,
                        Some(Box::new(move |_, _, cx| cx.open_with_system(&button_path))),
                    )
                    .w_full(),
                ),
            )
    }

    fn data_location(&self, colors: &Theme) -> Vec<Div> {
        let Some(locations) = self.storage_locations.as_ref() else {
            return Vec::new();
        };
        let mut rows = vec![self.folder_location_row(
            "open-data-location",
            "open-data-folder",
            "Data location",
            &locations.data_dir,
            colors,
        )];
        if let Some(database_dir) = locations.separate_database_dir() {
            rows.push(self.folder_location_row(
                "open-database-location",
                "open-database-folder",
                "Notes database",
                database_dir,
                colors,
            ));
        }
        rows
    }

    /// `settings/general/storage/export-location.tsx`.
    fn export_location(&self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let stored = self.text("export_directory");
        let path = if stored.is_empty() {
            std::env::home_dir()
                .map(|home| home.join("Downloads").to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            stored.clone()
        };
        let shown = tildify_path(&path);
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
                            .a11y_button(format!("Export location, {shown}"))
                            .flex_1()
                            .min_w_0()
                            .px_2()
                            .py_2()
                            .flex()
                            .items_center()
                            .gap_2()
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
                                            .font_weight(crate::ui::theme::MEDIUM)
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
            .when(!stored.is_empty(), |view| {
                view.child(
                    div()
                        .id("reset-export-folder")
                        .a11y_button("Reset to Downloads")
                        .px_2()
                        .text_xs()
                        .font_weight(crate::ui::theme::MEDIUM)
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
                    .child(controls::section_title("Theme"))
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
                                    .a11y_radio(label, selected)
                                    .aria_description(detail)
                                    .a11y_disabled(!writable)
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
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
                                                            .font_weight(crate::ui::theme::MEDIUM)
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
                None,
                true,
                &colors,
                cx,
            ))
            .when(cfg!(target_os = "macos"), |view| {
                view.child(self.app_icons(&colors, window, cx))
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(controls::section_title("Notes list"))
                    .child(self.switch_row(
                        "sidebar_show_folder",
                        "Folder",
                        None,
                        true,
                        &colors,
                        cx,
                    ))
                    .child(self.switch_row("sidebar_show_tags", "Tags", None, true, &colors, cx)),
            )
            .into_any_element()
    }

    /// `settings/appearance/app-icon.tsx`: the Dock icon choices; other plans can only keep
    /// the current one.
    fn app_icons(&self, colors: &Theme, window: &Window, cx: &mut Context<Self>) -> Div {
        const OPTIONS: [(&str, &str); 10] = [
            ("default", "Default"),
            ("stable", "Production"),
            ("anagram", "Anagram"),
            ("dev", "Blueprint"),
            ("staging", "Sketch"),
            ("journal", "Field Journal"),
            ("notepad", "Notepad"),
            ("stone", "Stone"),
            ("typewriter-key", "Typewriter Key"),
            ("walnut", "Walnut"),
        ];
        let pro = super::account::is_pro(cx);
        let stored = self.text("app_icon");
        let default = crate::platform::system_settings::DEFAULT_ICON;
        fn resolve(value: &'static str) -> &'static str {
            if value == "default" {
                crate::platform::system_settings::DEFAULT_ICON
            } else {
                value
            }
        }
        let current = OPTIONS
            .iter()
            .find(|(value, _)| *value == stored)
            .map_or(default, |(value, _)| resolve(value));
        let dark = matches!(
            window.appearance(),
            gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
        );
        let sky = |light: u32, dark_color: u32, alpha: f32| {
            Hsla::from(gpui::rgb(if dark { dark_color } else { light })).opacity(alpha)
        };
        let writable = self.writable("app_icon");
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(controls::section_title("App icon"))
            .child(
                div().flex().flex_wrap().gap_3().children(
                    OPTIONS
                        .into_iter()
                        .filter(|(value, _)| *value == "default" || *value != default)
                        .map(|(value, _)| {
                            let name = resolve(value);
                            let selected = name == current;
                            let locked = !pro && !selected;
                            let preview =
                                if matches!(name, "stable" | "anagram" | "dev" | "staging") {
                                    format!(
                                        "product/app-icons/{name}-{}.png",
                                        if dark { "dark" } else { "light" }
                                    )
                                } else {
                                    format!("product/app-icons/{name}.png")
                                };
                            div()
                                .id(SharedString::from(format!("app-icon-{value}")))
                                .a11y_radio(
                                    if locked {
                                        format!("{name}, Requires Anarlog Pro")
                                    } else {
                                        name.to_owned()
                                    },
                                    selected,
                                )
                                .group(SharedString::from(format!("app-icon-{value}")))
                                .relative()
                                .p(px(2.))
                                .pb(px(10.))
                                .cursor_pointer()
                                .when(selected, |view| {
                                    view.child(
                                        div()
                                            .absolute()
                                            .bottom(px(2.))
                                            .left(px(11.))
                                            .w(px(56.))
                                            .h(px(16.))
                                            .shadow(vec![gpui::BoxShadow {
                                                color: sky(0x38bdf8, 0x7dd3fc, 0.6),
                                                offset: gpui::point(px(0.), px(0.)),
                                                blur_radius: px(12.),
                                                spread_radius: px(0.),
                                                inset: false,
                                            }]),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .bottom(px(6.))
                                            .left(px(23.))
                                            .w(px(32.))
                                            .h(px(6.))
                                            .shadow(vec![gpui::BoxShadow {
                                                color: sky(0x7dd3fc, 0xbae6fd, 0.9),
                                                offset: gpui::point(px(0.), px(0.)),
                                                blur_radius: px(3.),
                                                spread_radius: px(0.),
                                                inset: false,
                                            }]),
                                    )
                                })
                                .child(
                                    div()
                                        .relative()
                                        .size(px(64.))
                                        .when(selected, |view| view.top(px(-6.)))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .overflow_hidden()
                                        .child(
                                            gpui::img(SharedString::from(preview))
                                                .flex_shrink_0()
                                                .size(px(74.24))
                                                .group_hover(
                                                    SharedString::from(format!("app-icon-{value}")),
                                                    |style| style.size(px(77.44)),
                                                ),
                                        ),
                                )
                                .when(locked, |view| {
                                    view.child(
                                        div()
                                            .absolute()
                                            .right(px(4.))
                                            .bottom(px(12.))
                                            .size(px(20.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .border_1()
                                            .border_color(colors.border)
                                            .bg(colors.background)
                                            .shadow_xs()
                                            .child(controls::icon(
                                                "product/LockPasswordIcon.svg",
                                                12.,
                                                colors.muted_foreground,
                                            )),
                                    )
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if locked {
                                        cx.emit(super::PlanRequired);
                                    } else if writable {
                                        this.set("app_icon", json!(value), cx);
                                    }
                                }))
                        }),
                ),
            )
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
                        None,
                        true,
                        &colors,
                        cx,
                    ))
                    .child(self.switch_row(
                        "notification_summary_complete",
                        "Summary complete",
                        None,
                        enabled,
                        &colors,
                        cx,
                    ))
                    .child(self.switch_row(
                        "notification_cloudsync_complete",
                        "Cloud sync complete",
                        None,
                        enabled,
                        &colors,
                        cx,
                    ))
                    .child(self.switch_row(
                        "notification_event",
                        "Event notifications",
                        None,
                        enabled,
                        &colors,
                        cx,
                    ))
                    .child(self.switch_row(
                        "notification_recording",
                        "Recording status prompts",
                        None,
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
                                None,
                                enabled,
                                &colors,
                                cx,
                            ))
                            .when(self.flag("notification_completion_sound"), |view| {
                                view.child(
                                    controls::nested(&colors).child(setting_row(
                                        "Sound",
                                        None,
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
                                                        preview_sound(&value);
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
                                            .child(
                                                controls::outline_button(
                                                    "preview-sound",
                                                    "Preview",
                                                    enabled,
                                                    &colors,
                                                    Some(Box::new(move |_, _, _| {
                                                        preview_sound(&sound)
                                                    })),
                                                )
                                                .h(px(28.))
                                                .px_2()
                                                .text_xs(),
                                            ),
                                        &colors,
                                    )),
                                )
                            }),
                    )
                    .when(bounce, |view| {
                        view.child(self.switch_row(
                            "notification_bounce",
                            "Bounce app icon",
                            None,
                            enabled,
                            &colors,
                            cx,
                        ))
                    })
                    .when(!cfg!(target_os = "windows"), |view| {
                        view.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_4()
                                .child(self.switch_row(
                                    "notification_detect",
                                    "Microphone detection",
                                    None,
                                    enabled,
                                    &colors,
                                    cx,
                                ))
                                .when(detect && enabled, |view| {
                                    view.child(self.detection_settings(&colors, window, cx))
                                }),
                        )
                    })
                    .when(cfg!(target_os = "macos"), |view| {
                        view.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_6()
                                .child(
                                    div()
                                        .pt_4()
                                        .pb_2()
                                        .flex()
                                        .items_center()
                                        .gap_4()
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .border_t_1()
                                                .border_color(colors.muted),
                                        )
                                        .child(
                                            div()
                                                .flex_shrink_0()
                                                .text_xs()
                                                .font_weight(crate::ui::theme::MEDIUM)
                                                .text_color(colors.muted_foreground)
                                                .child("For enabled notifications"),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .border_t_1()
                                                .border_color(colors.muted),
                                        ),
                                )
                                .child(self.switch_row(
                                    "respect_dnd",
                                    "Respect Do-Not-Disturb mode",
                                    None,
                                    enabled && (self.flag("notification_event") || detect),
                                    &colors,
                                    cx,
                                )),
                        )
                    }),
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
            .child(
                div()
                    .id("detection-delay")
                    .a11y_group(gpui::Role::Group, "Detection delay")
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .line_height(px(20.))
                            .font_weight(crate::ui::theme::MEDIUM)
                            .child("Detection delay"),
                    )
                    .child(
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
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(crate::ui::theme::MEDIUM)
                            .child("Exclude apps from detection"),
                    )
                    .child(self.excluded_apps(colors, window, cx)),
            )
    }

    /// The excluded-apps badges and their installed-apps search popover.
    fn excluded_apps(&self, colors: &Theme, window: &Window, cx: &mut Context<Self>) -> Div {
        use crate::meeting::automation::detect;
        const MENU: &str = "excluded-apps";
        let installed = crate::meeting::automation::installed_apps(cx);
        let ignored = detect::parse_ids(&self.text("ignored_platforms"));
        let included = detect::parse_ids(&self.text("included_platforms"));
        let writable = self.writable("ignored_platforms") && self.writable("included_platforms");
        let name = |id: &str| {
            installed
                .iter()
                .find(|app| app.id == id)
                .map_or_else(|| id.to_owned(), |app| app.name.clone())
        };
        let badges = detect::excluded_ids(&installed, &ignored, &included)
            .into_iter()
            .enumerate()
            .map(|(index, id)| {
                let default = detect::DEFAULT_IGNORED.contains(&id.as_str());
                let text = if default {
                    colors.muted_foreground
                } else {
                    colors.secondary_foreground
                };
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .py(px(2.))
                    .border_1()
                    .border_color(gpui::transparent_black())
                    .bg(if default { colors.accent } else { colors.muted })
                    .text_xs()
                    .line_height(px(16.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(text)
                    .child(name(&id))
                    .when(default, |view| {
                        view.child(div().text_size(px(10.)).opacity(0.7).child("(default)"))
                    })
                    .child(
                        div()
                            .id(("excluded-app-remove", index))
                            .a11y_button(format!("Remove {}", name(&id)))
                            .a11y_disabled(!writable)
                            .ml(px(2.))
                            .size(px(12.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .child(controls::icon("chat/XIcon.svg", 10., text))
                            .when(writable, |view| {
                                view.on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.toggle_excluded(&id, cx);
                                }))
                            }),
                    )
            });
        let trigger = div()
            .id(MENU)
            .role(gpui::Role::ComboBox)
            .aria_label("Search installed apps...")
            .aria_expanded(self.menu == Some(MENU))
            .min_h(px(38.))
            .w_full()
            .p_2()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .border_1()
            .border_color(colors.border)
            .cursor_text()
            .children(badges)
            .child(
                div()
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child("Search installed apps..."),
            )
            .when(writable, |view| {
                view.on_click(cx.listener(|this, _, window, cx| {
                    this.menu = Some(MENU);
                    this.menu_search.update(cx, |search, cx| {
                        search.set_placeholder("Search installed apps...");
                        search.set_text(String::new(), cx);
                        search.focus_handle(cx).focus(window, cx);
                    });
                    cx.notify();
                }))
            });
        let menu = (self.menu == Some(MENU)).then(|| {
            let query = self.menu_search.read(cx).buffer.text.clone();
            let apps = detect::excludable(&installed, &ignored, &included, &query);
            let empty = apps.is_empty();
            div()
                .p(px(2.))
                .border_1()
                .border_color(colors.app_floating_border)
                .bg(colors.app_floating_chrome)
                .text_color(colors.popover_foreground)
                .shadow_lg()
                .occlude()
                .child(
                    div()
                        .border_1()
                        .border_color(colors.app_floating_border)
                        .bg(colors.app_floating_panel)
                        .overflow_hidden()
                        .child(
                            div()
                                .h(px(40.))
                                .px_3()
                                .flex()
                                .items_center()
                                .gap_2()
                                .border_b_1()
                                .border_color(colors.border)
                                .text_sm()
                                .child(controls::icon(
                                    "Search01Icon.svg",
                                    16.,
                                    colors.foreground.opacity(0.5),
                                ))
                                .child(div().flex_1().h(px(20.)).child(self.menu_search.clone())),
                        )
                        .when(empty, |view| {
                            view.child(
                                div().py_6().flex().justify_center().child(
                                    div()
                                        .px_2()
                                        .py(px(6.))
                                        .text_sm()
                                        .text_color(colors.muted_foreground)
                                        .child("No apps found."),
                                ),
                            )
                        })
                        .when(!empty, |view| {
                            view.child(
                                div()
                                    .id("excluded-apps-options")
                                    .max_h(px(250.))
                                    .overflow_y_scroll()
                                    .p_1()
                                    .flex()
                                    .flex_col()
                                    .children(apps.into_iter().enumerate().map(|(index, app)| {
                                        let hover = colors.accent;
                                        div()
                                            .id(("excluded-app-option", index))
                                            .a11y_option(app.name.clone(), false)
                                            .px_2()
                                            .py(px(6.))
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .text_sm()
                                            .cursor_pointer()
                                            .hover(move |style| style.bg(hover))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .truncate()
                                                    .child(app.name.clone()),
                                            )
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.toggle_excluded(&app.id, cx)
                                            }))
                                    })),
                            )
                        }),
                )
        });
        div()
            .relative()
            .w_full()
            .child(trigger)
            .when_some(menu, |view, menu| {
                view.child(controls::backdrop(
                    |this: &mut Self| this.menu = None,
                    window,
                    cx,
                ))
                .child(
                    deferred(
                        div()
                            .absolute()
                            .top_full()
                            .left_0()
                            .right_0()
                            .mt_1()
                            .child(menu),
                    )
                    .with_priority(1),
                )
            })
    }

    /// Shipping's `handleToggleIgnoredApp`: saves both lists and closes the search.
    fn toggle_excluded(&mut self, id: &str, cx: &mut Context<Self>) {
        use crate::meeting::automation::detect;
        let (ignored, included) = detect::toggle_excluded(
            id,
            &detect::parse_ids(&self.text("ignored_platforms")),
            &detect::parse_ids(&self.text("included_platforms")),
        );
        self.menu = None;
        self.set("ignored_platforms", json!(json!(ignored).to_string()), cx);
        self.set("included_platforms", json!(json!(included).to_string()), cx);
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
                        None,
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
                    .child(self.primary_recording_device(window, cx))
                    .child(plan_gate(
                        pro,
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(self.gated_switch_row(
                                "auto_start_scheduled_meetings",
                                "Start when meeting begins",
                                None,
                                pro,
                                true,
                                &colors,
                                cx,
                            ))
                            .child(self.gated_switch_row(
                                "auto_join_scheduled_meetings",
                                "Join scheduled meetings",
                                None,
                                pro,
                                self.flag("auto_start_scheduled_meetings"),
                                &colors,
                                cx,
                            )),
                        cx,
                    ))
                    .when(!windows, |view| {
                        view.child(self.switch_row(
                            "auto_stop_meetings",
                            "Stop when meeting ends",
                            None,
                            true,
                            &colors,
                            cx,
                        ))
                    })
                    .when(!windows, |view| {
                        view.child(self.switch_row(
                            "consent_auto_send_chat",
                            "Post recording disclosure in meeting chat",
                            None,
                            true,
                            &colors,
                            cx,
                        ))
                        .child(self.switch_row(
                            "capture_meeting_chat",
                            "Capture meeting chat in Memos",
                            None,
                            true,
                            &colors,
                            cx,
                        ))
                    })
                    .when(macos || windows, |view| {
                        view.child(self.switch_row(
                            "capture_shared_screens",
                            "Capture shared screens in Memos",
                            None,
                            true,
                            &colors,
                            cx,
                        ))
                    })
                    .child(self.switch_row(
                        "floating_bar_enabled",
                        "Show floating bar",
                        None,
                        true,
                        &colors,
                        cx,
                    )),
            )
            .child(
                div().child(controls::section_title("Audio").mb_4()).child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(self.microphone_row(window, cx))
                        .child(setting_row(
                            "Audio file retention",
                            None,
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
                            None,
                            true,
                            &colors,
                            cx,
                        )),
                ),
            )
            .into_any_element()
    }

    /// `AudioDeviceRow`: the current default, then the devices sorted by name; a stored
    /// device that is not connected stays listed as unavailable.
    fn microphone_row(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(window);
        let stored = self.text("microphone_device");
        let mut devices = self.microphones.clone();
        devices.sort();
        devices.dedup();
        let unavailable = !stored.is_empty() && !devices.contains(&stored);
        if unavailable {
            devices.insert(0, stored.clone());
        }
        let options = std::iter::once(SelectOption::new("", "Current default"))
            .chain(devices.into_iter().map(|device| {
                let label = if unavailable && device == stored {
                    format!("{device} (Unavailable — using current default)")
                } else {
                    device.clone()
                };
                SelectOption::new(device, label)
            }))
            .collect();
        setting_row(
            "Microphone",
            None,
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

    /// `PrimaryRecordingDeviceSelector`: Pro accounts can prefer this computer.
    fn primary_recording_device(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(window);
        let account = super::account::current(cx);
        let pro = super::account::is_pro(cx);
        let stored = self.text("primary_recording_device");
        let recorder = account
            .devices
            .as_ref()
            .and_then(|devices| devices.recorder.clone());
        let mut options = vec![SelectOption::new("ask", "Ask each meeting")];
        if let Some(recorder) = &recorder {
            options.push(SelectOption::new(
                recorder.fingerprint.to_string(),
                recorder.name.to_string(),
            ));
        }
        if !stored.is_empty() && recorder.is_none_or(|recorder| *recorder.fingerprint != *stored) {
            options.push(SelectOption::new(stored.clone(), "Unavailable device"));
        }
        let selected = if stored.is_empty() {
            "ask".into()
        } else {
            stored
        };
        setting_row(
            "Primary recording device",
            if !account.signed_in() {
                Some("Sign in to choose a primary recording device.".into())
            } else if !pro {
                Some("Anarlog Pro coordinates recording across your devices.".into())
            } else {
                None
            },
            true,
            self.dropdown_select(
                "primary_recording_device",
                options,
                &selected,
                "Choose a device",
                account.signed_in()
                    && pro
                    && account.devices.is_some()
                    && self.writable("primary_recording_device"),
                false,
                controls::CONTROL_WIDTH,
                |this, value, cx| {
                    let value = if value == "ask" { String::new() } else { value };
                    this.set("primary_recording_device", json!(value), cx)
                },
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
                    .child({
                        let available = self.lock_available;
                        let checked = self.flag("lock_app") && available;
                        setting_row(
                            "Lock app",
                            (!available).then_some(
                                "Device authentication is not available on this computer.".into(),
                            ),
                            false,
                            controls::switch(
                                "switch-lock_app",
                                checked,
                                available && !self.authenticating && self.writable("lock_app"),
                                &colors,
                                cx.listener(move |this, _, _, cx| this.toggle_lock(!checked, cx)),
                            )
                            .aria_label("Lock app"),
                            &colors,
                        )
                    })
                    .child(self.switch_row(
                        "telemetry_consent",
                        "Share usage data",
                        None,
                        true,
                        &colors,
                        cx,
                    ))
                    .child(self.switch_row(
                        "crash_reporting_consent",
                        "Share error reports",
                        None,
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
        let keys = super::dictation_shortcut::labels(&self.text("dictation_shortcut"));
        let chip = |label: String| {
            div()
                .px(px(6.))
                .py(px(2.))
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
                    .child(self.gated_switch_row(
                        "dictation_enabled",
                        "Enable dictation",
                        None,
                        pro,
                        true,
                        &colors,
                        cx,
                    ))
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
                                            .font_weight(crate::ui::theme::MEDIUM)
                                            .child("Dictation shortcut"),
                                    )
                                    .child(
                                        div()
                                            .id("dictation-shortcut")
                                            .a11y_button(if self.recording_shortcut {
                                                "Press shortcut…".to_owned()
                                            } else {
                                                format!("Dictation shortcut, {}", keys.join(" "))
                                            })
                                            .track_focus(&self.shortcut_focus)
                                            .h(px(36.))
                                            .px_4()
                                            .flex()
                                            .items_center()
                                            .gap_1()
                                            .border_1()
                                            .border_color(colors.input)
                                            .bg(colors.background)
                                            .text_sm()
                                            .font_weight(crate::ui::theme::MEDIUM)
                                            .shadow_xs()
                                            .cursor_pointer()
                                            .hover(|style| style.bg(colors.accent))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.toggle_shortcut_recording(window, cx)
                                            }))
                                            .on_key_down(cx.listener(
                                                |this, event: &KeyDownEvent, _, cx| {
                                                    this.record_shortcut(event, cx)
                                                },
                                            ))
                                            .when(self.recording_shortcut, |view| {
                                                view.child("Press shortcut…")
                                            })
                                            .when(!self.recording_shortcut, |view| {
                                                view.children(keys.into_iter().map(chip))
                                            }),
                                    ),
                            )
                            .when_some(self.shortcut_error.clone(), |view, error| {
                                view.child(
                                    div()
                                        .id("shortcut-error")
                                        .a11y_status(error.clone(), true)
                                        .text_sm()
                                        .text_color(colors.destructive)
                                        .child(error),
                                )
                            })
                            .when(cfg!(target_os = "macos"), |view| {
                                view.child(
                                    div().flex().gap_2().children(
                                        [("Fn", "Fn / Globe"), ("RightCommand", "Right Command")]
                                            .into_iter()
                                            .map(|(value, label)| {
                                                div()
                                                    .id(value)
                                                    .a11y_button(label)
                                                    .h(px(28.))
                                                    .px_2()
                                                    .flex()
                                                    .items_center()
                                                    .border_1()
                                                    .border_color(colors.input)
                                                    .bg(colors.background)
                                                    .shadow_xs()
                                                    .text_xs()
                                                    .font_weight(crate::ui::theme::MEDIUM)
                                                    .cursor_pointer()
                                                    .hover(|style| style.bg(colors.accent))
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.save_shortcut(value.into(), cx)
                                                    }))
                                                    .child(label)
                                            }),
                                    ),
                                )
                            }),
                    )
                    .child(self.microphone_row(window, cx))
                    .child(self.gated_switch_row(
                        "dictation_live_preview",
                        "Live transcript preview",
                        None,
                        pro,
                        true,
                        &colors,
                        cx,
                    ))
                    .child(self.gated_switch_row(
                        "dictation_hands_free",
                        "Hands-free dictation",
                        None,
                        pro,
                        true,
                        &colors,
                        cx,
                    ))
                    .child(
                        div().flex().flex_wrap().gap_2().children(
                            [
                                ("Permissions", "permissions"),
                                ("Language & transcription", "transcription"),
                                ("Dictionary", "dictionary"),
                            ]
                            .map(|(label, page)| {
                                let open = cx.listener(move |_, _: &gpui::ClickEvent, _, cx| {
                                    cx.emit(super::OpenSettingsPage(page))
                                });
                                controls::outline_button(
                                    label,
                                    label,
                                    true,
                                    &colors,
                                    Some(Box::new(open)),
                                )
                            }),
                        ),
                    )
                    .when(self.flag("dictation_enabled"), |view| {
                        view.child(self.dictation_status(&colors, cx))
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .text_sm()
                            .child("Try dictation")
                            .child(
                                div()
                                    .min_h(px(64.))
                                    .px_3()
                                    .py_2()
                                    .border_1()
                                    .border_color(colors.input)
                                    .shadow_xs()
                                    .child(self.dictation_try.clone()),
                            ),
                    )
                    .children(self.last_dictation(&colors, cx)),
                cx,
            ))
            .into_any_element()
    }

    /// The `role="status"` card: the dictation phase, setup errors with Retry, and Cancel.
    fn dictation_status(&self, colors: &Theme, cx: &mut Context<Self>) -> AnyElement {
        use crate::dictation::{DictationStatus, controller::Phase};
        let Some(status) = DictationStatus::global(cx) else {
            return div().into_any_element();
        };
        let current = status.read(cx);
        let label = match current.phase {
            Phase::Recording => "Listening…",
            Phase::Transcribing => "Transcribing…",
            Phase::Starting => "Starting microphone…",
            Phase::Idle if current.ready => "Ready to dictate",
            Phase::Idle => "Setting up dictation…",
        };
        let error = current.error.clone();
        let active = current.phase != Phase::Idle;
        let retry = status.clone();
        let cancel = status.clone();
        div()
            .p_4()
            .border_1()
            .border_color(colors.border)
            .text_sm()
            .child(label)
            .when_some(error, |view, error| {
                view.child(div().mt_2().text_color(colors.destructive).child(error))
                    .child(
                        controls::outline_button(
                            "dictation-retry",
                            "Retry setup",
                            true,
                            colors,
                            Some(Box::new(move |_, _, cx| {
                                retry.update(cx, |status, cx| status.retry(cx))
                            })),
                        )
                        .mt_2(),
                    )
            })
            .when(active, |view| {
                view.child(
                    controls::outline_button(
                        "dictation-cancel",
                        "Cancel dictation",
                        true,
                        colors,
                        Some(Box::new(move |_, _, cx| {
                            cancel.update(cx, |status, cx| status.cancel(cx))
                        })),
                    )
                    .mt_2(),
                )
            })
            .into_any_element()
    }

    /// "Last dictation" with its copy button, while a transcript is kept in memory.
    fn last_dictation(&self, colors: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let transcript = crate::dictation::DictationStatus::global(cx)?
            .read(cx)
            .last_transcript
            .clone();
        if transcript.is_empty() {
            return None;
        }
        let copied = self.dictation_copied.as_ref() == Some(&transcript);
        let copy = transcript.clone();
        Some(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_sm()
                        .font_weight(crate::ui::theme::MEDIUM)
                        .child("Last dictation"),
                )
                .child(
                    div()
                        .p_4()
                        .border_1()
                        .border_color(colors.border)
                        .text_sm()
                        .child(transcript),
                )
                .child(div().flex().child(controls::outline_button(
                    "dictation-copy",
                    if copied {
                        "Copied"
                    } else {
                        "Copy last dictation"
                    },
                    true,
                    colors,
                    Some(Box::new(cx.listener(
                        move |this, _: &gpui::ClickEvent, _, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                copy.to_string(),
                            ));
                            this.dictation_copied = Some(copy.clone());
                            cx.notify();
                        },
                    ))),
                )))
                .into_any_element(),
        )
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
                                    .bg(colors.foreground)
                                    .text_color(colors.background)
                                    .text_sm()
                                    .font_weight(crate::ui::theme::MEDIUM)
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
                                .border_1()
                                .border_color(colors.border)
                                .bg(colors.card)
                                .text_center()
                                .child(controls::icon("product/BookOpen01Icon.svg", 20., colors.muted_foreground).mb_3())
                                .child(div().text_sm().font_weight(crate::ui::theme::MEDIUM).child("Your dictionary is empty"))
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
            cx))
            .into_any_element()
    }

    fn toggle_shortcut_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_recording_shortcut(!self.recording_shortcut, cx);
        if self.recording_shortcut {
            self.shortcut_error = None;
            self.shortcut_focus.focus(window, cx);
        }
        cx.notify();
    }

    fn record_shortcut(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        use super::dictation_shortcut::{Recorded, record};
        if !self.recording_shortcut {
            return;
        }
        cx.stop_propagation();
        if event.is_held {
            return;
        }
        match record(&event.keystroke) {
            Recorded::Cancel => {
                self.set_recording_shortcut(false, cx);
                cx.notify();
            }
            Recorded::Ignore => {}
            Recorded::Shortcut(shortcut) => self.save_shortcut(shortcut, cx),
        }
    }

    /// Validates before saving, like the shortcut plugin; an invalid choice keeps the old one.
    fn save_shortcut(&mut self, shortcut: String, cx: &mut Context<Self>) {
        self.set_recording_shortcut(false, cx);
        match crate::dictation::shortcut::validate(&shortcut) {
            Ok(()) => {
                self.shortcut_error = None;
                self.set("dictation_shortcut", json!(shortcut), cx);
            }
            Err(error) => self.shortcut_error = Some(error.to_string()),
        }
        cx.notify();
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

fn preview_sound(name: &str) {
    if let Some(name) = crate::platform::sounds::COMPLETION_SOUNDS
        .into_iter()
        .find(|sound| *sound == name)
    {
        crate::platform::sounds::play_completion(name);
    }
}

/// Shipping's `PlanGate`: content stays visible at 60% opacity and ignores input.
fn plan_gate(allowed: bool, content: Div, cx: &mut Context<PreferencesView>) -> Div {
    super::account_view::plan_gate(
        allowed,
        content,
        cx.listener(|_, _, _, cx| cx.emit(super::PlanRequired)),
    )
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
        div().w(width).h(px(height)).bg(color)
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
                .child(div().size(px(6.)).bg(gpui::rgb(0xf87171)))
                .child(div().size(px(6.)).bg(gpui::rgb(0xfbbf24)))
                .child(div().size(px(6.)).bg(gpui::rgb(0x4ade80))),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .gap_3()
                .child(div().w(gpui::relative(0.25)).bg(sidebar))
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

    #[test]
    fn tildify_path_respects_directory_boundaries() {
        let home = std::path::Path::new("/Users/john");
        assert_eq!(tildify_path_in("/Users/john", Some(home)), "~");
        assert_eq!(
            tildify_path_in(
                "/Users/john/Library/Application Support/anarlog",
                Some(home)
            ),
            "~/Library/Application Support/anarlog"
        );
        assert_eq!(
            tildify_path_in(
                "/Users/john/Library/Application Support/anarlog",
                Some(std::path::Path::new("/Users/john/"))
            ),
            "~/Library/Application Support/anarlog"
        );
        assert_eq!(
            tildify_path_in("/Users/john2/foo", Some(home)),
            "/Users/john2/foo"
        );
        assert_eq!(
            tildify_path_in(
                r"C:\Users\john\AppData\Roaming\anarlog",
                Some(std::path::Path::new(r"C:\Users\john\"))
            ),
            r"~\AppData\Roaming\anarlog"
        );
        assert_eq!(
            tildify_path_in("/Users/john/anarlog", None),
            "/Users/john/anarlog"
        );
    }
}
