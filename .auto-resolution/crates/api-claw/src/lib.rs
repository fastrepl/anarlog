//! User-scoped VM lifecycle on top of [`exedev`](../exedev/index.html).
//!
//! The `exedev` crate is a thin typed SDK over the exe.dev HTTPS API. This
//! crate adds the conventions we need to treat VMs as "one per user":
//!
//! - Deterministic naming: [`UserId`] → [`vm_name`] (`claw-<hex>`), so any
//!   caller can look up a user's VM with a single `ls <name>` query and the
//!   name is a DNS-safe subdomain (`claw-<hex>.exe.xyz`).
//! - [`UserKeyring`] abstracts where per-user SSH keys live (supabase, kms,
//!   in-memory for tests). The manager never touches storage directly.
//! - [`ClawManager`] orchestrates `provision / get / suspend / resume /
//!   deprovision`, mints VM-scoped tokens with a `ctx` that carries the user
//!   id through to claw (via `X-ExeDev-Token-Ctx`), and exposes a typed
//!   `call::<Req, Res>` helper for HTTPS control-plane requests.
//!
//! Billing is intentionally out of scope: callers (e.g. the Stripe webhook in
//! `apps/api`) decide *when* to provision/suspend/deprovision and delegate
//! the mechanics here.

mod keyring;
mod manager;
mod naming;

pub use hypr_exedev as exedev;

pub use keyring::{InMemoryKeyring, UserKeyring};
pub use manager::{
    ClawCallOptions, ClawManager, ClawProvisionSpec, ClawTokenCtx, ProvisionOutcome,
};
pub use naming::{UserId, VM_NAME_PREFIX, vm_name};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Exedev(#[from] hypr_exedev::Error),
    #[error("keyring backend: {0}")]
    Keyring(String),
    #[error("ssh key pem generation: {0}")]
    KeyGen(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
