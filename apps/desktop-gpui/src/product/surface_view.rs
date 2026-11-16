use std::sync::Arc;

use gpui::{Context, Entity, EventEmitter, Render, Subscription, Window, div, prelude::*};

use crate::contracts::ProductEvent;

use super::{
    cloud::{CloudServices, view::CloudView},
    local::view::LocalSettingsView,
    service_view::ServiceView,
    services::{ProductServices, Scope, Surface},
};

enum Mounted {
    Cloud(Entity<CloudView>),
    Local(Entity<LocalSettingsView>),
    Generic(Entity<ServiceView>),
}

pub struct SurfaceView {
    pub surface: Surface,
    mounted: Mounted,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ProductEvent> for SurfaceView {}
impl EventEmitter<Scope> for SurfaceView {}

impl SurfaceView {
    pub fn new(
        services: Arc<dyn ProductServices>,
        cloud: Option<CloudServices>,
        surface: Surface,
        scope: Scope,
        cx: &mut Context<Self>,
    ) -> Self {
        let (mounted, subscriptions) = if matches!(
            surface,
            Surface::Account
                | Surface::Billing
                | Surface::Teams
                | Surface::CloudSync
                | Surface::Sharing
        ) && let Some(cloud) = cloud
        {
            let view = cx.new(|cx| CloudView::new(cloud, surface, scope, cx));
            let subscriptions = vec![
                cx.subscribe(&view, |_, _, event: &ProductEvent, cx| {
                    cx.emit(event.clone())
                }),
                cx.subscribe(&view, |_, _, scope: &Scope, cx| cx.emit(scope.clone())),
            ];
            (Mounted::Cloud(view), subscriptions)
        } else if matches!(
            surface,
            Surface::Onboarding
                | Surface::Permissions
                | Surface::Imports
                | Surface::Exports
                | Surface::Models
                | Surface::Calendar
                | Surface::Developers
                | Surface::Storage
        ) {
            let view = cx.new(|cx| LocalSettingsView::new(services, surface, scope, cx));
            let subscriptions = vec![
                cx.subscribe(&view, |_, _, event: &ProductEvent, cx| {
                    cx.emit(event.clone())
                }),
                cx.subscribe(&view, |_, _, scope: &Scope, cx| cx.emit(scope.clone())),
            ];
            (Mounted::Local(view), subscriptions)
        } else {
            let view = cx.new(|cx| ServiceView::new(services, surface, scope, cx));
            let subscriptions = vec![
                cx.subscribe(&view, |_, _, event: &ProductEvent, cx| {
                    cx.emit(event.clone())
                }),
                cx.subscribe(&view, |_, _, scope: &Scope, cx| cx.emit(scope.clone())),
            ];
            (Mounted::Generic(view), subscriptions)
        };
        Self {
            surface,
            mounted,
            _subscriptions: subscriptions,
        }
    }

    pub fn has_unsaved(&self, cx: &gpui::App) -> bool {
        match &self.mounted {
            Mounted::Cloud(view) => view.read(cx).has_unsaved(),
            Mounted::Local(view) => view.read(cx).has_unsaved(),
            Mounted::Generic(view) => view.read(cx).has_unsaved(),
        }
    }

    pub fn permissions_ready(&self, cx: &gpui::App) -> bool {
        match &self.mounted {
            Mounted::Local(view) => view.read(cx).permissions_ready(),
            Mounted::Generic(view) => view.read(cx).permissions_ready(),
            Mounted::Cloud(_) => false,
        }
    }

    pub fn suspend(&mut self, cx: &mut Context<Self>) {
        match &self.mounted {
            Mounted::Cloud(view) => view.update(cx, |view, _| view.suspend()),
            Mounted::Local(view) => view.update(cx, |view, _| view.suspend()),
            Mounted::Generic(view) => view.update(cx, |view, _| view.suspend()),
        }
    }

    pub fn change_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        match &self.mounted {
            Mounted::Cloud(view) => view.update(cx, |view, cx| view.change_scope(scope, cx)),
            Mounted::Local(view) => view.update(cx, |view, cx| view.change_scope(scope, cx)),
            Mounted::Generic(view) => view.update(cx, |view, cx| view.change_scope(scope, cx)),
        }
    }
}

impl Render for SurfaceView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        match &self.mounted {
            Mounted::Cloud(view) => div().child(view.clone()),
            Mounted::Local(view) => div().child(view.clone()),
            Mounted::Generic(view) => div().child(view.clone()),
        }
    }
}
