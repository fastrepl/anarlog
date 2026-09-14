mod ai;
mod billing;
mod core;
mod sync;

use axum::Router;

pub(super) use ai::router as ai;
pub(super) use billing::router as billing;
pub(super) use core::router as core;
#[cfg(test)]
pub(super) use sync::build_sync_routes;
pub(super) use sync::router as sync;

fn subscription_aliases(router: Router) -> Router {
    Router::new()
        .nest("/subscription", router.clone())
        .nest("/rpc", router.clone())
        .nest("/billing", router)
}
