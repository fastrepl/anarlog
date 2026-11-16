pub mod cloud;
pub mod local;
mod navigation;
mod onboarding;
mod preferences_view;
mod providers;
mod service_view;
pub mod services;
pub mod settings;
mod surface_view;

use std::sync::Arc;

use gpui::{
    Context, Entity, EventEmitter, KeyDownEvent, Render, Subscription, Window, div, prelude::*, px,
};

use crate::{
    contracts::{LaneContext, ProductEvent, ProductRoute},
    ui::{
        input::{InputEvent, TextInput},
        theme::{system_font, theme},
    },
};

use onboarding::{Onboarding, Step};
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
    onboarding: Option<Onboarding>,
    notice: String,
    _search_subscription: Subscription,
    service_subscription: Option<Subscription>,
    settings_subscription: Option<Subscription>,
    scope_subscription: Option<Subscription>,
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
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| TextInput::new("Search settings", cx));
        let subscription = cx.subscribe(&search, |_, _, event, cx| {
            if matches!(event, InputEvent::Changed | InputEvent::Rejected) {
                cx.notify();
            }
        });
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
            onboarding: None,
            notice: String::new(),
            _search_subscription: subscription,
            service_subscription: None,
            settings_subscription: None,
            scope_subscription: None,
        };
        this.open_route(route, cx);
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

    pub fn open_route(&mut self, route: ProductRoute, cx: &mut Context<Self>) {
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
            ProductRoute::Account => ("account", Some(Surface::Account)),
            ProductRoute::Billing => ("billing", Some(Surface::Billing)),
            ProductRoute::CloudSync => ("sync", Some(Surface::CloudSync)),
            ProductRoute::Permissions => ("permissions", Some(Surface::Permissions)),
            ProductRoute::Share(_) => ("account", Some(Surface::Sharing)),
            ProductRoute::Export(_) => ("app", Some(Surface::Exports)),
            ProductRoute::Import => ("imports", Some(Surface::Imports)),
            ProductRoute::Models => ("transcription", Some(Surface::Models)),
            ProductRoute::Integrations => ("developers", Some(Surface::Developers)),
            ProductRoute::Calendar => ("calendar", Some(Surface::Calendar)),
            ProductRoute::Onboarding => {
                let flow = Onboarding::new(cfg!(target_os = "macos"));
                let surface = flow.step().surface();
                self.onboarding = Some(flow);
                ("app", Some(surface))
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
            "account" => Some(Surface::Account),
            "billing" => Some(Surface::Billing),
            "team" => Some(Surface::Teams),
            "sync" => Some(Surface::CloudSync),
            "imports" => Some(Surface::Imports),
            "permissions" => Some(Surface::Permissions),
            "developers" => Some(Surface::Developers),
            "calendar" | "folders" | "contacts" | "templates" | "automations" | "insights" => {
                cx.emit(OpenWorkspaceSection(page.id));
                cx.notify();
                return;
            }
            _ => None,
        };
        self.active = page;
        self.show_surface(surface, cx);
    }

    fn next_onboarding(&mut self, back: bool, cx: &mut Context<Self>) {
        let Some(flow) = &mut self.onboarding else {
            return;
        };
        if self
            .service
            .as_ref()
            .is_some_and(|view| view.read(cx).has_unsaved(cx))
        {
            self.notice = "Wait for the current operation and save or restore form edits.".into();
            cx.notify();
            return;
        }
        let ready = self
            .service
            .as_ref()
            .is_some_and(|view| view.read(cx).permissions_ready(cx));
        if if back {
            flow.back()
        } else {
            flow.advance(ready)
        } {
            let surface = flow.step().surface();
            self.show_surface(Some(surface), cx);
        } else if flow.step() == Step::Permissions {
            self.notice = "Microphone, system audio, and Accessibility access must all be verified by the native permission service.".into();
            cx.notify();
        }
    }
}

impl EventEmitter<ProductEvent> for ProductPane {}
impl EventEmitter<Scope> for ProductPane {}
impl EventEmitter<OpenWorkspaceSection> for ProductPane {}

impl Render for ProductPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let onboarding = self.onboarding.is_some();
        let title = self
            .onboarding
            .as_ref()
            .map(|flow| flow.step().label())
            .or_else(|| {
                self.service
                    .as_ref()
                    .map(|view| view.read(cx).surface.title())
            })
            .unwrap_or(self.active.label);
        let query = self.search.read(cx).buffer.text.clone();
        let mut sidebar = div()
            .w(px(232.))
            .h_full()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(colors.border)
            .child(div().p_3().child(self.search.clone()));
        let mut items = div()
            .id("settings-navigation")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_3();
        let mut group = "";
        for page in navigation::PAGES
            .iter()
            .copied()
            .filter(|page| navigation::matches(*page, &query))
        {
            if group != page.group {
                items = items.child(
                    div()
                        .pt_4()
                        .pb_2()
                        .text_xs()
                        .text_color(colors.muted_foreground)
                        .child(page.group),
                );
                group = page.group;
            }
            items = items.child(
                div()
                    .id(page.id)
                    .px_3()
                    .py_2()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .when(page == self.active, |view| view.bg(colors.accent))
                    .child(page.label)
                    .on_click(cx.listener(move |this, _, _, cx| this.select_page(page, cx))),
            );
        }
        sidebar = sidebar.child(items);
        let content = div()
            .id("product-content")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_2xl()
                    .font_family(if cfg!(target_os = "macos") {
                        "Bradley Hand"
                    } else if cfg!(target_os = "windows") {
                        "Segoe Print"
                    } else {
                        "Comic Sans MS"
                    })
                    .child(title),
            )
            .when(onboarding, |view| {
                view.child(
                    div().flex().gap_3().children(
                        self.onboarding
                            .as_ref()
                            .into_iter()
                            .flat_map(|flow| flow.steps.iter())
                            .map(|step| div().text_sm().child(step.label())),
                    ),
                )
            })
            .when(!self.notice.is_empty(), |view| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(colors.destructive)
                        .child(self.notice.clone()),
                )
            })
            .when_some(self.service.clone(), |view, service| view.child(service))
            .when(self.service.is_none(), |view| {
                view.when_some(self.preferences.clone(), |view, preferences| {
                    view.child(preferences)
                })
            })
            .when(
                matches!(self.active.id, "transcription" | "intelligence")
                    && !onboarding
                    && self.service.is_none(),
                |view| {
                    view.when_some(self.providers.clone(), |view, providers| {
                        view.child(providers)
                    })
                    .child(
                        div()
                            .id("local-models")
                            .cursor_pointer()
                            .child("Manage local models")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_surface(Some(Surface::Models), cx)
                            })),
                    )
                },
            )
            .when(
                self.active.id == "app" && !onboarding && self.service.is_none(),
                |view| {
                    view.child(
                        div()
                            .id("storage")
                            .cursor_pointer()
                            .child("Storage location")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_surface(Some(Surface::Storage), cx)
                            })),
                    )
                },
            )
            .when(onboarding, |view| {
                view.child(
                    div()
                        .flex()
                        .gap_4()
                        .child(
                            div()
                                .id("onboarding-back")
                                .cursor_pointer()
                                .child("Back")
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.next_onboarding(true, cx)),
                                ),
                        )
                        .when(
                            self.onboarding
                                .as_ref()
                                .is_some_and(|flow| flow.step() != Step::Final),
                            |view| {
                                view.child(
                                    div()
                                        .id("onboarding-next")
                                        .cursor_pointer()
                                        .child(
                                            if self.onboarding.as_ref().is_some_and(|flow| {
                                                flow.step() == Step::Permissions
                                            }) {
                                                "Continue"
                                            } else {
                                                "Skip for now"
                                            },
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.next_onboarding(false, cx)
                                        })),
                                )
                            },
                        ),
                )
            });
        div()
            .size_full()
            .flex()
            .flex_col()
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
            .child(
                div()
                    .h(px(56.))
                    .px_6()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(
                        div()
                            .id("back-to-workspace")
                            .cursor_pointer()
                            .child("Back to workspace")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.can_close(cx) {
                                    cx.emit(ProductEvent::NavigateWorkspace);
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .when(!onboarding, |view| view.child(sidebar))
                    .child(content),
            )
    }
}
