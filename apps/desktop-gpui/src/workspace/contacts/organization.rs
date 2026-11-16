//! `OrganizationDetailsColumn` from shipping's `contacts/organization-details.tsx`.

use gpui::{
    AnyElement, Context, Entity, SharedString, Subscription, Window, div, prelude::*, px, svg,
};

use super::{
    Contacts, Page, Selection, avatar,
    data::Organization,
    ops::{Kind, Write},
    text_sm, text_xs,
};
use crate::ui::{
    input::{InputEvent, TextInput},
    theme::theme,
};

pub(super) struct OrganizationPage {
    pub id: SharedString,
    name: Entity<TextInput>,
    _subscription: Subscription,
}

impl OrganizationPage {
    pub fn new(organization: &Organization, cx: &mut Context<Contacts>) -> Self {
        let name = cx.new(|cx| {
            let mut input = TextInput::new("Organization name", cx).inline();
            input.set_text(organization.name.to_string(), cx);
            input
        });
        let id = organization.id.to_string();
        let subscription = cx.subscribe(&name, move |this, input, event, cx| {
            if matches!(event, InputEvent::Changed) {
                let name = input.read(cx).buffer.text.clone();
                this.write(
                    Write::RenameOrganization {
                        id: id.clone(),
                        name,
                    },
                    cx,
                );
            }
        });
        Self {
            id: organization.id.clone(),
            name,
            _subscription: subscription,
        }
    }
}

impl Contacts {
    pub(super) fn organization(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = theme(window);
        let Page::Organization(page) = &self.page else {
            return div().into_any_element();
        };
        let organization = self
            .data
            .organization(&page.id)
            .cloned()
            .unwrap_or_default();
        let name_input = page.name.clone();
        let members = self
            .data
            .members(&organization.id)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let compact = avatar::organization(
            organization.avatar.as_ref(),
            &organization.name,
            24.,
            colors.muted,
            colors,
        );
        let large = avatar::organization(
            organization.avatar.as_ref(),
            &organization.name,
            64.,
            colors.accent,
            colors,
        );
        let title = if organization.name.is_empty() {
            SharedString::from("Unnamed")
        } else {
            organization.name.clone()
        };
        let header = self.header(
            title,
            compact,
            organization.team_workspace,
            organization.pinned,
            organization.avatar.is_some(),
            None,
            colors,
            cx,
        );
        let count = members.len();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(header)
            .child(
                div()
                    .id("contact-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .on_scroll(cx.listener(|_, _, _, cx| cx.notify()))
                    .child(
                        div()
                            .py_6()
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_b_1()
                            .border_color(colors.border)
                            .child(self.avatar_upload(
                                Kind::Organization,
                                organization.id.clone(),
                                large,
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .border_b_1()
                            .border_color(colors.border)
                            .px_4()
                            .py_3()
                            .child(
                                text_sm(div())
                                    .w(px(112.))
                                    .flex_shrink_0()
                                    .text_color(colors.muted_foreground)
                                    .child("Name"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .h(px(28.))
                                    .flex()
                                    .items_center()
                                    .child(name_input),
                            ),
                    )
                    .child(
                        div()
                            .p_6()
                            .child(
                                text_sm(div())
                                    .mb_4()
                                    .flex()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(colors.muted_foreground)
                                    .child("People")
                                    .child(
                                        div()
                                            .font_weight(gpui::FontWeight::NORMAL)
                                            .whitespace_nowrap()
                                            .child(format!(
                                                "\u{a0}· {count} {}",
                                                if count == 1 { "member" } else { "members" }
                                            )),
                                    ),
                            )
                            .map(|view| {
                                if members.is_empty() {
                                    view.child(
                                        text_sm(div())
                                            .text_color(colors.muted_foreground)
                                            .child("No people in this organization"),
                                    )
                                } else {
                                    view.child(div().grid().grid_cols(3).gap_4().children(
                                        members.iter().map(|human| {
                                            let id = human.id.clone();
                                            let email = human.email.clone();
                                            let linkedin = human.linkedin.clone();
                                            div()
                                                .id(SharedString::from(format!("member-{}", human.id)))
                                                .min_w_0()
                                                .p_4()
                                                .rounded(px(8.))
                                                .border_1()
                                                .border_color(colors.border)
                                                .bg(colors.card)
                                                .cursor_pointer()
                                                .hover(|style| style.shadow_xs())
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.select(Some(Selection::Person(id.clone())), cx)
                                                }))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .items_center()
                                                        .gap_3()
                                                        .child(avatar::person(
                                                            human.avatar.as_ref(),
                                                            &human.avatar_name(),
                                                            48.,
                                                        ))
                                                        .child(
                                                            div()
                                                                .w_full()
                                                                .flex()
                                                                .flex_col()
                                                                .items_center()
                                                                .child(
                                                                    text_sm(div())
                                                                        .max_w_full()
                                                                        .truncate()
                                                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                                                        .child(if human.name.is_empty() && human.email.is_empty() {
                                                                            SharedString::from("Unnamed")
                                                                        } else {
                                                                            human.display_name()
                                                                        }),
                                                                )
                                                                .when(!human.job_title.is_empty(), |view| {
                                                                    view.child(
                                                                        text_xs(div())
                                                                            .mt_1()
                                                                            .max_w_full()
                                                                            .truncate()
                                                                            .text_color(colors.muted_foreground)
                                                                            .child(human.job_title.clone()),
                                                                    )
                                                                }),
                                                        )
                                                        .child(
                                                            div()
                                                                .mt_1()
                                                                .flex()
                                                                .gap_2()
                                                                .when(!email.is_empty(), |view| {
                                                                    view.child(
                                                                        // A plain ghost button keeps the foreground icon colour.
                                                                        div()
                                                                            .id(SharedString::from(format!("email-{}", human.id)))
                                                                            .size(px(28.))
                                                                            .flex()
                                                                            .items_center()
                                                                            .justify_center()
                                                                            .rounded(px(8.))
                                                                            .cursor_pointer()
                                                                            .hover(move |style| style.bg(colors.accent))
                                                                            .child(
                                                                                svg()
                                                                                    .path("chat/Mail01Icon.svg")
                                                                                    .size(px(16.))
                                                                                    .text_color(colors.foreground),
                                                                            )
                                                                        .on_click(move |_, _, cx| {
                                                                            cx.stop_propagation();
                                                                            cx.open_url(&format!("mailto:{email}"));
                                                                        }),
                                                                    )
                                                                })
                                                                .when(!linkedin.is_empty(), |view| {
                                                                    view.child(
                                                                        div()
                                                                            .id(SharedString::from(format!("linkedin-{}", human.id)))
                                                                            .size(px(28.))
                                                                            .flex()
                                                                            .items_center()
                                                                            .justify_center()
                                                                            .rounded(px(8.))
                                                                            .cursor_pointer()
                                                                            .hover(move |style| style.bg(colors.accent))
                                                                            .on_click(move |_, _, cx| {
                                                                                cx.stop_propagation();
                                                                                cx.open_url(&linkedin_url(&linkedin));
                                                                            })
                                                                            .child(
                                                                                gpui::img("contacts/LinkedInIcon.svg")
                                                                                    .size(px(14.)),
                                                                            ),
                                                                    )
                                                                }),
                                                        ),
                                                )
                                        }),
                                    ))
                                }
                            }),
                    )
                    .child(div().h(px(384.))),
            )
            .into_any_element()
    }
}

fn linkedin_url(value: &str) -> String {
    if value.starts_with("http://") || value.starts_with("https://") {
        value.to_owned()
    } else {
        format!(
            "https://www.linkedin.com/in/{}",
            value.strip_prefix('@').unwrap_or(value)
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn linkedin_handles_become_profile_urls() {
        assert_eq!(
            super::linkedin_url("@ada"),
            "https://www.linkedin.com/in/ada"
        );
        assert_eq!(
            super::linkedin_url("https://www.linkedin.com/in/ada/"),
            "https://www.linkedin.com/in/ada/"
        );
    }
}
