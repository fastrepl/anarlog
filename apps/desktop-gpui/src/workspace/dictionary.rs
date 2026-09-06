//! `settings/dictionary/index.tsx` + `stt/keywords.ts`: the Dictionary
//! settings page over `personalization_dictionary_terms`, behind
//! `PlanGate plan="pro"`.

use gpui::{Context, Div, Focusable, SharedString, Window, div, prelude::*, px};

use super::Workspace;
use crate::text_input::{TextInput, TextInputEvent, TextInputStyle};
use crate::ui::{TailwindText, icon};

/// `normalizeKeywordList`: trimmed, inner whitespace collapsed, at least two
/// characters, case-insensitively unique in first-seen order.
pub fn normalize_keyword_list<'a>(words: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for word in words {
        let normalized = word.split_whitespace().collect::<Vec<_>>().join(" ");
        let key = normalized.to_lowercase();
        if normalized.chars().count() < 2 || seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        result.push(normalized);
    }
    result
}

/// `parseDictionaryTermsText`: newline / comma separated terms.
pub fn parse_dictionary_terms_text(value: &str) -> Vec<String> {
    normalize_keyword_list(
        value
            .split(['\n', ','])
            .map(str::trim)
            .filter(|term| !term.is_empty()),
    )
}

/// `parseDictionaryTermsJson`: the stored `"[...]"` string (or a JSON array).
pub fn parse_dictionary_terms_json(value: &str) -> Vec<String> {
    match serde_json::from_str::<serde_json::Value>(value) {
        Ok(serde_json::Value::Array(items)) => {
            normalize_keyword_list(items.iter().filter_map(|item| item.as_str()))
        }
        Ok(serde_json::Value::String(text)) => parse_dictionary_terms_json(&text),
        _ => Vec::new(),
    }
}

/// `appendDictionaryTerms`
pub fn append_dictionary_terms(terms: &[String], value: &str) -> Vec<String> {
    let parsed = parse_dictionary_terms_text(value);
    normalize_keyword_list(
        terms
            .iter()
            .map(String::as_str)
            .chain(parsed.iter().map(String::as_str)),
    )
}

/// `getEditedDictionaryTerm`: the normalised replacement, unless it is empty,
/// unchanged, or duplicates another term.
pub fn edited_dictionary_term(terms: &[String], current: &str, value: &str) -> Option<String> {
    let next = normalize_keyword_list([value]).into_iter().next()?;
    if next == current {
        return None;
    }
    let key = next.to_lowercase();
    let duplicate = terms
        .iter()
        .any(|term| term != current && term.to_lowercase() == key);
    (!duplicate).then_some(next)
}

/// `getVisibleDictionaryTerms`: the typed terms filter the list both ways.
pub fn visible_dictionary_terms(terms: &[String], value: &str) -> Vec<String> {
    let queries: Vec<String> = parse_dictionary_terms_text(value)
        .into_iter()
        .map(|term| term.to_lowercase())
        .collect();
    if queries.is_empty() {
        return terms.to_vec();
    }
    terms
        .iter()
        .filter(|term| {
            let key = term.to_lowercase();
            queries
                .iter()
                .any(|query| key.contains(query.as_str()) || query.contains(key.as_str()))
        })
        .cloned()
        .collect()
}

type RowAction = Box<dyn Fn(&mut Workspace, &mut Window, &mut Context<Workspace>)>;
type EditAction = Box<dyn Fn(&mut Workspace, &mut Context<Workspace>)>;

/// `DictionaryTermRow`'s inline edit state.
pub(crate) struct DictionaryEdit {
    pub term: String,
    pub input: gpui::Entity<TextInput>,
}

impl Workspace {
    /// `useConfigValue("personalization_dictionary_terms")`
    pub(crate) fn dictionary_terms(&self) -> Vec<String> {
        let stored = self
            .provider_settings
            .string_setting(
                "personalization_dictionary_terms",
                &["personalization", "dictionary_terms"],
            )
            .unwrap_or_else(|| "[]".to_string());
        parse_dictionary_terms_json(&stored)
    }

    fn save_dictionary_terms(&mut self, terms: Vec<String>, cx: &mut Context<Self>) {
        self.set_setting(
            "personalization_dictionary_terms",
            serde_json::Value::String(
                serde_json::to_string(&terms).unwrap_or_else(|_| "[]".to_string()),
            ),
            cx,
        );
    }

    /// `useBillingAccess().isPro`: unresolved without Supabase, so the page
    /// stays behind the gate like the Tauri app signed out.
    fn dictionary_allowed(&self) -> bool {
        false
    }

    pub(crate) fn ensure_dictionary_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dictionary_input.is_some() {
            return;
        }
        let theme = self.theme;
        let input = cx.new(|cx| {
            TextInput::new(
                "Add names, jargon, or product terms to prefer",
                TextInputStyle {
                    text: theme.foreground,
                    placeholder: theme.muted_foreground,
                    selection: theme.selection,
                    underline_when_focused: false,
                    masked: false,
                },
                window,
                cx,
            )
        });
        cx.subscribe_in(
            &input,
            window,
            |this, input, event: &TextInputEvent, window, cx| match event {
                TextInputEvent::Changed => cx.notify(),
                TextInputEvent::Enter => {
                    // `form.handleSubmit`
                    let value = input.read(cx).text().to_string();
                    this.add_dictionary_terms(&value, cx);
                    input.read(cx).focus_handle(cx).focus(window);
                }
                _ => {}
            },
        )
        .detach();
        self.dictionary_input = Some(input);
    }

    fn add_dictionary_terms(&mut self, value: &str, cx: &mut Context<Self>) {
        let terms = self.dictionary_terms();
        let next = append_dictionary_terms(&terms, value);
        if next.len() == terms.len() {
            return;
        }
        self.save_dictionary_terms(next, cx);
        if let Some(input) = self.dictionary_input.clone() {
            input.update(cx, |input, cx| input.set_text("", cx));
        }
    }

    fn remove_dictionary_term(&mut self, term: &str, cx: &mut Context<Self>) {
        let next: Vec<String> = self
            .dictionary_terms()
            .into_iter()
            .filter(|value| value != term)
            .collect();
        self.save_dictionary_terms(next, cx);
    }

    fn start_dictionary_edit(&mut self, term: &str, window: &mut Window, cx: &mut Context<Self>) {
        let theme = self.theme;
        let input = cx.new(|cx| {
            let mut input = TextInput::new(
                "",
                TextInputStyle {
                    text: theme.foreground,
                    placeholder: theme.muted_foreground,
                    selection: theme.selection,
                    underline_when_focused: false,
                    masked: false,
                },
                window,
                cx,
            );
            input.set_text(term.to_string(), cx);
            input
        });
        cx.subscribe_in(
            &input,
            window,
            |this, _, event: &TextInputEvent, _, cx| match event {
                TextInputEvent::Changed => cx.notify(),
                TextInputEvent::Enter => this.save_dictionary_edit(cx),
                TextInputEvent::Escape => {
                    this.dictionary_edit = None;
                    cx.notify();
                }
                _ => {}
            },
        )
        .detach();
        input.read(cx).focus_handle(cx).focus(window);
        self.dictionary_edit = Some(DictionaryEdit {
            term: term.to_string(),
            input,
        });
        cx.notify();
    }

    fn save_dictionary_edit(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = &self.dictionary_edit else {
            return;
        };
        let terms = self.dictionary_terms();
        let value = edit.input.read(cx).text().to_string();
        let Some(next_term) = edited_dictionary_term(&terms, &edit.term, &value) else {
            return;
        };
        let current = edit.term.clone();
        let next = terms
            .into_iter()
            .map(|term| {
                if term == current {
                    next_term.clone()
                } else {
                    term
                }
            })
            .collect();
        self.save_dictionary_terms(next, cx);
        self.dictionary_edit = None;
        cx.notify();
    }

    /// `SettingsDictionary`: the title, then `DictionarySettings` inside the
    /// gate (`opacity-60 cursor-not-allowed`, presses notify).
    pub(super) fn render_dictionary_settings(
        &self,
        title: impl IntoElement,
        cx: &Context<Self>,
    ) -> Div {
        let allowed = self.dictionary_allowed();
        let content = self.render_dictionary_form(allowed, cx);
        div()
            .flex()
            .flex_col()
            .gap_8()
            .child(title)
            .child(if allowed {
                content.into_any_element()
            } else {
                div()
                    .id("dictionary-plan-gate")
                    .relative()
                    .opacity(0.6)
                    .cursor_not_allowed()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _, cx| this.notify_pro_required(cx)),
                    )
                    .child(content)
                    .into_any_element()
            })
    }

    /// `DictionarySettings`: the `InputGroup` with the Add button, then the
    /// empty card, the `No match` line, or the term rows.
    fn render_dictionary_form(&self, allowed: bool, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let terms = self.dictionary_terms();
        let typed = self
            .dictionary_input
            .as_ref()
            .map(|input| input.read(cx).text().to_string())
            .unwrap_or_default();
        let can_add = append_dictionary_terms(&terms, &typed).len() != terms.len();
        let visible = visible_dictionary_terms(&terms, &typed);
        let has_search = !parse_dictionary_terms_text(&typed).is_empty();

        // `InputGroup rounded-full border-border bg-card shadow-none`
        // (`rounded-full` is 0.5rem): 36px tall, the `xs` Add button
        // (`h-6 px-2 rounded-full bg-black text-white`, `opacity-50` disabled)
        // inset by the addon's `pr-1.5`.
        let group = div()
            .flex()
            .h(px(36.0))
            .items_center()
            .rounded(px(8.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.card)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px_4()
                    .tw_text_sm()
                    .children(self.dictionary_input.clone()),
            )
            .child(
                div().flex().items_center().pr(px(6.0)).child(
                    div()
                        .id("dictionary-add")
                        .flex()
                        .h(px(24.0))
                        .items_center()
                        .gap_1()
                        .px_2()
                        .rounded(px(8.0))
                        .bg(gpui::rgb(0x000000))
                        .text_color(gpui::rgb(0xffffff))
                        .tw_text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .when(!can_add, |b| b.opacity(0.5))
                        .when(can_add && allowed, |b| b.cursor_pointer())
                        .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                            if !can_add {
                                return;
                            }
                            let value = this
                                .dictionary_input
                                .as_ref()
                                .map(|input| input.read(cx).text().to_string())
                                .unwrap_or_default();
                            this.add_dictionary_terms(&value, cx);
                        }))
                        .child(icon("plus", px(14.0), gpui::rgb(0xffffff)))
                        .child("Add"),
                ),
            );

        let body: gpui::AnyElement = if terms.is_empty() {
            // `min-h-40 rounded-2xl border bg-card` centred copy.
            div()
                .flex()
                .flex_col()
                .min_h(px(160.0))
                .items_center()
                .justify_center()
                .rounded(px(16.0))
                .border_1()
                .border_color(theme.border)
                .bg(theme.card)
                .px_6()
                .child(
                    div()
                        .mb_3()
                        .child(icon("book-open", px(20.0), theme.muted_foreground)),
                )
                .child(
                    div()
                        .tw_text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .child("Your dictionary is empty"),
                )
                .child(
                    div()
                        .mt_1()
                        .max_w(px(384.0))
                        .tw_text_xs()
                        .text_color(theme.muted_foreground)
                        .text_center()
                        .child(
                            "Tip: Add teammate names, acronyms, company jargon, and product terms.",
                        ),
                )
                .into_any_element()
        } else if visible.is_empty() {
            if has_search {
                div()
                    .px_4()
                    .tw_text_sm()
                    .text_color(theme.muted_foreground)
                    .child("No match")
                    .into_any_element()
            } else {
                div().into_any_element()
            }
        } else {
            let editing = self.dictionary_edit.as_ref().map(|edit| edit.term.clone());
            div()
                .flex()
                .flex_col()
                .rounded(px(16.0))
                .border_1()
                .border_color(theme.border)
                .bg(theme.card)
                .overflow_hidden()
                .children(visible.iter().enumerate().map(|(index, term)| {
                    let row = if editing.as_deref() == Some(term.as_str()) {
                        self.render_dictionary_edit_row(cx)
                    } else {
                        self.render_dictionary_term_row(term, cx)
                    };
                    row.when(index > 0, |row| row.border_t_1().border_color(theme.border))
                }))
                .into_any_element()
        };

        div().flex().flex_col().gap_4().child(group).child(body)
    }

    /// `DictionaryTermRow`: `min-h-12 py-3 pr-3 pl-4` with the hover-only
    /// pencil and minus buttons.
    fn render_dictionary_term_row(&self, term: &str, cx: &Context<Self>) -> gpui::Stateful<Div> {
        let theme = self.theme;
        let term_owned = term.to_string();
        let edit_term = term_owned.clone();
        let remove_term = term_owned.clone();
        let icon_button = |id: SharedString, name: &'static str, on_click: RowAction| {
            div()
                .id(id)
                .flex()
                .size(px(28.0))
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .rounded(px(8.0))
                .cursor_pointer()
                .hover(|s| s.bg(theme.accent))
                .on_click(cx.listener(move |this, _: &gpui::ClickEvent, window, cx| {
                    on_click(this, window, cx)
                }))
                .child(icon(name, px(16.0), theme.muted_foreground))
        };
        div()
            .id(SharedString::from(format!("dictionary-term-{term}")))
            .group("dictionary-row")
            .flex()
            .min_h(px(48.0))
            .items_center()
            .justify_between()
            .gap_3()
            .py_3()
            .pr_3()
            .pl_4()
            .child(
                div()
                    .tw_text_sm()
                    .text_color(theme.foreground)
                    .child(SharedString::from(term_owned)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .opacity(0.0)
                    .group_hover("dictionary-row", |s| s.opacity(1.0))
                    .child(icon_button(
                        SharedString::from(format!("dictionary-edit-{term}")),
                        "pencil-simple",
                        Box::new(move |this, window, cx| {
                            this.start_dictionary_edit(&edit_term, window, cx)
                        }),
                    ))
                    .child(icon_button(
                        SharedString::from(format!("dictionary-remove-{term}")),
                        "minus-circle",
                        Box::new(move |this, _, cx| this.remove_dictionary_term(&remove_term, cx)),
                    )),
            )
    }

    /// The row while editing: the `h-8` input with the check and X buttons.
    fn render_dictionary_edit_row(&self, cx: &Context<Self>) -> gpui::Stateful<Div> {
        let theme = self.theme;
        let Some(edit) = &self.dictionary_edit else {
            return div().id("dictionary-edit-row");
        };
        let terms = self.dictionary_terms();
        let value = edit.input.read(cx).text().to_string();
        let can_save = edited_dictionary_term(&terms, &edit.term, &value).is_some();
        let icon_button =
            |id: &'static str, name: &'static str, enabled: bool, on_click: EditAction| {
                div()
                    .id(id)
                    .flex()
                    .size(px(28.0))
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.0))
                    .when(!enabled, |b| b.opacity(0.5))
                    .when(enabled, |b| {
                        b.cursor_pointer().hover(|s| s.bg(theme.accent))
                    })
                    .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                        if enabled {
                            on_click(this, cx)
                        }
                    }))
                    .child(icon(name, px(16.0), theme.muted_foreground))
            };
        div()
            .id("dictionary-edit-row")
            .flex()
            .min_h(px(48.0))
            .items_center()
            .gap_2()
            .py_2()
            .pr_3()
            .pl_4()
            .child(
                div()
                    .flex()
                    .h(px(32.0))
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .px_3()
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.card)
                    .tw_text_sm()
                    .child(edit.input.clone()),
            )
            .child(icon_button(
                "dictionary-edit-save",
                "check",
                can_save,
                Box::new(|this, cx| this.save_dictionary_edit(cx)),
            ))
            .child(icon_button(
                "dictionary-edit-cancel",
                "x",
                true,
                Box::new(|this, cx| {
                    this.dictionary_edit = None;
                    cx.notify();
                }),
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_lists_are_trimmed_deduplicated_and_kept_in_order() {
        assert_eq!(
            normalize_keyword_list([
                "  Acme  Corp ",
                "acme corp",
                "x",
                "Kubernetes",
                "kubernetes "
            ]),
            vec!["Acme Corp", "Kubernetes"]
        );
        assert!(normalize_keyword_list(["", " ", "a"]).is_empty());
    }

    #[test]
    fn typed_terms_split_on_commas_and_newlines() {
        assert_eq!(
            parse_dictionary_terms_text("Alice, Bob\nAlice,, kubectl"),
            vec!["Alice", "Bob", "kubectl"]
        );
        assert!(parse_dictionary_terms_text(" , \n").is_empty());
    }

    #[test]
    fn stored_terms_parse_from_json_strings_and_arrays() {
        assert_eq!(
            parse_dictionary_terms_json(r#"["Alice","bob"]"#),
            vec!["Alice", "bob"]
        );
        assert_eq!(
            parse_dictionary_terms_json(r#""[\"Alice\"]""#),
            vec!["Alice"]
        );
        assert!(parse_dictionary_terms_json("null").is_empty());
        assert!(parse_dictionary_terms_json("not json").is_empty());
    }

    #[test]
    fn appending_ignores_duplicates_and_editing_rejects_collisions() {
        let terms = vec!["Alice".to_string(), "Bob".to_string()];
        assert_eq!(
            append_dictionary_terms(&terms, "alice, Carol"),
            vec!["Alice", "Bob", "Carol"]
        );
        assert_eq!(append_dictionary_terms(&terms, "ALICE"), terms);
        assert_eq!(
            edited_dictionary_term(&terms, "Alice", "  Alicia "),
            Some("Alicia".to_string())
        );
        assert_eq!(
            edited_dictionary_term(&terms, "Alice", "alice"),
            Some("alice".to_string())
        );
        assert_eq!(edited_dictionary_term(&terms, "Alice", "Alice"), None);
        assert_eq!(edited_dictionary_term(&terms, "Alice", "bob"), None);
        assert_eq!(edited_dictionary_term(&terms, "Alice", "a"), None);
    }

    #[test]
    fn visible_terms_match_the_typed_query_in_either_direction() {
        let terms = vec![
            "Kubernetes".to_string(),
            "Alice".to_string(),
            "Bob".to_string(),
        ];
        assert_eq!(visible_dictionary_terms(&terms, ""), terms);
        assert_eq!(visible_dictionary_terms(&terms, "kube"), vec!["Kubernetes"]);
        assert_eq!(
            visible_dictionary_terms(&terms, "Alice Smith"),
            vec!["Alice"]
        );
        assert_eq!(
            visible_dictionary_terms(&terms, "al, bo"),
            vec!["Alice", "Bob"]
        );
        assert!(visible_dictionary_terms(&terms, "zz").is_empty());
    }
}
