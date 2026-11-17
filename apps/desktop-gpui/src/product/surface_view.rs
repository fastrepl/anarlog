use std::sync::Arc;

use gpui::{Context, Entity, EventEmitter, Render, Subscription, Window, div, prelude::*};

use crate::contracts::ProductEvent;

use super::{
    PlanRequired,
    cloud::{CloudServices, calls, sync_view::SyncView, teams_view::TeamsView, view::CloudView},
    local::{developers_view::DevelopersView, view::LocalSettingsView},
    service_view::ServiceView,
    services::{ProductServices, Scope, Surface},
};

enum Mounted {
    Cloud(Entity<CloudView>),
    Local(Entity<LocalSettingsView>),
    Developers(Entity<DevelopersView>),
    Generic(Entity<ServiceView>),
    Teams(Entity<TeamsView>),
    Sync(Entity<SyncView>),
}

pub struct SurfaceView {
    pub surface: Surface,
    mounted: Mounted,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ProductEvent> for SurfaceView {}
impl EventEmitter<Scope> for SurfaceView {}
impl EventEmitter<PlanRequired> for SurfaceView {}

impl SurfaceView {
    pub fn new(
        services: Arc<dyn ProductServices>,
        cloud: Option<CloudServices>,
        surface: Surface,
        scope: Scope,
        cx: &mut Context<Self>,
    ) -> Self {
        let account_cloud = calls::installed(cx);
        let (mounted, subscriptions) = if surface == Surface::Teams
            && let Some(calls) = account_cloud.clone()
        {
            let view = cx.new(|cx| TeamsView::new(calls, cx));
            let subscriptions =
                vec![cx.subscribe(&view, |_, _, _: &PlanRequired, cx| cx.emit(PlanRequired))];
            (Mounted::Teams(view), subscriptions)
        } else if surface == Surface::CloudSync
            && let Some(calls) = account_cloud
        {
            (Mounted::Sync(cx.new(|cx| SyncView::new(calls, cx))), vec![])
        } else if matches!(
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
        } else if surface == Surface::Developers {
            let view = cx.new(DevelopersView::new);
            let subscriptions = vec![cx.subscribe(&view, |_, _, _: &super::PlanRequired, cx| {
                cx.emit(super::PlanRequired)
            })];
            (Mounted::Developers(view), subscriptions)
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

    /// The signed-in Teams and Sync pages draw their own title, like shipping's pages.
    pub fn titled(&self) -> bool {
        matches!(self.mounted, Mounted::Teams(_) | Mounted::Sync(_))
    }

    pub fn has_unsaved(&self, cx: &gpui::App) -> bool {
        match &self.mounted {
            Mounted::Teams(view) => view.read(cx).has_unsaved(),
            Mounted::Sync(view) => view.read(cx).has_unsaved(),
            Mounted::Cloud(view) => view.read(cx).has_unsaved(),
            Mounted::Local(view) => view.read(cx).has_unsaved(),
            Mounted::Developers(_) => false,
            Mounted::Generic(view) => view.read(cx).has_unsaved(),
        }
    }

    pub fn suspend(&mut self, cx: &mut Context<Self>) {
        match &self.mounted {
            Mounted::Teams(_) | Mounted::Sync(_) => {}
            Mounted::Cloud(view) => view.update(cx, |view, _| view.suspend()),
            Mounted::Local(view) => view.update(cx, |view, _| view.suspend()),
            Mounted::Developers(_) => {}
            Mounted::Generic(view) => view.update(cx, |view, _| view.suspend()),
        }
    }

    pub fn change_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        match &self.mounted {
            // These follow the app-wide account, not the pane's scope.
            Mounted::Teams(_) | Mounted::Sync(_) => {}
            Mounted::Cloud(view) => view.update(cx, |view, cx| view.change_scope(scope, cx)),
            Mounted::Local(view) => view.update(cx, |view, cx| view.change_scope(scope, cx)),
            // The page follows the app-wide account itself.
            Mounted::Developers(_) => {}
            Mounted::Generic(view) => view.update(cx, |view, cx| view.change_scope(scope, cx)),
        }
    }
}

impl Render for SurfaceView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        match &self.mounted {
            Mounted::Teams(view) => div().child(view.clone()),
            Mounted::Sync(view) => div().child(view.clone()),
            Mounted::Cloud(view) => div().child(view.clone()),
            Mounted::Local(view) => div().child(view.clone()),
            Mounted::Developers(view) => div().child(view.clone()),
            Mounted::Generic(view) => div().child(view.clone()),
        }
    }
}
