//! Shipping's CRM integration: `settings/crm/index.tsx`, `crm/connection.ts` and the contact
//! enrichment in `crm/{enrichment,enrich-contact}.ts(x)`.

use std::{collections::HashMap, sync::Arc};

use desktop_runtime::{CancellationToken, Result, RuntimeHandle, ServiceError};
use gpui::{
    AnyElement, App, ClickEvent, Context, Div, Render, SharedString, Subscription, Window, div,
    img, prelude::*, px,
};
use serde_json::{Value, json};

use super::{
    account,
    controls::{self, dropdown, icon, menu_panel},
    integrations::{self, Connection, Integrations},
    local::imports_view::{
        connect_group, group_button, group_caret, hover_label, menu_entry, spinner,
    },
};
use crate::{
    application::toasts,
    ui::theme::{MEDIUM, Theme, theme},
};

#[derive(Debug, PartialEq, Eq)]
pub struct Provider {
    pub id: &'static str,
    pub name: &'static str,
    pub nango: &'static str,
}

impl Provider {
    pub fn icon(&self) -> String {
        format!("product/brands/{}.svg", self.id)
    }
}

/// `CRM_PROVIDERS`.
pub const PROVIDERS: &[Provider] = &[
    Provider {
        id: "hubspot",
        name: "HubSpot",
        nango: "hubspot",
    },
    Provider {
        id: "attio",
        name: "Attio",
        nango: "attio",
    },
    Provider {
        id: "salesforce",
        name: "Salesforce",
        nango: "salesforce",
    },
    Provider {
        id: "pipedrive",
        name: "Pipedrive",
        nango: "pipedrive",
    },
    Provider {
        id: "close",
        name: "Close",
        nango: "close",
    },
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Contact {
    pub name: Option<String>,
    pub email: Option<String>,
    pub company_name: Option<String>,
    pub job_title: Option<String>,
    pub phone: Option<String>,
    pub linkedin_url: Option<String>,
    pub url: Option<String>,
}

impl Contact {
    fn from_value(value: &Value) -> Self {
        let text = |key: &str| value[key].as_str().map(str::to_owned);
        Self {
            name: text("name"),
            email: text("email"),
            company_name: text("companyName"),
            job_title: text("jobTitle"),
            phone: text("phone"),
            linkedin_url: text("linkedinUrl"),
            url: text("url"),
        }
    }
}

/// The contact fields enrichment reads; empty strings are missing values.
#[derive(Clone, Debug, Default)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub email: String,
    pub organization_id: String,
    pub job_title: String,
    pub phone: String,
    pub linkedin: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub name: Option<String>,
    pub email: Option<String>,
    pub company_name: Option<String>,
    pub job_title: Option<String>,
    pub phone: Option<String>,
    pub linkedin_username: Option<String>,
}

impl Changes {
    /// The filled fields' labels, in shipping's order.
    pub fn labels(&self) -> Vec<&'static str> {
        [
            (self.name.is_some(), "name"),
            (self.email.is_some(), "email"),
            (self.company_name.is_some(), "company"),
            (self.job_title.is_some(), "job title"),
            (self.phone.is_some(), "phone"),
            (self.linkedin_username.is_some(), "LinkedIn"),
        ]
        .into_iter()
        .filter_map(|(filled, label)| filled.then_some(label))
        .collect()
    }
}

fn clean(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// `linkedinUsernameFromUrl`.
pub fn linkedin_username(url: Option<&str>) -> Option<String> {
    static PROFILE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?i)linkedin\.com/in/([^/?#]+)").expect("valid LinkedIn pattern")
    });
    let value = clean(url)?;
    if let Some(name) = PROFILE
        .captures(&value)
        .and_then(|captures| captures.get(1))
    {
        return Some(decode(name.as_str()));
    }
    (!value.contains('/') && !value.contains('.')).then_some(value)
}

fn decode(value: &str) -> String {
    url::form_urlencoded::parse(format!("v={value}").as_bytes())
        .next()
        .map(|(_, decoded)| decoded.into_owned())
        .unwrap_or_else(|| value.to_owned())
}

/// `planCrmEnrichment`: only fills fields that are empty; never overwrites local edits.
pub fn plan(person: &Person, contact: &Contact) -> Changes {
    let empty = |value: &str| value.trim().is_empty();
    let fill = |local: &str, remote: Option<&str>| {
        if empty(local) { clean(remote) } else { None }
    };
    Changes {
        name: fill(&person.name, contact.name.as_deref()),
        email: fill(&person.email, contact.email.as_deref()),
        company_name: fill(&person.organization_id, contact.company_name.as_deref()),
        job_title: fill(&person.job_title, contact.job_title.as_deref()),
        phone: fill(&person.phone, contact.phone.as_deref()),
        linkedin_username: if empty(&person.linkedin) {
            linkedin_username(contact.linkedin_url.as_deref())
        } else {
            None
        },
    }
}

/// `pickBestCrmContact`: the record that fills the most empty fields; ties keep CRM order.
pub fn pick_best(person: &Person, contacts: Vec<Contact>) -> Option<(Contact, Changes)> {
    let mut best: Option<(Contact, Changes)> = None;
    for contact in contacts {
        let changes = plan(person, &contact);
        if best
            .as_ref()
            .is_none_or(|(_, current)| changes.labels().len() > current.labels().len())
        {
            best = Some((contact, changes));
        }
    }
    best
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    NotConnected,
    NoMatch(Vec<&'static str>),
    Matched {
        provider: &'static str,
        contact: Box<Contact>,
        changes: Changes,
    },
}

/// `connectedCrmProviders`.
pub fn connected(connections: &[Connection]) -> Vec<(&'static Provider, String)> {
    PROVIDERS
        .iter()
        .filter_map(|provider| {
            integrations::find(connections, provider.nango)
                .filter(|connection| connection.ready())
                .map(|connection| (provider, connection.connection_id.clone()))
        })
        .collect()
}

/// `lookupCrmContacts`.
pub async fn search(
    service: &dyn Integrations,
    provider: &Provider,
    connection: String,
    email: Option<String>,
    name: Option<String>,
) -> Result<Vec<Contact>> {
    let response = service
        .post(
            "/crm/search-contacts".into(),
            json!({"provider": provider.id, "connection_id": connection, "email": email, "name": name}),
        )
        .await
        .map_err(|_| integrations::failure(format!("Failed to look up contacts in {}", provider.name)))?;
    Ok(response["contacts"]
        .as_array()
        .map(|contacts| contacts.iter().map(Contact::from_value).collect())
        .unwrap_or_default())
}

/// `enrichHumanFromCrm`: the first connected CRM with a matching record fills the person's
/// empty fields.
pub async fn enrich(
    service: &dyn Integrations,
    runtime: &RuntimeHandle,
    person: Person,
    owner: String,
) -> Result<Outcome> {
    let connections = service.connections().await?;
    let connected = connected(&connections);
    if connected.is_empty() {
        return Ok(Outcome::NotConnected);
    }
    let names = connected
        .iter()
        .map(|(provider, _)| provider.name)
        .collect();
    let email = clean(Some(&person.email));
    let name = clean(Some(&person.name));
    if email.is_none() && name.is_none() {
        return Ok(Outcome::NoMatch(names));
    }
    let mut first_error = None;
    for (provider, connection) in connected {
        let contacts =
            match search(service, provider, connection, email.clone(), name.clone()).await {
                Ok(contacts) => contacts,
                Err(error) => {
                    first_error.get_or_insert(error);
                    continue;
                }
            };
        let Some((contact, changes)) = pick_best(&person, contacts) else {
            continue;
        };
        if changes != Changes::default() {
            apply(runtime, person.id.clone(), owner, changes.clone()).await?;
        }
        return Ok(Outcome::Matched {
            provider: provider.name,
            contact: Box::new(contact),
            changes,
        });
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(Outcome::NoMatch(names)),
    }
}

/// `applyContactEnhancement`: creates the company if needed and links it only when the person
/// has none.
pub async fn apply(
    runtime: &RuntimeHandle,
    human: String,
    owner: String,
    changes: Changes,
) -> Result<()> {
    runtime
        .submit(move |services| async move {
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            let statement = |sql: String, params: Vec<Value>| anlg_db_execute::TransactionStatement {
                sql,
                params,
                expected_rows_affected: None,
            };
            let mut statements = Vec::new();
            let company = changes.company_name.clone().unwrap_or_default();
            if !company.is_empty() {
                statements.push(statement(
                    "INSERT INTO organizations (id, workspace_id, owner_user_id, name, memo, pinned, pin_order, metadata_json, created_at, updated_at, deleted_at) SELECT ?, NULLIF((SELECT json_extract(value_json, '$.workspace_id') FROM app_settings WHERE id = 'cloudsync_workspace_binding'), ''), COALESCE((SELECT library_workspace_id FROM local_library_connections WHERE active = 1), ?), ?, '', 0, NULL, '{}', ?, ?, NULL WHERE NOT EXISTS (SELECT 1 FROM organizations WHERE lower(name) = lower(?) AND deleted_at IS NULL)".into(),
                    vec![json!(uuid::Uuid::new_v4().to_string()), json!(owner), json!(company), json!(now), json!(now), json!(company)],
                ));
            }
            let mut assignments = Vec::new();
            let mut params = Vec::new();
            for (column, value) in [
                ("name", &changes.name),
                ("email", &changes.email),
                ("job_title", &changes.job_title),
                ("phone", &changes.phone),
                ("linkedin_username", &changes.linkedin_username),
            ] {
                // The plan filled empty fields; one edited while the lookup ran keeps the edit.
                if let Some(value) = value {
                    assignments.push(format!(
                        "{column} = CASE WHEN trim({column}) = '' THEN ? ELSE {column} END"
                    ));
                    params.push(json!(value));
                }
            }
            if !company.is_empty() {
                assignments.push("organization_id = CASE WHEN organization_id = '' THEN COALESCE((SELECT id FROM organizations WHERE lower(name) = lower(?) AND deleted_at IS NULL ORDER BY created_at, id LIMIT 1), organization_id) ELSE organization_id END".into());
                params.push(json!(company));
            }
            if !assignments.is_empty() {
                params.extend([json!(now), json!(human)]);
                statements.push(statement(
                    format!(
                        "UPDATE humans SET {}, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
                        assignments.join(", ")
                    ),
                    params,
                ));
            }
            if statements.is_empty() {
                return Ok(());
            }
            services
                .executor
                .execute_transaction(statements)
                .await
                .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
            Ok(())
        })?
        .receive()
        .await
}

/// `EnrichContactStatus`'s message for an outcome, and whether it links to CRM settings.
pub fn outcome_text(outcome: &Outcome) -> (String, Option<&'static str>) {
    match outcome {
        Outcome::NotConnected => ("No CRM connected.".into(), Some("Connect one in Settings")),
        Outcome::NoMatch(providers) => (
            format!("No matching contact found in {}.", providers.join(", ")),
            None,
        ),
        Outcome::Matched {
            provider, changes, ..
        } => {
            let labels = changes.labels();
            if labels.is_empty() {
                (format!("Matched in {provider}; nothing new to add."), None)
            } else {
                (
                    format!("Filled {} from {provider}.", labels.join(", ")),
                    None,
                )
            }
        }
    }
}

/// `useCrmEnrichment` for the contact page.
#[derive(Default)]
pub struct Enrichment {
    /// Whether a CRM is connected; `None` while the connections load.
    pub connected: Option<bool>,
    /// The contact being enriched.
    pub pending: Option<String>,
    /// The last enrichment's contact and outcome.
    pub result: Option<(String, std::result::Result<Outcome, String>)>,
}

#[derive(Default)]
struct Row {
    connecting: Option<CancellationToken>,
    verifying: bool,
    verified: bool,
    disconnecting: bool,
    error: Option<String>,
}

/// Settings › CRM.
pub struct CrmView {
    signed_in: bool,
    connections: Option<std::result::Result<Vec<Connection>, String>>,
    rows: HashMap<&'static str, Row>,
    menu: Option<&'static str>,
    _subscription: Subscription,
}

impl CrmView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            signed_in: account::current(cx).signed_in(),
            connections: None,
            rows: HashMap::new(),
            menu: None,
            _subscription: cx.observe_global::<account::Account>(|this, cx| {
                let signed_in = account::current(cx).signed_in();
                if signed_in != this.signed_in {
                    this.signed_in = signed_in;
                    this.load(cx);
                }
            }),
        };
        this.load(cx);
        this
    }

    fn service(&self, cx: &App) -> Option<Arc<dyn Integrations>> {
        integrations::installed(cx).filter(|_| self.signed_in)
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        self.connections = None;
        let Some(service) = self.service(cx) else {
            cx.notify();
            return;
        };
        let request = service.connections();
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.connections = Some(result.map_err(|error| error.to_string()));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn connection(&self, provider: &Provider) -> Option<Connection> {
        let connections = self.connections.as_ref()?.as_ref().ok()?;
        integrations::find(connections, provider.nango).cloned()
    }

    fn connect(&mut self, provider: &'static Provider, cx: &mut Context<Self>) {
        let row = self.rows.entry(provider.id).or_default();
        if let Some(cancel) = row.connecting.take() {
            cancel.cancel();
            cx.notify();
            return;
        }
        let Some(service) = self.service(cx) else {
            return;
        };
        let reconnect = self
            .connection(provider)
            .filter(|connection| !connection.ready())
            .map(|connection| connection.connection_id);
        let cancel = CancellationToken::new();
        let row = self.rows.entry(provider.id).or_default();
        row.connecting = Some(cancel.clone());
        row.error = None;
        cx.spawn(async move |this, cx| {
            let action = if reconnect.is_some() {
                "reconnect"
            } else {
                "connect"
            };
            let result = match service.open(provider.nango, action, reconnect, "crm").await {
                Ok(()) => {
                    integrations::wait_for_connection(
                        service.clone(),
                        provider.name,
                        provider.nango,
                        cancel,
                        cx,
                    )
                    .await
                }
                Err(_) => {
                    cx.update(|cx| toasts::error(integrations::SETUP_ERROR, cx));
                    Err(integrations::failure(integrations::SETUP_ERROR))
                }
            };
            let _ = this.update(cx, |this, cx| {
                let row = this.rows.entry(provider.id).or_default();
                row.connecting = None;
                match result {
                    Ok(_) => this.load(cx),
                    Err(ServiceError::Cancelled) => {}
                    Err(error) => row.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// `verifyCrmConnection`: a test search.
    fn verify(&mut self, provider: &'static Provider, cx: &mut Context<Self>) {
        self.menu = None;
        let (Some(service), Some(connection)) = (self.service(cx), self.connection(provider))
        else {
            return;
        };
        let row = self.rows.entry(provider.id).or_default();
        row.verifying = true;
        row.verified = false;
        row.error = None;
        cx.spawn(async move |this, cx| {
            let result = search(
                service.as_ref(),
                provider,
                connection.connection_id,
                None,
                Some("test".into()),
            )
            .await;
            let _ = this.update(cx, |this, cx| {
                let row = this.rows.entry(provider.id).or_default();
                row.verifying = false;
                match result {
                    Ok(_) => row.verified = true,
                    Err(error) => row.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn disconnect(&mut self, provider: &'static Provider, cx: &mut Context<Self>) {
        self.menu = None;
        let (Some(service), Some(connection)) = (self.service(cx), self.connection(provider))
        else {
            return;
        };
        let row = self.rows.entry(provider.id).or_default();
        row.disconnecting = true;
        row.error = None;
        cx.spawn(async move |this, cx| {
            let result = service
                .open(
                    provider.nango,
                    "disconnect",
                    Some(connection.connection_id),
                    "crm",
                )
                .await;
            let _ = this.update(cx, |this, cx| {
                let row = this.rows.entry(provider.id).or_default();
                row.disconnecting = false;
                if result.is_ok() {
                    row.verified = false;
                    this.load(cx);
                } else {
                    toasts::error(integrations::SETUP_ERROR, cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn control(
        &self,
        provider: &'static Provider,
        colors: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = provider.id;
        let row = self.rows.get(id);
        let connection = self.connection(provider);
        let connected = connection.as_ref().is_some_and(Connection::ready);
        let reconnect = connection.is_some() && !connected;
        let pending = self.connections.is_none();
        let fg = colors.primary_foreground;
        if connected {
            let verifying = row.is_some_and(|row| row.verifying);
            let enabled = !verifying && !row.is_some_and(|row| row.disconnecting);
            let group = SharedString::from(format!("crm-test-{id}"));
            let open = self.menu == Some(id);
            return div()
                .relative()
                .flex_none()
                .child(
                    connect_group(format!("crm-{id}"), true, verifying, colors)
                        .child(
                            group_button(group.clone(), true, enabled, colors)
                                .group(group.clone())
                                .flex_1()
                                .min_w_0()
                                .when(enabled, |button| {
                                    button.on_click(cx.listener(
                                        move |this, _: &ClickEvent, _, cx| {
                                            this.verify(provider, cx)
                                        },
                                    ))
                                })
                                .child(if verifying {
                                    spinner(SharedString::from(format!("crm-spin-{id}")), 14., fg)
                                } else {
                                    icon("product/RefreshIcon.svg", 14., fg).into_any_element()
                                })
                                .child(hover_label(group, "Connected", "Test")),
                        )
                        .child(
                            group_caret(
                                SharedString::from(format!("crm-more-{id}")),
                                true,
                                enabled,
                                colors,
                            )
                            .when(enabled, |caret| {
                                caret.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.menu = (this.menu != Some(id)).then_some(id);
                                    cx.notify();
                                }))
                            }),
                        ),
                )
                .child(dropdown(
                    SharedString::from(format!("crm-menu-{id}")),
                    open.then(|| {
                        menu_panel(colors)
                            .w(px(160.))
                            .child(
                                menu_entry(
                                    SharedString::from(format!("crm-menu-test-{id}")),
                                    "Test",
                                    Some("product/RefreshIcon.svg"),
                                    enabled,
                                    colors,
                                )
                                .on_click(cx.listener(
                                    move |this, _: &ClickEvent, _, cx| this.verify(provider, cx),
                                )),
                            )
                            .child(
                                menu_entry(
                                    SharedString::from(format!("crm-menu-disconnect-{id}")),
                                    "Disconnect",
                                    None,
                                    enabled,
                                    colors,
                                )
                                .on_click(cx.listener(
                                    move |this, _: &ClickEvent, _, cx| {
                                        this.disconnect(provider, cx)
                                    },
                                )),
                            )
                    }),
                    true,
                    |this: &mut Self| this.menu = None,
                    window,
                    cx,
                ))
                .into_any_element();
        }
        let connecting = row.is_some_and(|row| row.connecting.is_some());
        let enabled = connecting || !pending;
        connect_group(format!("crm-{id}"), true, false, colors)
            .child(
                group_button(
                    SharedString::from(format!("crm-connect-{id}")),
                    true,
                    enabled,
                    colors,
                )
                .flex_1()
                .min_w_0()
                .when(enabled, |button| {
                    button.on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| this.connect(provider, cx)),
                    )
                })
                .child(if connecting {
                    spinner(SharedString::from(format!("crm-spin-{id}")), 14., fg)
                } else {
                    icon("product/ElectricPlugsIcon.svg", 14., fg).into_any_element()
                })
                .child(if connecting {
                    "Cancel"
                } else if reconnect {
                    "Reconnect"
                } else {
                    "Connect"
                }),
            )
            .into_any_element()
    }

    fn provider_row(
        &self,
        provider: &'static Provider,
        first: bool,
        colors: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let row = self.rows.get(provider.id);
        let connection = self.connection(provider);
        let status = if self.connections.is_none() {
            "Checking connection…"
        } else if connection.as_ref().is_some_and(|c| !c.ready()) {
            "Reconnect required"
        } else if connection.is_some() {
            if row.is_some_and(|row| row.verified) {
                "Connected and working"
            } else {
                "Connected"
            }
        } else {
            "Not connected"
        };
        div()
            .when(!first, |view| view.border_t_1().border_color(colors.border))
            .child(
                div()
                    .min_h(px(64.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .child(
                        div()
                            .size(px(32.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(img(SharedString::from(provider.icon())).size(px(32.))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .truncate()
                                    .text_sm()
                                    .line_height(px(20.))
                                    .font_weight(MEDIUM)
                                    .child(provider.name),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .line_height(px(16.))
                                    .text_color(colors.muted_foreground)
                                    .child(status),
                            ),
                    )
                    .child(self.control(provider, colors, window, cx)),
            )
            .when_some(row.and_then(|row| row.error.clone()), |view, error| {
                view.child(
                    div()
                        .px_4()
                        .pb_3()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(colors.destructive)
                        .child(error),
                )
            })
    }
}

impl Render for CrmView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let muted = |text: &'static str| {
            div()
                .text_sm()
                .line_height(px(20.))
                .text_color(colors.muted_foreground)
                .child(text)
        };
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(controls::page_title("CRM"))
            .child(muted("Connect a CRM to look up your contacts from Anarlog. Connections are managed through your Anarlog account."))
            .map(|view| {
                if !self.signed_in {
                    return view.child(muted("Sign in to connect a CRM."));
                }
                view.child(
                    div()
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.card)
                        .overflow_hidden()
                        .children(PROVIDERS.iter().enumerate().map(|(index, provider)| {
                            self.provider_row(provider, index == 0, &colors, window, cx)
                        })),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use desktop_runtime::Profile;

    use super::*;
    use crate::product::integrations::fake::FakeIntegrations;

    #[test]
    fn best_crm_match_fills_only_empty_fields() {
        let person = Person {
            id: "h".into(),
            name: "Ada".into(),
            job_title: "CTO".into(),
            ..Default::default()
        };
        let sparse = Contact {
            email: Some("ada@acme.dev".into()),
            ..Default::default()
        };
        let rich = Contact {
            name: Some("Ada Lovelace".into()),
            job_title: Some("Engineer".into()),
            email: Some("ada@acme.dev".into()),
            company_name: Some("Acme".into()),
            linkedin_url: Some("https://www.linkedin.com/in/ada%2Dl/?trk=x".into()),
            ..Default::default()
        };
        let (contact, changes) = pick_best(&person, vec![sparse, rich.clone()]).unwrap();
        // Case folding can change byte lengths; the username must still come from the URL.
        assert_eq!(
            linkedin_username(Some("https://İ.example/LinkedIn.com/in/é")),
            Some("é".into())
        );
        assert_eq!(contact, rich);
        assert_eq!(
            changes,
            Changes {
                email: Some("ada@acme.dev".into()),
                company_name: Some("Acme".into()),
                linkedin_username: Some("ada-l".into()),
                ..Default::default()
            }
        );
    }

    #[test]
    fn enrichment_keeps_fields_edited_while_its_lookup_was_pending() {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        futures::executor::block_on(async {
            ready.receive().await.unwrap();
            let sql = |sql: &'static str| {
                runtime
                    .submit(move |services| async move {
                        services
                            .executor
                            .execute(sql.into(), vec![])
                            .await
                            .map_err(|e| ServiceError::Failed(e.to_string().into()))
                    })
                    .unwrap()
                    .receive()
            };
            sql("INSERT INTO humans (id, name, job_title) VALUES ('h', 'Ada', 'Founder')")
                .await
                .unwrap();
            // Planned against a snapshot taken before the job title was typed.
            let changes = Changes {
                job_title: Some("CTO".into()),
                phone: Some("+1 555".into()),
                ..Default::default()
            };
            apply(&runtime, "h".into(), "owner".into(), changes)
                .await
                .unwrap();
            let row = sql("SELECT job_title, phone FROM humans WHERE id = 'h'")
                .await
                .unwrap();
            assert_eq!(
                (&row[0]["job_title"], &row[0]["phone"]),
                (&json!("Founder"), &json!("+1 555"))
            );
            runtime.shutdown().await.unwrap();
        });
    }

    #[test]
    fn enrich_writes_the_planned_fields_and_links_the_company() {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        futures::executor::block_on(async {
            ready.receive().await.unwrap();
            runtime
                .submit(|services| async move {
                    services
                        .executor
                        .execute(
                            "INSERT INTO humans (id, name, email) VALUES ('h', 'Ada', 'ada@acme.dev')"
                                .into(),
                            vec![],
                        )
                        .await
                        .map_err(|e| ServiceError::Failed(e.to_string().into()))
                })
                .unwrap()
                .receive()
                .await
                .unwrap();
            let fake = FakeIntegrations::default();
            *fake.connections.lock().unwrap() =
                vec![FakeIntegrations::connection("attio", "a1", None)];
            fake.responses.lock().unwrap().push((
                "/crm/search-contacts".into(),
                Ok(json!({"contacts": [{"name": "Ada", "companyName": "Acme", "jobTitle": "CTO"}]})),
            ));
            let person = Person {
                id: "h".into(),
                name: "Ada".into(),
                email: "ada@acme.dev".into(),
                ..Default::default()
            };
            let outcome = enrich(&fake, &runtime, person, "owner".into())
                .await
                .unwrap();
            assert!(matches!(
                outcome,
                Outcome::Matched {
                    provider: "Attio",
                    ..
                }
            ));
            assert_eq!(
                fake.posts.lock().unwrap()[0].1,
                json!({"provider": "attio", "connection_id": "a1", "email": "ada@acme.dev", "name": "Ada"})
            );
            let row = runtime
                .read(CancellationToken::new(), |services| async move {
                    services
                        .executor
                        .execute(
                            "SELECT h.job_title, o.name AS company FROM humans h JOIN organizations o ON o.id = h.organization_id WHERE h.id = 'h'".into(),
                            vec![],
                        )
                        .await
                        .map_err(|e| ServiceError::Failed(e.to_string().into()))
                })
                .unwrap()
                .receive()
                .await
                .unwrap();
            assert_eq!(row[0]["job_title"], "CTO");
            assert_eq!(row[0]["company"], "Acme");
            runtime.shutdown().await.unwrap();
        });
    }
}
