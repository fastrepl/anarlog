use std::{collections::HashMap, sync::Arc};

use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedWorkspace {
    pub workspace_id: Arc<str>,
}

pub trait WorkspaceAuthenticator: Send + Sync {
    fn authenticate(
        &self,
        authorization: Option<&str>,
    ) -> Result<AuthenticatedWorkspace, AuthenticationError>;
}

pub struct StaticTokenAuthenticator {
    workspaces_by_token_hash: HashMap<[u8; 32], Arc<str>>,
}

impl StaticTokenAuthenticator {
    pub fn new(
        workspace_tokens: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, AuthenticationError> {
        let mut workspaces_by_token_hash = HashMap::new();
        for (workspace_id, token) in workspace_tokens {
            let workspace_id: Arc<str> = workspace_id.into();
            if workspaces_by_token_hash
                .insert(hash(&token), workspace_id.clone())
                .is_some()
            {
                return Err(AuthenticationError::DuplicateToken);
            }
        }
        if workspaces_by_token_hash.is_empty() {
            return Err(AuthenticationError::NoCredentials);
        }
        Ok(Self {
            workspaces_by_token_hash,
        })
    }
}

impl WorkspaceAuthenticator for StaticTokenAuthenticator {
    fn authenticate(
        &self,
        authorization: Option<&str>,
    ) -> Result<AuthenticatedWorkspace, AuthenticationError> {
        let token = authorization
            .and_then(|authorization| authorization.strip_prefix("Bearer "))
            .filter(|token| !token.is_empty())
            .ok_or(AuthenticationError::InvalidCredentials)?;
        let workspace_id = self
            .workspaces_by_token_hash
            .get(&hash(token))
            .cloned()
            .ok_or(AuthenticationError::InvalidCredentials)?;
        Ok(AuthenticatedWorkspace { workspace_id })
    }
}

fn hash(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AuthenticationError {
    #[error("workspace credentials are required")]
    NoCredentials,
    #[error("the same bearer token cannot authorize multiple workspaces")]
    DuplicateToken,
    #[error("invalid bearer credentials")]
    InvalidCredentials,
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn authenticates_only_the_configured_token() {
        let authenticator =
            StaticTokenAuthenticator::new([("workspace-a".into(), TOKEN.into())]).unwrap();

        assert_eq!(
            authenticator
                .authenticate(Some(&format!("Bearer {TOKEN}")))
                .unwrap()
                .workspace_id
                .as_ref(),
            "workspace-a"
        );
        assert_eq!(
            authenticator.authenticate(Some("Bearer wrong")),
            Err(AuthenticationError::InvalidCredentials)
        );
    }

    #[test]
    fn rejects_a_token_shared_by_two_workspaces() {
        assert!(matches!(
            StaticTokenAuthenticator::new([
                ("workspace-a".into(), TOKEN.into()),
                ("workspace-b".into(), TOKEN.into()),
            ]),
            Err(AuthenticationError::DuplicateToken)
        ));
    }
}
