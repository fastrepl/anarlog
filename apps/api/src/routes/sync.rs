use crate::rate_limit;
use crate::{
    auth::{self, AuthState},
    env::RuntimeConfig,
};
use axum::{Router, middleware};
use std::{num::NonZeroU32, time::Duration};

pub(crate) fn build_sync_routes(
    state: Option<anlg_api_sync::AppState>,
    replica_state: anlg_api_sync::ReplicaState,
    cloudsync_rate_limit_state: rate_limit::RateLimitState,
    device_rate_limit_state: rate_limit::RateLimitState,
    session_share_rate_limit_state: rate_limit::RateLimitState,
    witness_rate_limit_state: rate_limit::RateLimitState,
    auth_state: AuthState,
) -> Router {
    let replica_routes = anlg_api_sync::replica_router(replica_state.clone())
        .route_layer(middleware::from_fn_with_state(
            cloudsync_rate_limit_state.clone(),
            rate_limit::rate_limit,
        ))
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            auth_state.clone().with_required_entitlement("hyprnote_pro"),
            auth::require_auth,
        ));
    let device_routes = anlg_api_sync::device_router(replica_state.clone())
        .route_layer(middleware::from_fn_with_state(
            device_rate_limit_state,
            rate_limit::rate_limit,
        ))
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            auth_state.clone().with_required_entitlement("hyprnote_pro"),
            auth::require_auth,
        ));
    let witness_routes = anlg_api_sync::e2ee_witness_router(replica_state)
        .route_layer(middleware::from_fn_with_state(
            witness_rate_limit_state,
            rate_limit::wait_for_rate_limit,
        ))
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            auth_state.clone().with_required_entitlement("hyprnote_pro"),
            auth::require_auth,
        ));
    let replica_routes = replica_routes.merge(device_routes).merge(witness_routes);

    let Some(state) = state else {
        return replica_routes;
    };

    let cloudsync_routes = anlg_api_sync::cloudsync_router(state.clone())
        .route_layer(middleware::from_fn_with_state(
            cloudsync_rate_limit_state,
            rate_limit::rate_limit,
        ))
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            auth_state.clone().with_required_entitlement("hyprnote_pro"),
            auth::require_auth,
        ));
    let session_share_routes = anlg_api_sync::session_share_router(state.clone())
        .route_layer(middleware::from_fn_with_state(
            session_share_rate_limit_state.clone(),
            rate_limit::rate_limit,
        ))
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            auth_state.clone().with_required_entitlement("hyprnote_pro"),
            auth::require_auth,
        ));
    let web_edit_routes = anlg_api_sync::web_edit_router(state)
        .route_layer(middleware::from_fn_with_state(
            session_share_rate_limit_state,
            rate_limit::rate_limit,
        ))
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            auth_state,
            auth::require_auth,
        ));

    replica_routes
        .merge(cloudsync_routes)
        .merge(session_share_routes)
        .merge(web_edit_routes)
}

pub(crate) fn router(env: &RuntimeConfig) -> Router {
    let build_sync_rate_limit = || {
        let quota = || {
            governor::Quota::with_period(Duration::from_secs(30))
                .unwrap()
                .allow_burst(NonZeroU32::new(20).unwrap())
        };
        rate_limit::RateLimitState::builder()
            .pro(quota())
            .free(quota())
            .build()
    };
    let cloudsync_rate_limit = build_sync_rate_limit();
    let device_quota = rate_limit::device_management_quota();
    let device_rate_limit = rate_limit::RateLimitState::builder()
        .pro(device_quota)
        .free(device_quota)
        .build();
    let session_share_rate_limit = build_sync_rate_limit();
    let e2ee_witness_rate_limit = rate_limit::RateLimitState::builder()
        .pro(
            governor::Quota::with_period(Duration::from_millis(100))
                .unwrap()
                .allow_burst(NonZeroU32::new(20).unwrap()),
        )
        .free(
            governor::Quota::with_period(Duration::from_millis(100))
                .unwrap()
                .allow_burst(NonZeroU32::new(20).unwrap()),
        )
        .build();
    let shared_notes_rate_limit = rate_limit::IpRateLimitState::new(
        governor::Quota::with_period(Duration::from_secs(1))
            .unwrap()
            .allow_burst(NonZeroU32::new(30).unwrap()),
    );
    let cloud_api_rate_limit = rate_limit::RateLimitState::builder()
        .pro(
            governor::Quota::with_period(Duration::from_millis(200))
                .unwrap()
                .allow_burst(NonZeroU32::new(10).unwrap()),
        )
        .free(
            governor::Quota::with_period(Duration::from_millis(200))
                .unwrap()
                .allow_burst(NonZeroU32::new(10).unwrap()),
        )
        .build();

    let auth_state = AuthState::new(&env.supabase.supabase_url);
    let sync_config = anlg_api_sync::SyncConfig::from_env(
        &env.sync,
        &env.supabase.supabase_url,
        &env.supabase.supabase_anon_key,
        &env.supabase.supabase_service_role_key,
    )
    .unwrap_or_else(|error| panic!("Failed to load environment: {error}"));
    let shared_notes_config = anlg_api_sync::SharedNotesConfig::new(
        &env.supabase.supabase_url,
        &env.supabase.supabase_service_role_key,
    )
    .unwrap_or_else(|error| panic!("Failed to load environment: {error}"));
    let (Some(resend_api_key), Some(resend_from_email)) = (
        env.resend.resend_api_key.as_deref(),
        env.resend.resend_from_email.as_deref(),
    ) else {
        panic!(
            "Failed to load environment: RESEND_API_KEY and RESEND_FROM_EMAIL are required for shared note email"
        );
    };
    let shared_notes_config = shared_notes_config
        .with_resend_email(resend_api_key, resend_from_email)
        .unwrap_or_else(|error| panic!("Failed to load environment: {error}"));
    let cloud_api_state = anlg_api_cloud::AppState::new(
        anlg_api_cloud::CloudApiConfig::new(
            &env.supabase.supabase_url,
            &env.supabase.supabase_service_role_key,
        )
        .unwrap_or_else(|error| panic!("Failed to load environment: {error}")),
    );

    let replica_state = anlg_api_sync::ReplicaState::new(
        anlg_api_sync::ReplicaConfig::new(
            &env.supabase.supabase_url,
            &env.supabase.supabase_anon_key,
            &env.supabase.supabase_service_role_key,
        )
        .unwrap_or_else(|error| panic!("Failed to load environment: {error}")),
    );
    let sync_state = sync_config.map(anlg_api_sync::AppState::new);
    let sync_routes = build_sync_routes(
        sync_state,
        replica_state,
        cloudsync_rate_limit,
        device_rate_limit,
        session_share_rate_limit,
        e2ee_witness_rate_limit,
        auth_state.clone(),
    );
    let shared_notes_state = anlg_api_sync::SharedNotesState::new(shared_notes_config);
    let shared_notes_routes = anlg_api_sync::shared_notes_router(shared_notes_state.clone())
        .route_layer(middleware::from_fn_with_state(
            shared_notes_rate_limit.clone(),
            rate_limit::rate_limit_by_ip,
        ));
    let authenticated_shared_notes_routes =
        anlg_api_sync::authenticated_shared_notes_router(shared_notes_state)
            .route_layer(middleware::from_fn_with_state(
                shared_notes_rate_limit,
                rate_limit::rate_limit_by_ip,
            ))
            .route_layer(middleware::from_fn(auth::sentry_and_analytics))
            .route_layer(middleware::from_fn_with_state(
                auth_state.clone(),
                auth::require_auth,
            ));
    let cloud_api_management_routes = anlg_api_cloud::management_router(cloud_api_state.clone())
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            auth_state.clone(),
            auth::require_auth,
        ));
    let cloud_api_connector_routes = anlg_api_cloud::connector_router(cloud_api_state.clone())
        .route_layer(middleware::from_fn_with_state(
            cloud_api_rate_limit,
            rate_limit::rate_limit,
        ))
        .route_layer(middleware::from_fn(auth::sentry_and_analytics))
        .route_layer(middleware::from_fn_with_state(
            cloud_api_state.clone(),
            anlg_api_cloud::require_cloud_connector_auth,
        ));
    let cloud_api_oauth_routes = anlg_api_cloud::oauth_metadata_router(cloud_api_state);

    Router::new()
        .nest("/sync", sync_routes)
        .merge(shared_notes_routes)
        .merge(authenticated_shared_notes_routes)
        .merge(cloud_api_management_routes)
        .merge(cloud_api_connector_routes)
        .merge(cloud_api_oauth_routes)
}
