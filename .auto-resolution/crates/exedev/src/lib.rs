//! Typed client for the exe.dev HTTPS API (`POST /exec`) and VM HTTPS proxy.
//!
//! # Crate layout
//!
//! - [`ExedevClient`] drives `POST https://exe.dev/exec`. One of:
//!   - a pre-minted bearer `token` (exe0 or exe1), or
//!   - a `signing_key` + `permissions` to mint an exe0 at build time.
//! - [`VmHttpClient`] drives the per-VM auth proxy at `https://<vm>.exe.xyz`.
//! - [`Exe0Token`] / [`Exe1Token`] model the two token formats, including
//!   [`Exe0Token::mint_for_vm`] for VM-scoped tokens that set
//!   `X-ExeDev-Token-Ctx` on your VM's HTTP server.
//!
//! All live-wire response shapes are typed in [`models`]; see
//! `commands::tests` for fixtures derived from the live API.
mod client;
mod commands;
mod error;
mod http_vm;
pub mod models;
mod token;

pub use client::{DEFAULT_API_BASE, ExedevClient, ExedevClientBuilder};
pub use commands::{GenerateApiKeyArgs, ShareVisibility, VmCopySpec, VmNewArgs, VmResizeSpec};
pub use error::Error;
pub use http_vm::{VmAuth, VmHttpClient};
pub use models::{GeneratedApiKey, SshKey, Vm, VmStat, VmStatus, WhoAmI};
pub use token::{Exe0Token, Exe1Token, NAMESPACE_API, Permissions, namespace_vm};
