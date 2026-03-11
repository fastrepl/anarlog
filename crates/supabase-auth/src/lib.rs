// Supabase JWT authentication utilities.
//
// References:
// - https://supabase.com/docs/guides/auth/jwts
// - https://supabase.com/docs/guides/auth/signing-keys

mod claims;
pub use claims::*;

mod error;
pub use error::*;

#[cfg(feature = "server")]
pub mod server;

#[cfg(feature = "client")]
pub mod client;
