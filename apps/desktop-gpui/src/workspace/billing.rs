use super::Workspace;

impl Workspace {
    pub(crate) fn is_pro(&self) -> bool {
        self.auth_service
            .claims()
            .is_some_and(|claims| claims.is_pro())
    }

    pub(crate) fn is_paid(&self) -> bool {
        self.auth_service
            .claims()
            .is_some_and(|claims| claims.is_paid())
    }

    pub(crate) fn billing_ready(&self) -> bool {
        self.auth_service.claims().is_some()
    }
}
