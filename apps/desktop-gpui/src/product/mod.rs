pub mod account;
mod account_view;
pub mod assets;
pub mod cloud;
mod controls;
pub mod local;
mod locale;
pub(crate) mod navigation;
pub mod onboarding;
mod pages;
mod preferences_view;
mod profile;
pub(crate) mod provider_catalog;
mod providers;
mod service_view;
pub mod services;
pub mod settings;
mod surface_view;

use std::sync::Arc;

use gpui::{
    Context, Div, Entity, EventEmitter, Focusable, FontWeight, KeyDownEvent, Render, SharedString,
    Subscription, Window, WindowAppearance, div, prelude::*, px,
};

use crate::{
    contracts::{LaneContext, ProductEvent, ProductRoute},
    ui::{
        input::{InputEvent, TextInput},
        theme::{Theme, system_font, theme},
    },
};

use onboarding::OnboardingView;
use preferences_view::PreferencesView;
use services::{ProductServices, Scope, Surface, UnavailableServices};
use surface_view::SurfaceView;

#[derive(Clone, Debug)]
pub struct OpenWorkspaceSection(pub &'static str);

pub struct ProductPane {
    pub context: LaneContext,
    pub route: ProductRoute,
    services: Arc<dyn ProductServices>,
    cloud: Option<cloud::CloudServices>,
    scope: Scope,
    search: Entity<TextInput>,
    preferences: Option<Entity<PreferencesView>>,
    providers: Option<Entity<providers::ProviderView>>,
    service: Option<Entity<SurfaceView>>,
    active: navigation::Page,
    account_ui: account_view::AccountUi,
    profile: Option<(profile::ProfileKey, Entity<profile::ProfileCard>)>,
    workspace_name: Entity<TextInput>,
    onboarding: Option<Entity<OnboardingView>>,
    notice: String,
    _search_subscription: Subscription,
    service_subscription: Option<Subscription>,
    settings_subscription: Option<Subscription>,
    scope_subscription: Option<Subscription>,
    _account_subscriptions: [Subscription; 2],
}

impl ProductPane {
    pub fn new(
        context: LaneContext,
        route: ProductRoute,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::with_services(
            context,
            route,
            Arc::new(UnavailableServices),
            None,
            Scope::default(),
            window,
            cx,
        )
    }

    pub fn with_services(
        context: LaneContext,
        route: ProductRoute,
        services: Arc<dyn ProductServices>,
        cloud: Option<cloud::CloudServices>,
        scope: Scope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| TextInput::new("Search settings...", cx).inline());
        let subscription = cx.subscribe(&search, |_, _, event, cx| {
            if matches!(event, InputEvent::Changed | InputEvent::Rejected) {
                cx.notify();
            }
        });
        let workspace_name = cx.new(|cx| TextInput::new("Acme", cx));
        let account_subscriptions = [
            cx.observe_global::<account::Account>(|_, cx| cx.notify()),
            cx.subscribe(&workspace_name, |_, _, _: &InputEvent, cx| cx.notify()),
        ];
        let providers = cx
            .try_global::<crate::meeting::config::ProviderServices>()
            .cloned()
            .map(|services| cx.new(|cx| providers::ProviderView::new(services, cx)));
        let mut this = Self {
            context,
            route: route.clone(),
            services,
            cloud,
            scope,
            search,
            preferences: None,
            providers,
            service: None,
            active: navigation::page("app"),
            account_ui: Default::default(),
            profile: None,
            workspace_name,
            onboarding: None,
            notice: String::new(),
            _search_subscription: subscription,
            service_subscription: None,
            settings_subscription: None,
            scope_subscription: None,
            _account_subscriptions: account_subscriptions,
        };
        this.open_route(route, window, cx);
        this
    }

    pub fn set_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        if self.scope != scope {
            self.preferences = None;
            self.settings_subscription = None;
            self.scope = scope.clone();
            if let Some(service) = &self.service {
                service.update(cx, |service, cx| service.change_scope(scope, cx));
            } else {
                self.show_surface(None, cx);
            }
            cx.notify();
        }
    }

    pub fn can_close(&mut self, cx: &mut Context<Self>) -> bool {
        if self
            .providers
            .as_ref()
            .is_some_and(|view| view.read(cx).has_unsaved(cx))
            || self
                .preferences
                .as_ref()
                .is_some_and(|view| view.read(cx).has_unsaved(cx))
            || self
                .service
                .as_ref()
                .is_some_and(|view| view.read(cx).has_unsaved(cx))
        {
            self.notice =
                "Save or restore your edits and wait for pending work before leaving.".into();
            cx.notify();
            false
        } else {
            true
        }
    }

    pub fn open_route(&mut self, route: ProductRoute, window: &mut Window, cx: &mut Context<Self>) {
        self.route = route.clone();
        self.scope.session_id = match &route {
            ProductRoute::Share(id) | ProductRoute::Export(id) => Some(id.clone()),
            _ => None,
        };
        self.onboarding = None;
        let (page, surface) = match route {
            ProductRoute::Settings => ("app", None),
            ProductRoute::Transcription => ("transcription", None),
            ProductRoute::Intelligence => ("intelligence", None),
            ProductRoute::Account => ("account", None),
            ProductRoute::Billing => ("billing", None),
            ProductRoute::CloudSync => ("sync", Some(Surface::CloudSync)),
            ProductRoute::Permissions => ("permissions", Some(Surface::Permissions)),
            ProductRoute::Share(_) => ("account", Some(Surface::Sharing)),
            ProductRoute::Export(_) => ("app", Some(Surface::Exports)),
            ProductRoute::Import => ("imports", Some(Surface::Imports)),
            ProductRoute::Models => ("transcription", Some(Surface::Models)),
            ProductRoute::Integrations => ("developers", Some(Surface::Developers)),
            ProductRoute::Calendar => ("calendar", Some(Surface::Calendar)),
            ProductRoute::Onboarding => {
                let view = cx.new(|cx| {
                    OnboardingView::new(
                        self.services.clone(),
                        self.cloud.clone(),
                        self.context.runtime.clone(),
                        window,
                        cx,
                    )
                });
                self.service_subscription =
                    Some(cx.subscribe(&view, |_, _, event: &ProductEvent, cx| {
                        cx.emit(event.clone())
                    }));
                self.onboarding = Some(view);
                cx.notify();
                return;
            }
        };
        self.active = navigation::page(page);
        self.show_surface(surface, cx);
    }

    pub fn navigate_section(&mut self, section: &str, cx: &mut Context<Self>) {
        self.select_page(navigation::page(section), cx);
    }

    pub fn active_section(&self) -> &'static str {
        self.active.id
    }

    fn show_surface(&mut self, surface: Option<Surface>, cx: &mut Context<Self>) {
        if let Some(previous) = self.service.take() {
            previous.update(cx, |view, cx| view.suspend(cx));
        }
        self.service_subscription = None;
        self.scope_subscription = None;
        if let Some(surface) = surface {
            if let Some(preferences) = &self.preferences {
                preferences.update(cx, |view, _| view.suspend());
            }
            let view = cx.new(|cx| {
                SurfaceView::new(
                    self.services.clone(),
                    self.cloud.clone(),
                    surface,
                    self.scope.clone(),
                    cx,
                )
            });
            self.service_subscription =
                Some(cx.subscribe(&view, |_, _, event: &ProductEvent, cx| {
                    cx.emit(event.clone())
                }));
            self.scope_subscription = Some(cx.subscribe(&view, |this, _, scope: &Scope, cx| {
                this.scope = scope.clone();
                this.preferences = None;
                this.settings_subscription = None;
                cx.emit(scope.clone());
                cx.notify();
            }));
            self.service = Some(view);
        } else {
            if let Some(providers) = &self.providers {
                let kind = match self.active.id {
                    "transcription" => Some(crate::meeting::config::ProviderKind::Stt),
                    "intelligence" => Some(crate::meeting::config::ProviderKind::Llm),
                    _ => None,
                };
                if let Some(kind) = kind {
                    providers.update(cx, |view, cx| view.select_kind(kind, cx));
                }
            }
            if self.preferences.is_none() {
                let view = cx.new(|cx| PreferencesView::new(self.context.runtime.clone(), cx));
                self.settings_subscription = Some(cx.subscribe(&view, |_, _, error, cx| {
                    cx.emit(ProductEvent::Failed(error.clone()))
                }));
                self.preferences = Some(view);
            }
            if let Some(view) = &self.preferences {
                view.update(cx, |view, cx| view.select(self.active.id, cx));
            }
        }
        self.notice.clear();
        cx.notify();
    }

    fn select_page(&mut self, page: navigation::Page, cx: &mut Context<Self>) {
        if self
            .providers
            .as_ref()
            .is_some_and(|view| view.read(cx).has_unsaved(cx))
            && page != self.active
        {
            self.notice = "Save or restore provider edits before changing pages.".into();
            cx.notify();
            return;
        }
        if self
            .service
            .as_ref()
            .is_some_and(|view| view.read(cx).has_unsaved(cx))
        {
            self.notice =
                "Finish the current operation or restore form edits before changing pages.".into();
            cx.notify();
            return;
        }
        let surface = match page.id {
            "team" => Some(Surface::Teams),
            "sync" => Some(Surface::CloudSync),
            "imports" => Some(Surface::Imports),
            "permissions" => Some(Surface::Permissions),
            "developers" => Some(Surface::Developers),
            _ if navigation::opens_workspace(page) => {
                cx.emit(OpenWorkspaceSection(page.id));
                cx.notify();
                return;
            }
            _ => None,
        };
        self.active = page;
        self.show_surface(surface, cx);
    }
}

impl EventEmitter<ProductEvent> for ProductPane {}
impl EventEmitter<Scope> for ProductPane {}
impl EventEmitter<OpenWorkspaceSection> for ProductPane {}

impl ProductPane {
    /// `sidebar/settings.tsx`: back arrow, search field, then the grouped pages.
    fn sidebar(&self, colors: &Theme, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let query = self.search.read(cx).buffer.text.clone();
        let searching = !query.is_empty();
        let account = account::current(cx);
        let focused = self.search.focus_handle(cx).is_focused(window);
        let mut groups: Vec<(&'static str, Vec<navigation::Page>)> = Vec::new();
        for page in navigation::PAGES
            .iter()
            .copied()
            .filter(|page| navigation::matches(*page, &query))
        {
            match groups.last_mut() {
                Some((group, pages)) if *group == page.group => pages.push(page),
                _ => groups.push((page.group, vec![page])),
            }
        }
        let muted = colors.muted_foreground;
        let hover = colors.sidebar_accent.opacity(0.5);
        let header = div()
            .h(px(48.))
            .flex_shrink_0()
            .pt(px(9.))
            // The macOS traffic lights overlay the sidebar; keep the shipping app's gutter.
            .pl(px(if cfg!(target_os = "macos") { 76. } else { 8. }))
            .pr_1()
            .flex()
            .items_start()
            .child(
                div()
                    .id("settings-back")
                    .group("settings-back")
                    .size(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .hover(|style| style.bg(colors.accent))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.can_close(cx) {
                            cx.emit(ProductEvent::NavigateWorkspace);
                        }
                    }))
                    .child(
                        controls::icon("workspace/ArrowLeft02Icon.svg", 16., muted)
                            .group_hover("settings-back", |style| {
                                style.text_color(colors.foreground)
                            }),
                    ),
            );
        let search = div().pb_2().flex_shrink_0().child(
            div()
                .h(px(32.))
                .w_full()
                .px_3()
                .flex()
                .items_center()
                .gap_2()
                .rounded(px(8.))
                .border_1()
                .border_color(colors.border)
                .bg(if focused {
                    colors.accent
                } else {
                    colors.accent.opacity(0.5)
                })
                .text_sm()
                .child(controls::icon("Search01Icon.svg", 16., muted))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h(px(20.))
                        .line_height(px(20.))
                        .overflow_hidden()
                        .child(self.search.clone()),
                )
                .when(searching, |view| {
                    view.child(
                        div()
                            .id("clear-settings-search")
                            .group("clear-settings-search")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.search
                                    .update(cx, |search, cx| search.set_text(String::new(), cx));
                                cx.notify();
                            }))
                            .child(
                                controls::icon("product/XIcon.svg", 16., muted)
                                    .group_hover("clear-settings-search", |style| {
                                        style.text_color(colors.foreground)
                                    }),
                            ),
                    )
                }),
        );
        let list = div()
            .flex()
            .flex_col()
            .gap_5()
            .pb_6()
            .when(groups.is_empty(), |view| {
                view.child(
                    div()
                        .px_3()
                        .py_8()
                        .flex()
                        .flex_col()
                        .items_center()
                        .text_color(muted)
                        .child(controls::icon("Search01Icon.svg", 32., muted.opacity(0.7)).mb_2())
                        .child(div().text_sm().child("No results found.")),
                )
            })
            .children(groups.into_iter().map(|(group, pages)| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .px_3()
                            .pb_1()
                            .text_size(px(11.))
                            .line_height(px(16.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(muted.opacity(0.6))
                            .child(group.to_uppercase()),
                    )
                    .children(pages.into_iter().map(|page| {
                        let active = page.id == self.active.id;
                        let locked = account::locked(page.id, page.requires_pro, &account);
                        let tint = if active { colors.foreground } else { muted };
                        let group = SharedString::from(format!("settings-nav-{}", page.id));
                        div()
                            .id(page.id)
                            .group(group.clone())
                            .w_full()
                            .px_3()
                            .py_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .rounded(px(8.))
                            .text_sm()
                            .line_height(px(20.))
                            .cursor_pointer()
                            .text_color(tint)
                            .when(active, |view| {
                                view.bg(colors.sidebar_accent)
                                    .font_weight(FontWeight::MEDIUM)
                            })
                            .when(!active, |view| {
                                view.hover(|style| style.bg(hover).text_color(colors.foreground))
                            })
                            .on_click(cx.listener(move |this, _, _, cx| this.select_page(page, cx)))
                            .child(
                                controls::icon(page.icon, 15., tint)
                                    .group_hover(group.clone(), |style| {
                                        style.text_color(colors.foreground)
                                    }),
                            )
                            .child(div().flex_1().min_w_0().truncate().child(page.label))
                            .when(locked, |view| {
                                view.child(
                                    controls::icon("product/LockPasswordIcon.svg", 14., tint)
                                        .group_hover(group, |style| {
                                            style.text_color(colors.foreground)
                                        }),
                                )
                            })
                    }))
            }));
        div()
            .w(px(200.))
            .h_full()
            .flex_shrink_0()
            .pr_1()
            .flex()
            .flex_col()
            .child(header)
            .child(search)
            .child(
                div()
                    .id("settings-navigation")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(list),
            )
    }

    fn page(&mut self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let preferences = self.service.is_none()
            && matches!(
                self.active.id,
                "app"
                    | "appearance"
                    | "notifications"
                    | "meetings"
                    | "privacy"
                    | "dictation"
                    | "dictionary"
            );
        if preferences && let Some(view) = self.preferences.clone() {
            return div().child(view);
        }
        let surface = self.service.as_ref().map(|view| view.read(cx).surface);
        if matches!(surface, None | Some(Surface::Teams | Surface::CloudSync))
            && let Some(page) = self.account_page(colors, cx)
        {
            return page;
        }
        let title = self
            .service
            .as_ref()
            .map(|view| view.read(cx).surface.title())
            .unwrap_or(self.active.label);
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(controls::page_title(title))
            .when(!self.notice.is_empty(), |view| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(colors.destructive)
                        .child(self.notice.clone()),
                )
            })
            .when_some(self.service.clone(), |view, service| view.child(service))
            .when(
                matches!(self.active.id, "transcription" | "intelligence")
                    && self.service.is_none(),
                |view| {
                    view.when_some(self.providers.clone(), |view, providers| {
                        view.child(providers)
                    })
                    .when(self.active.id == "transcription", |view| {
                        view.child(div().flex().child(controls::outline_button(
                            "local-models",
                            "Manage local models",
                            true,
                            colors,
                            Some(Box::new(cx.listener(|this, _, _, cx| {
                                this.show_surface(Some(Surface::Models), cx)
                            }))),
                        )))
                    })
                },
            )
    }
}

impl Render for ProductPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Onboarding is edge to edge, without the settings sidebar or content card.
        if let Some(onboarding) = self.onboarding.clone() {
            return div().size_full().child(onboarding);
        }
        let colors = theme(window);
        let dark = matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        let sidebar = self.sidebar(&colors, window, cx);
        let page = self.page(&colors, cx);
        let dialog = self.sign_out_dialog(&colors, cx);
        div()
            .relative()
            .size_full()
            .flex()
            .pl(px(4.))
            .bg(colors.background)
            .text_color(colors.foreground)
            .font_family(system_font(cx))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    this.search
                        .update(cx, |search, cx| search.set_text(String::new(), cx));
                    cx.notify();
                }
            }))
            .child(sidebar)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_hidden()
                    .bg(if dark { colors.accent } else { colors.card })
                    .border_l_1()
                    .border_color(colors.border)
                    .rounded_l(px(12.))
                    .child(
                        div()
                            .id("product-content")
                            .size_full()
                            .overflow_y_scroll()
                            .px_6()
                            .pt_6()
                            .pb_10()
                            .child(page),
                    ),
            )
            .children(dialog)
    }
}
