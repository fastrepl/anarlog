mod create;
mod enhance;
mod merge;
mod pin;
mod update;

pub use create::{
    CreateHumanRequest, CreateOrganizationRequest, SavePersonalContactRequest, create_human,
    create_organization, save_personal_contact,
};
pub use enhance::{ApplyContactEnhancementRequest, apply_contact_enhancement};
pub use merge::{MergeHumansRequest, merge_humans};
pub use pin::{
    ReorderPinnedContactsRequest, ToggleContactPinRequest, reorder_pinned_contacts,
    toggle_contact_pin,
};
pub use update::{
    SoftDeleteContactRequest, UpdateContactAvatarRequest, UpdateHumanContactSummaryRequest,
    UpdateHumanRequest, UpdateOrganizationRequest, soft_delete_contact, update_contact_avatar,
    update_human, update_human_contact_summary, update_organization,
};

use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use crate::storage::transaction_utils::rollback_error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ContactKind {
    Human,
    Organization,
}

impl ContactKind {
    fn table(self) -> &'static str {
        match self {
            ContactKind::Human => "humans",
            ContactKind::Organization => "organizations",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct PinnedContactEntry {
    pub kind: ContactKind,
    pub id: String,
}

type WriteFut<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>>;

pub(crate) async fn run_write<F>(pool: &SqlitePool, write: F) -> Result<(), String>
where
    F: for<'a> FnOnce(&'a mut sqlx::SqliteConnection) -> WriteFut<'a>,
{
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    if let Err(error) = write(&mut transaction).await {
        return Err(rollback_error(transaction, error).await);
    }

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}
