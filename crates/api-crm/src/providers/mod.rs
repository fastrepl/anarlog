mod attio;
mod close;
mod hubspot;
mod pipedrive;
mod salesforce;

use anlg_nango::OwnedNangoHttpClient;
use futures_util::future::BoxFuture;

use crate::contacts::{CrmContact, CrmContactQuery};
use crate::error::Result;

pub const MAX_CONTACT_RESULTS: usize = 5;

pub type SearchFn =
    fn(OwnedNangoHttpClient, CrmContactQuery, usize) -> BoxFuture<'static, Result<Vec<CrmContact>>>;

/// A CRM reachable through a Nango integration. `nango_integration_id` must
/// match the integration configured in the Nango dashboard; `search` issues
/// the provider's contact search through the Nango proxy, requesting up to
/// `limit` upstream records.
pub struct CrmProvider {
    pub id: &'static str,
    pub name: &'static str,
    pub nango_integration_id: &'static str,
    pub search: SearchFn,
}

pub fn resolve(provider_id: &str) -> Option<&'static CrmProvider> {
    PROVIDERS
        .iter()
        .find(|provider| provider.id == provider_id)
        .copied()
}

pub static PROVIDERS: &[&CrmProvider] = &[
    &hubspot::PROVIDER,
    &attio::PROVIDER,
    &salesforce::PROVIDER,
    &pipedrive::PROVIDER,
    &close::PROVIDER,
];
