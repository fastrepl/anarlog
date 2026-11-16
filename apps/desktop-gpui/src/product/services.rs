use std::{collections::BTreeSet, sync::Arc};

use desktop_runtime::{CancellationToken, Result, ServiceError, SessionId};
use futures::future::BoxFuture;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scope {
    pub account_id: Option<Arc<str>>,
    pub workspace_id: Option<Arc<str>>,
    pub session_id: Option<SessionId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Permissions,
    Account,
    Billing,
    Teams,
    CloudSync,
    Sharing,
    Imports,
    Exports,
    Models,
    Calendar,
    Developers,
    Onboarding,
    Storage,
}

impl Surface {
    pub fn title(self) -> &'static str {
        match self {
            Self::Permissions => "Permissions",
            Self::Account => "Account",
            Self::Billing => "Billing",
            Self::Teams => "Teams",
            Self::CloudSync => "Sync",
            Self::Sharing => "Share meeting",
            Self::Imports => "Import meetings",
            Self::Exports => "Export meeting",
            Self::Models => "Local models",
            Self::Calendar => "Calendar",
            Self::Developers => "Developers",
            Self::Onboarding => "Welcome to Anarlog",
            Self::Storage => "Storage",
        }
    }

    pub fn missing_service(self) -> &'static str {
        match self {
            Self::Permissions => {
                "Native microphone, system audio, and Accessibility permission services are not connected."
            }
            Self::Account => {
                "Secure session persistence, browser authentication, admission checks, and sign-out coordination are not connected."
            }
            Self::Billing => {
                "Authenticated entitlement, trial, checkout, and billing portal services are not connected."
            }
            Self::Teams => {
                "Workspace membership, invitations, ownership transfer, policy, and SCIM services are not connected."
            }
            Self::CloudSync => {
                "Encrypted sync, device enrollment, recovery keys, Keychain repair, and local-library isolation services are not connected."
            }
            Self::Sharing => {
                "Canonical editor flush, scoped share projections, access management, publication, and conflict reconciliation are not connected."
            }
            Self::Imports => {
                "Provider discovery, authorized connections, parsing, and non-overwriting import transactions are not connected. No files have been imported."
            }
            Self::Exports => {
                "Lossless editor/transcript snapshots, format conversion, PDF generation, and native destination services are not connected. No output has been written."
            }
            Self::Models => {
                "The supported-model registry, downloads, file inspection, and engine lifecycle services are not connected. Custom STT GGUF files are unsupported by the shipping engine."
            }
            Self::Calendar => {
                "Calendar provider connections are not connected. Apple Calendar requires macOS."
            }
            Self::Developers => {
                "Installed CLI/MCP paths, agent skill installation, Cloud API opt-in, keys, and webhook services are not connected."
            }
            Self::Onboarding => {
                "Welcome-session reuse, durable onboarding completion, analytics, and relaunch services are not connected. Setup has not been marked complete."
            }
            Self::Storage => {
                "The flush, validated copy-before-switch, and relaunch storage service is not connected."
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    RequestPermission,
    OpenPermissionSettings,
    DismissPermissionAssistant,
    SignIn,
    SignOut,
    LogoutLibrary,
    RefreshAccount,
    StartTrial,
    Checkout,
    BillingPortal,
    CreateWorkspace,
    AcceptInvitation,
    DeclineInvitation,
    InviteMember,
    ResendInvitation,
    RevokeInvitation,
    SetMemberRole,
    TransferOwnership,
    LeaveWorkspace,
    DeleteWorkspace,
    SetWorkspacePolicy,
    SetSharingSlug,
    SetupEncryption,
    ImportRecoveryKey,
    CopyRecoveryKey,
    ExportRecoveryKey,
    ApproveDevice,
    ReplaceDevice,
    RenameDevice,
    RemoveDevice,
    PauseSync,
    ResumeSync,
    SyncNow,
    RepairKeychain,
    ConnectLocalLibrary,
    CreateShare,
    InviteToShare,
    SetShareRole,
    SetShareScope,
    EnableShareLink,
    CopyShareLink,
    RotateShareLink,
    RevokeShareLink,
    DeleteShare,
    RespondShareRequest,
    AddComment,
    DeleteComment,
    PublishShare,
    ImportFiles,
    ConnectImport,
    SyncImport,
    CancelImport,
    RetryImport,
    Export,
    OpenOutput,
    DownloadModel,
    CancelDownload,
    DeleteModel,
    RetryDownload,
    StartModel,
    StopModel,
    InspectCustomModel,
    SelectProvider,
    ConnectCalendar,
    DisconnectCalendar,
    InstallCli,
    InstallMcp,
    InstallSkills,
    EnableCloudApi,
    BackfillCloudApi,
    CreateApiKey,
    CopyApiKey,
    RevokeApiKey,
    CreateWebhook,
    DeleteWebhook,
    SetWebhookActive,
    TestWebhook,
    MoveStorage,
    CompleteOnboarding,
    NextPage,
}

impl Action {
    pub fn requires_confirmation(&self) -> bool {
        matches!(
            self,
            Self::LogoutLibrary
                | Self::TransferOwnership
                | Self::LeaveWorkspace
                | Self::DeleteWorkspace
                | Self::RemoveDevice
                | Self::ReplaceDevice
                | Self::RevokeShareLink
                | Self::DeleteShare
                | Self::DeleteModel
                | Self::DisconnectCalendar
                | Self::EnableCloudApi
                | Self::RevokeApiKey
                | Self::DeleteWebhook
                | Self::MoveStorage
        )
    }

    pub fn clears_identity(&self) -> bool {
        matches!(self, Self::SignOut | Self::LogoutLibrary)
    }
}

#[derive(Clone, Debug)]
pub struct Field {
    pub id: Arc<str>,
    pub label: Arc<str>,
    pub value: Arc<str>,
}

#[derive(Clone, Debug)]
pub struct Operation {
    pub id: Arc<str>,
    pub label: Arc<str>,
    pub action: Action,
    pub target_id: Option<Arc<str>>,
    pub disabled_reason: Option<Arc<str>>,
    pub confirmation: Option<Arc<str>>,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub id: Arc<str>,
    pub title: Arc<str>,
    pub detail: Arc<str>,
}

#[derive(Clone, Debug)]
pub struct Panel {
    pub scope: Scope,
    pub status: Arc<str>,
    pub rows: Arc<[Row]>,
    pub fields: Arc<[Field]>,
    pub operations: Arc<[Operation]>,
    pub permissions_ready: bool,
}

impl Panel {
    pub fn validate(&self, scope: &Scope) -> Result<()> {
        if &self.scope != scope {
            return Err(ServiceError::Conflict);
        }
        if self.rows.len() > 128 || self.fields.len() > 16 || self.operations.len() > 64 {
            return Err(ServiceError::Unsupported(
                "Product response exceeds the page limit".into(),
            ));
        }
        let mut ids = BTreeSet::new();
        for row in self.rows.iter() {
            if row.id.is_empty()
                || row.id.len() > 256
                || !ids.insert(row.id.clone())
                || row.title.len() > 4096
                || row.detail.len() > 16_384
            {
                return Err(ServiceError::Failed("Invalid product row".into()));
            }
        }
        ids.clear();
        for field in self.fields.iter() {
            if field.id.is_empty()
                || field.id.len() > 256
                || !ids.insert(field.id.clone())
                || field.value.len() > 4096
                || field.label.len() > 256
            {
                return Err(ServiceError::Failed("Invalid product field".into()));
            }
        }
        ids.clear();
        for operation in self.operations.iter() {
            if operation.id.is_empty()
                || operation.id.len() > 256
                || !ids.insert(operation.id.clone())
                || operation.label.len() > 256
                || operation
                    .target_id
                    .as_ref()
                    .is_some_and(|id| id.len() > 256)
                || operation
                    .disabled_reason
                    .as_ref()
                    .is_some_and(|reason| reason.len() > 4096)
                || operation
                    .confirmation
                    .as_ref()
                    .is_some_and(|reason| reason.len() > 4096)
                || (operation.action.requires_confirmation()
                    && operation
                        .confirmation
                        .as_ref()
                        .is_none_or(|text| text.trim().is_empty()))
            {
                return Err(ServiceError::Failed("Invalid product operation".into()));
            }
        }
        if self.status.len() > 16_384 {
            return Err(ServiceError::Failed("Invalid product status".into()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Request {
    pub scope: Scope,
    pub generation: u64,
    pub cancel: CancellationToken,
}

#[derive(Clone, Debug)]
pub struct Mutation {
    pub operation: Operation,
    pub fields: Arc<[(Arc<str>, Arc<str>)]>,
}

#[derive(Clone, Debug)]
pub enum Outcome {
    Refresh,
    OpenSession(SessionId),
    IdentityChanged(Scope),
}

/// The host supplies transport-free domain services; no credential belongs in a Panel.
pub trait ProductServices: Send + Sync {
    fn load(&self, surface: Surface, request: Request) -> BoxFuture<'static, Result<Panel>>;
    fn perform(
        &self,
        surface: Surface,
        request: Request,
        mutation: Mutation,
    ) -> BoxFuture<'static, Result<Outcome>>;
}

pub struct UnavailableServices;

impl ProductServices for UnavailableServices {
    fn load(&self, surface: Surface, _: Request) -> BoxFuture<'static, Result<Panel>> {
        Box::pin(async move { Err(ServiceError::Unsupported(surface.missing_service().into())) })
    }

    fn perform(
        &self,
        surface: Surface,
        _: Request,
        _: Mutation,
    ) -> BoxFuture<'static, Result<Outcome>> {
        Box::pin(async move { Err(ServiceError::Unsupported(surface.missing_service().into())) })
    }
}

#[derive(Default)]
pub struct RequestGate {
    generation: u64,
    cancel: CancellationToken,
    busy: bool,
}

impl RequestGate {
    pub fn next(&mut self, scope: Scope) -> Request {
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
        self.generation = self
            .generation
            .checked_add(1)
            .expect("request generation exhausted");
        self.busy = false;
        Request {
            scope,
            generation: self.generation,
            cancel: self.cancel.clone(),
        }
    }

    pub fn begin_mutation(&mut self, scope: Scope) -> Result<Request> {
        if self.busy {
            return Err(ServiceError::Busy);
        }
        let request = self.next(scope);
        self.busy = true;
        Ok(request)
    }

    pub fn accepts(&self, request: &Request, scope: &Scope) -> bool {
        request.generation == self.generation
            && &request.scope == scope
            && !request.cancel.is_cancelled()
    }

    pub fn finish(&mut self, request: &Request, scope: &Scope) -> bool {
        if !self.accepts(request, scope) {
            return false;
        }
        self.busy = false;
        true
    }

    pub fn busy(&self) -> bool {
        self.busy
    }
}

impl Drop for RequestGate {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_change_cancels_work_and_rejects_old_completions() {
        let mut gate = RequestGate::default();
        let old_scope = Scope {
            account_id: Some("alice".into()),
            ..Scope::default()
        };
        let old = gate.begin_mutation(old_scope.clone()).unwrap();
        assert!(matches!(
            gate.begin_mutation(old_scope),
            Err(ServiceError::Busy)
        ));
        let new_scope = Scope {
            account_id: Some("bob".into()),
            ..Scope::default()
        };
        let current = gate.next(new_scope.clone());
        assert!(old.cancel.is_cancelled());
        assert!(!gate.finish(&old, &new_scope));
        assert!(gate.accepts(&current, &new_scope));
    }

    #[test]
    fn mutation_guard_and_cancelled_scope_are_not_success_states() {
        let mut gate = RequestGate::default();
        let request = gate.begin_mutation(Scope::default()).unwrap();
        assert!(gate.busy());
        assert!(gate.finish(&request, &Scope::default()));
        let request = gate.next(Scope::default());
        drop(gate);
        assert!(request.cancel.is_cancelled());
    }

    #[test]
    fn panel_rejects_wrong_scope_duplicate_ids_unbounded_fields_and_unconfirmed_destruction() {
        let mut panel = Panel {
            scope: Scope::default(),
            status: "".into(),
            rows: Arc::from([]),
            fields: Arc::from([]),
            operations: Arc::from([]),
            permissions_ready: false,
        };
        assert!(panel.validate(&Scope::default()).is_ok());
        assert!(matches!(
            panel.validate(&Scope {
                account_id: Some("other".into()),
                ..Scope::default()
            }),
            Err(ServiceError::Conflict)
        ));
        let row = Row {
            id: "duplicate".into(),
            title: "".into(),
            detail: "".into(),
        };
        panel.rows = Arc::from([row.clone(), row]);
        assert!(panel.validate(&Scope::default()).is_err());
        panel.rows = Arc::from([]);
        panel.fields = Arc::from([Field {
            id: "field".into(),
            label: "Field".into(),
            value: "x".repeat(4097).into(),
        }]);
        assert!(panel.validate(&Scope::default()).is_err());
        panel.fields = Arc::from([]);
        panel.operations = Arc::from([Operation {
            id: "delete-library".into(),
            label: "Log out library".into(),
            action: Action::LogoutLibrary,
            target_id: None,
            disabled_reason: None,
            confirmation: Some(" ".into()),
        }]);
        assert!(panel.validate(&Scope::default()).is_err());
    }

    #[test]
    fn unavailable_services_cannot_report_an_operation_success() {
        let services = UnavailableServices;
        let request = RequestGate::default().next(Scope::default());
        let result = futures::executor::block_on(services.perform(
            Surface::Exports,
            request,
            Mutation {
                operation: Operation {
                    id: "export".into(),
                    label: "Export".into(),
                    action: Action::Export,
                    target_id: None,
                    disabled_reason: None,
                    confirmation: None,
                },
                fields: Arc::from([]),
            },
        ));
        assert!(matches!(result, Err(ServiceError::Unsupported(_))));
    }
}
