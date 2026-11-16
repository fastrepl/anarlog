//! `settings/general/account-profile.tsx`: the personal contact card on the Account page.
//! Edits save as you type, through the same upsert shipping's `savePersonalContact` uses.

use std::{sync::Arc, time::Duration};

use desktop_runtime::{CancellationToken, RuntimeHandle, ServiceError};
use gpui::{
    Context, Entity, FontWeight, SharedString, Subscription, Task, Window, div, prelude::*, px,
};
use serde_json::{Value, json};

use super::controls;
use crate::ui::{
    input::{InputEvent, TextInput},
    theme::theme,
};

/// Shipping's `useOwnerUserId`: the local library's owner when nobody is signed in.
const OWNER_SQL: &str = "SELECT user_id FROM (
    SELECT owner_user_id AS user_id, updated_at, 0 AS source_priority FROM sessions
      WHERE owner_user_id <> '' AND deleted_at IS NULL
    UNION ALL SELECT id AS user_id, updated_at, 1 FROM humans
      WHERE id = owner_user_id AND id <> '' AND deleted_at IS NULL
    UNION ALL SELECT owner_user_id AS user_id, updated_at, 2 FROM chat_groups
      WHERE owner_user_id <> '' AND deleted_at IS NULL
  ) ORDER BY source_priority, updated_at DESC, user_id LIMIT 1";

const FIELDS: [(&str, &str, &str); 5] = [
    ("name", "Name", ""),
    ("job_title", "Job Title", ""),
    ("email", "Email", ""),
    ("phone", "Phone", "+1 202 555 0123"),
    ("linkedin_username", "LinkedIn", ""),
];

enum Load {
    Loading,
    Failed,
    Ready,
}

#[derive(PartialEq)]
enum Save {
    Idle,
    Saving,
    Saved,
    Failed,
}

pub struct ProfileCard {
    runtime: RuntimeHandle,
    human_id: Option<Arc<str>>,
    inputs: Vec<(&'static str, &'static str, Entity<TextInput>)>,
    memo: Entity<TextInput>,
    organizations: Vec<(String, String)>,
    organization: String,
    menu: bool,
    load: Load,
    save: Save,
    loading: bool,
    pending: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl ProfileCard {
    /// `account` is the signed-in user's id; signed out, the card edits the library owner.
    pub fn new(
        runtime: RuntimeHandle,
        account: Option<(Arc<str>, Option<Arc<str>>)>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subscriptions = Vec::new();
        let mut watch = |input: &Entity<TextInput>, cx: &mut Context<Self>| {
            subscriptions.push(cx.subscribe(input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Changed) && !this.loading {
                    this.schedule_save(cx);
                }
            }));
        };
        let inputs = FIELDS
            .iter()
            .map(|(key, label, placeholder)| {
                let input = cx.new(|cx| TextInput::new(*placeholder, cx).inline());
                watch(&input, cx);
                (*key, *label, input)
            })
            .collect();
        let memo = cx.new(|cx| TextInput::new("", cx).multiline().inline());
        watch(&memo, cx);
        let mut this = Self {
            runtime,
            human_id: None,
            inputs,
            memo,
            organizations: Vec::new(),
            organization: String::new(),
            menu: false,
            load: Load::Loading,
            save: Save::Idle,
            loading: true,
            pending: None,
            _subscriptions: subscriptions,
        };
        this.reload(account, cx);
        this
    }

    fn reload(&mut self, account: Option<(Arc<str>, Option<Arc<str>>)>, cx: &mut Context<Self>) {
        let reply = self
            .runtime
            .read(CancellationToken::new(), move |services| async move {
                let execute = |sql: &str, params: Vec<Value>| {
                    services.executor.execute(sql.into(), params)
                };
                let (id, email) = match account {
                    Some((id, email)) => (id.to_string(), email),
                    None => {
                        let owner = execute(OWNER_SQL, vec![]).await.map_err(failure)?;
                        let id = owner
                            .first()
                            .and_then(|row| row["user_id"].as_str())
                            .map(str::trim)
                            .filter(|id| !id.is_empty())
                            .unwrap_or(anlg_db_app::DEFAULT_USER_ID)
                            .to_owned();
                        (id, None)
                    }
                };
                let human = execute(
                    "SELECT name, email, phone, job_title, linkedin_username, memo, organization_id FROM humans WHERE id = ? AND deleted_at IS NULL",
                    vec![json!(id)],
                )
                .await
                .map_err(failure)?;
                let organizations = execute(
                    "SELECT id, name FROM organizations WHERE deleted_at IS NULL ORDER BY name, id",
                    vec![],
                )
                .await
                .map_err(failure)?;
                Ok((id, email, human.into_iter().next(), organizations))
            });
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| match result {
                Ok((id, email, human, organizations)) => {
                    this.loading = true;
                    let text = |key: &str| {
                        human
                            .as_ref()
                            .and_then(|row| row[key].as_str())
                            .unwrap_or_default()
                            .to_owned()
                    };
                    for (key, _, input) in &this.inputs {
                        let mut value = text(key);
                        if *key == "email" && human.is_none() {
                            value = email.as_deref().unwrap_or_default().to_owned();
                        }
                        input.update(cx, |input, cx| input.set_text(value, cx));
                    }
                    this.memo
                        .update(cx, |input, cx| input.set_text(text("memo"), cx));
                    this.organization = text("organization_id");
                    this.organizations = organizations
                        .iter()
                        .map(|row| {
                            let field =
                                |key: &str| row[key].as_str().unwrap_or_default().to_owned();
                            (field("id"), field("name"))
                        })
                        .collect();
                    this.human_id = Some(id.into());
                    this.load = Load::Ready;
                    this.loading = false;
                    cx.notify();
                }
                Err(_) => {
                    this.load = Load::Failed;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn value(&self, key: &str, cx: &gpui::App) -> String {
        self.inputs
            .iter()
            .find(|(name, _, _)| *name == key)
            .map(|(_, _, input)| input.read(cx).buffer.text.clone())
            .unwrap_or_default()
    }

    /// Coalesces keystrokes into one write; the latest values always win.
    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        self.save = Save::Saving;
        cx.notify();
        self.pending = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let _ = this.update(cx, |this, cx| this.save_now(cx));
        }));
    }

    fn save_now(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.human_id.clone() else {
            return;
        };
        let values = ["name", "email", "phone", "job_title", "linkedin_username"]
            .map(|key| self.value(key, cx));
        let memo = self.memo.read(cx).buffer.text.clone();
        let organization = self.organization.clone();
        let reply = self.runtime.submit(move |services| async move {
            let [name, email, phone, job_title, linkedin] = values;
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            let mut connection = services.db.pool().acquire().await.map_err(failure)?;
            anlg_db_app::upsert_personal_contact(
                &mut connection,
                &id,
                &name,
                &email,
                &phone,
                &job_title,
                &linkedin,
                &memo,
                &organization,
                &anlg_db_app::AvatarChange::Keep,
                &now,
            )
            .await
            .map_err(failure)?;
            Ok(())
        });
        self.pending = Some(cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                this.save = if result.is_ok() {
                    Save::Saved
                } else {
                    Save::Failed
                };
                cx.notify();
            });
        }));
    }
}

fn failure(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

/// `avatarInitials`: the first letter or digit of up to two words.
fn initials(label: &str) -> String {
    label
        .split_whitespace()
        .filter_map(|word| word.chars().find(|c| c.is_alphanumeric()))
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

pub type ProfileKey = Option<Arc<str>>;

pub fn key(account: &super::account::Account) -> ProfileKey {
    account.user.as_ref().map(|user| user.id.clone())
}

impl Render for ProfileCard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let muted = |text: &'static str| {
            div()
                .text_sm()
                .line_height(px(20.))
                .text_color(colors.muted_foreground)
                .child(text)
        };
        match self.load {
            Load::Loading => return div().text_sm().child("Loading...").into_any_element(),
            Load::Failed => {
                return div()
                    .text_sm()
                    .child("Couldn't load your profile. Reopen this page to try again.")
                    .into_any_element();
            }
            Load::Ready => {}
        }
        // Shipping labels the avatar with the name, else the email, else the contact id.
        let label = [self.value("name", cx), self.value("email", cx)]
            .into_iter()
            .find(|value| !value.trim().is_empty())
            .or_else(|| self.human_id.as_deref().map(str::to_owned))
            .unwrap_or_default();
        let initials = initials(&label);
        let field = |label: &'static str, control: gpui::AnyElement| {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .text_sm()
                .line_height(px(20.))
                .child(label)
                .child(control)
        };
        let input_box = |input: Entity<TextInput>, multiline: bool| {
            div()
                .w_full()
                .when(!multiline, |view| view.h(px(36.)).items_center())
                // `rows={3}`: three 20px lines inside `py-2`.
                .when(multiline, |view| view.min_h(px(78.)).py_2().items_start())
                .px_3()
                .flex()
                .rounded(px(6.))
                .border_1()
                .border_color(colors.input)
                .text_sm()
                .child(div().w_full().child(input))
                .into_any_element()
        };
        let organization = self
            .organizations
            .iter()
            .find(|(id, _)| *id == self.organization)
            .map(|(_, name)| SharedString::from(name.clone()));
        let company = div()
            .relative()
            .w(px(240.))
            .child(
                controls::select_trigger(
                    "profile-company",
                    organization,
                    "Select company",
                    true,
                    &colors,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.menu = !this.menu;
                    cx.notify();
                })),
            )
            .when(self.menu, |view| {
                view.child(controls::dropdown(
                    controls::menu_panel(240., &colors)
                        .child(
                            controls::menu_item(
                                "profile-company-none",
                                "No company",
                                None,
                                self.organization.is_empty(),
                                &colors,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.organization.clear();
                                this.menu = false;
                                this.schedule_save(cx);
                            })),
                        )
                        .children(self.organizations.iter().map(|(id, name)| {
                            let value = id.clone();
                            controls::menu_item(
                                SharedString::from(format!("profile-company-{id}")),
                                name.clone(),
                                None,
                                *id == self.organization,
                                &colors,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.organization = value.clone();
                                    this.menu = false;
                                    this.schedule_save(cx);
                                },
                            ))
                        })),
                    false,
                ))
            });
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_sm()
                    .line_height(px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .child("Your info"),
            )
            .child(muted(
                "Update your personal contact card. Your sign-in email is managed separately.",
            ))
            .child(
                div()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div().flex().items_center().gap_4().child(
                            // The generated dithered avatar is not ported; this is its
                            // initials layer on a neutral tile.
                            div()
                                .size(px(64.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(16.))
                                .border_1()
                                .border_color(gpui::black().opacity(0.1))
                                .bg(colors.muted)
                                .text_color(colors.muted_foreground)
                                .text_size(px(24.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(initials),
                        ),
                    )
                    .children(
                        self.inputs
                            .iter()
                            .map(|(_, label, input)| field(label, input_box(input.clone(), false))),
                    )
                    .child(field("Company", company.into_any_element()))
                    .child(field("Notes", input_box(self.memo.clone(), true))),
            )
            .when(self.save == Save::Failed, |view| {
                view.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .text_sm()
                                .text_color(colors.destructive)
                                .child("Couldn't save your profile. Try again."),
                        )
                        .child(controls::outline_button(
                            "profile-retry",
                            "Retry",
                            true,
                            &colors,
                            Some(Box::new(cx.listener(|this, _, _, cx| this.save_now(cx)))),
                        )),
                )
            })
            .when(matches!(self.save, Save::Saving | Save::Saved), |view| {
                view.child(muted(if self.save == Save::Saving {
                    "Saving..."
                } else {
                    "Saved"
                }))
            })
            .into_any_element()
    }
}
