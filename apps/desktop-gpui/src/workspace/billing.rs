//! `auth/billing-context.ts`: `useBillingAccess()` for the Pro gates. The
//! shell has no signed-in Supabase session yet, so the claims never resolve
//! and every gate behaves like the Tauri app signed out.

use super::Workspace;

impl Workspace {
    /// `billing.isPro`
    pub(crate) fn is_pro(&self) -> bool {
        false
    }

    /// `billing.isReady`: the claims query never settles without a session.
    pub(crate) fn billing_ready(&self) -> bool {
        false
    }
}
