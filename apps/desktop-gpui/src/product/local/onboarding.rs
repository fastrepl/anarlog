use std::sync::Arc;

use gpui::{Context, Entity, EventEmitter, Render, Window, div, prelude::*, px};

use super::view::LocalSettingsView;
use crate::{
    contracts::ProductEvent,
    product::{
        service_view::ServiceView,
        services::{ProductServices, Scope, Surface},
    },
    ui::theme::theme,
};

pub struct OnboardingView {
    services: Arc<dyn ProductServices>,
    scope: Scope,
    steps: Vec<Surface>,
    current: usize,
    content: StepContent,
}

impl EventEmitter<ProductEvent> for OnboardingView {}
impl EventEmitter<Scope> for OnboardingView {}

enum StepContent {
    Local(Entity<LocalSettingsView>),
    Account(Entity<ServiceView>),
}

impl OnboardingView {
    pub fn new(services: Arc<dyn ProductServices>, scope: Scope, cx: &mut Context<Self>) -> Self {
        let mut steps = Vec::new();
        if cfg!(target_os = "macos") {
            steps.push(Surface::Permissions);
        }
        steps.extend([
            Surface::Account,
            Surface::Calendar,
            Surface::Imports,
            Surface::Onboarding,
        ]);
        let content = Self::content(services.clone(), steps[0], scope.clone(), cx);
        Self {
            services,
            scope,
            steps,
            current: 0,
            content,
        }
    }

    pub fn has_unsaved(&self, cx: &gpui::App) -> bool {
        match &self.content {
            StepContent::Local(view) => view.read(cx).has_unsaved(),
            StepContent::Account(view) => view.read(cx).has_unsaved(),
        }
    }

    pub fn set_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        if self.scope != scope {
            self.scope = scope;
            self.replace_content(cx);
        }
    }

    fn replace_content(&mut self, cx: &mut Context<Self>) {
        match &self.content {
            StepContent::Local(view) => view.update(cx, |view, _| view.suspend()),
            StepContent::Account(view) => view.update(cx, |view, _| view.suspend()),
        }
        self.content = Self::content(
            self.services.clone(),
            self.steps[self.current],
            self.scope.clone(),
            cx,
        );
        cx.notify();
    }

    fn content(
        services: Arc<dyn ProductServices>,
        surface: Surface,
        scope: Scope,
        cx: &mut Context<Self>,
    ) -> StepContent {
        if surface == Surface::Account {
            let view = cx.new(|cx| ServiceView::new(services, surface, scope, cx));
            cx.subscribe(&view, |_, _, event: &ProductEvent, cx| {
                cx.emit(event.clone())
            })
            .detach();
            cx.subscribe(&view, |this, _, scope: &Scope, cx| {
                this.scope = scope.clone();
                cx.emit(scope.clone());
            })
            .detach();
            StepContent::Account(view)
        } else {
            let view = cx.new(|cx| LocalSettingsView::new(services, surface, scope, cx));
            cx.subscribe(&view, |_, _, event: &ProductEvent, cx| {
                cx.emit(event.clone())
            })
            .detach();
            cx.subscribe(&view, |this, _, scope: &Scope, cx| {
                this.scope = scope.clone();
                cx.emit(scope.clone());
            })
            .detach();
            StepContent::Local(view)
        }
    }

    fn permissions_ready(&self, cx: &gpui::App) -> bool {
        match &self.content {
            StepContent::Local(view) => view.read(cx).permissions_ready(),
            StepContent::Account(_) => false,
        }
    }

    fn next(&mut self, cx: &mut Context<Self>) {
        if self.has_unsaved(cx) || self.current + 1 == self.steps.len() {
            return;
        }
        if self.steps[self.current] == Surface::Permissions && !self.permissions_ready(cx) {
            return;
        }
        self.current += 1;
        self.replace_content(cx);
    }
}

impl Render for OnboardingView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let active = self.steps[self.current];
        let can_continue =
            !self.has_unsaved(cx) && (active != Surface::Permissions || self.permissions_ready(cx));
        div()
            .id("onboarding")
            .size_full()
            .overflow_y_scroll()
            .px_12()
            .py_4()
            .child(
                div()
                    .max_w(px(720.))
                    .mx_auto()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Welcome to Anarlog"),
                    )
                    .children(self.steps.iter().take(self.current).map(|step| {
                        div()
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child(format!("{} completed", step.title()))
                    }))
                    .child(match &self.content {
                        StepContent::Local(view) => view.clone().into_any_element(),
                        StepContent::Account(view) => view.clone().into_any_element(),
                    })
                    .when(active != Surface::Onboarding, |view| {
                        view.child(
                            div()
                                .flex()
                                .gap_3()
                                .child(
                                    div()
                                        .id("onboarding-continue")
                                        .rounded_full()
                                        .px_6()
                                        .py_2()
                                        .bg(colors.accent)
                                        .text_sm()
                                        .child("Continue")
                                        .when(can_continue, |button| {
                                            button.cursor_pointer().on_click(
                                                cx.listener(|this, _, _, cx| this.next(cx)),
                                            )
                                        }),
                                )
                                .when(active != Surface::Permissions, |view| {
                                    view.child(
                                        div()
                                            .id("onboarding-skip")
                                            .text_sm()
                                            .text_color(colors.muted_foreground)
                                            .px_3()
                                            .py_2()
                                            .child("Skip")
                                            .when(can_continue, |button| {
                                                button.cursor_pointer().on_click(
                                                    cx.listener(|this, _, _, cx| this.next(cx)),
                                                )
                                            }),
                                    )
                                }),
                        )
                    }),
            )
    }
}
