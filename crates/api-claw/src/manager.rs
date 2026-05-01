use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use hypr_exedev::{
    Exe0Token, ExedevClient, Permissions, ShareVisibility, Vm, VmAuth, VmHttpClient, VmNewArgs,
};
use serde::{Serialize, de::DeserializeOwned};

use crate::Error;
use crate::keyring::{UserKeyring, UserSshKey};
use crate::naming::{UserId, vm_name};

/// Static description of what "a claw VM" looks like for this deployment.
///
/// Everything here is environment-level: image, disk, the port claw binds
/// inside the VM, and a *template* of env vars. Per-user env vars (tokens,
/// user id) are injected by [`ClawManager::provision`].
#[derive(Debug, Clone)]
pub struct ClawProvisionSpec {
    pub image: String,
    pub disk: String,
    /// Internal port the claw HTTP listener binds to. Published via `share port`.
    pub port: u16,
    /// Deployment-wide env applied to every VM (e.g. OPENAI_BASE_URL). Per-user
    /// secrets should be layered in via `per_user_env` on [`ClawManager`].
    pub env: BTreeMap<String, String>,
}

impl Default for ClawProvisionSpec {
    fn default() -> Self {
        Self {
            image: "ghcr.io/boldsoftware/exeuntu:latest".into(),
            disk: "20GB".into(),
            port: 8080,
            env: BTreeMap::new(),
        }
    }
}

/// Shape of the `ctx` JSON embedded in VM-scoped tokens. Signed by exe.dev
/// and forwarded to claw verbatim via `X-ExeDev-Token-Ctx`.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct ClawTokenCtx {
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
}

/// What [`ClawManager::provision`] produced. `Created` means we issued a new
/// VM in this call; `Existed` means it was already present and we short-circuited.
#[derive(Debug, Clone)]
pub enum ProvisionOutcome {
    Created(Vm),
    Existed(Vm),
}

impl ProvisionOutcome {
    pub fn vm(&self) -> &Vm {
        match self {
            ProvisionOutcome::Created(vm) | ProvisionOutcome::Existed(vm) => vm,
        }
    }

    pub fn into_vm(self) -> Vm {
        match self {
            ProvisionOutcome::Created(vm) | ProvisionOutcome::Existed(vm) => vm,
        }
    }
}

/// Per-user env closure. Given a user id, returns the env vars claw needs in
/// that user's VM. Kept separate from the static [`ClawProvisionSpec`] so that
/// secrets can be fetched per-call from a vault without widening the spec type.
pub type PerUserEnv = Box<dyn Fn(&UserId) -> BTreeMap<String, String> + Send + Sync + 'static>;

pub struct ClawManager<K> {
    client: ExedevClient,
    spec: ClawProvisionSpec,
    keyring: K,
    per_user_env: PerUserEnv,
}

impl<K: UserKeyring> ClawManager<K> {
    pub fn new(client: ExedevClient, spec: ClawProvisionSpec, keyring: K) -> Self {
        Self {
            client,
            spec,
            keyring,
            per_user_env: Box::new(|_| BTreeMap::new()),
        }
    }

    /// Install the per-user env closure. Called for every provision and for
    /// every token mint (via `ctx.user_id`), so avoid network calls here if
    /// you can help it.
    pub fn with_per_user_env(mut self, f: PerUserEnv) -> Self {
        self.per_user_env = f;
        self
    }

    pub fn vm_name(&self, user: &UserId) -> String {
        vm_name(user)
    }

    pub fn spec(&self) -> &ClawProvisionSpec {
        &self.spec
    }

    /// Look up the VM for a user. `None` if not yet provisioned.
    pub async fn get(&self, user: &UserId) -> Result<Option<Vm>, Error> {
        let name = vm_name(user);
        Ok(self.client.vm_get(&name).await?)
    }

    /// Idempotent provision.
    ///
    /// Does:
    /// 1. keyring `get_or_create` — either returns an existing keypair or
    ///    generates a fresh one;
    /// 2. ensures the public key is registered on the exe.dev account
    ///    (unique per user — `remove` on deprovision revokes every token
    ///    ever minted for this user without touching other users);
    /// 3. if `ls <name>` already has a running/starting VM, returns `Existed`;
    /// 4. otherwise `new --name=<name> ...` with the spec + per-user env;
    /// 5. sets the proxy port and forces it private (bearer-token access).
    pub async fn provision(&self, user: &UserId) -> Result<ProvisionOutcome, Error> {
        let name = vm_name(user);
        let key = self.keyring.get_or_create(user).await?;
        self.ensure_ssh_key_registered(&key, user).await?;

        if let Some(existing) = self.client.vm_get(&name).await? {
            self.configure_proxy(&name).await?;
            return Ok(ProvisionOutcome::Existed(existing));
        }

        let args = self.build_new_args(user, &name);
        self.client.vm_new(args).await?;

        // Poll once via vm_get; `new` returns once the VM is booked but we
        // still want a strongly-typed record.
        let vm = self.client.vm_get(&name).await?.ok_or_else(|| {
            hypr_exedev::Error::NotFound(format!("vm {name} not found after new"))
        })?;

        self.configure_proxy(&name).await?;
        Ok(ProvisionOutcome::Created(vm))
    }

    /// Revoke all tokens minted for a user without destroying the VM.
    ///
    /// Implementation: remove the user's SSH key from the exe.dev account.
    /// Every exe0/exe1 token derived from it stops validating immediately.
    /// Call [`ClawManager::resume`] to reinstate.
    pub async fn suspend(&self, user: &UserId) -> Result<(), Error> {
        let key = self.keyring.get_or_create(user).await?;
        // `ssh-key remove` accepts name | fingerprint | public-key.
        // We use the fingerprint since the key may have been renamed upstream.
        match self.client.ssh_key_remove(&key.fingerprint).await {
            Ok(()) => Ok(()),
            // Ignore "not found" — already suspended is fine.
            Err(hypr_exedev::Error::NotFound(_)) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Reverse of [`ClawManager::suspend`]: re-registers the user's key.
    pub async fn resume(&self, user: &UserId) -> Result<(), Error> {
        let key = self.keyring.get_or_create(user).await?;
        self.ensure_ssh_key_registered(&key, user).await
    }

    /// Delete the VM and purge the user's keypair.
    pub async fn deprovision(&self, user: &UserId) -> Result<(), Error> {
        let name = vm_name(user);
        if self.client.vm_get(&name).await?.is_some() {
            self.client.vm_remove(&[&name]).await?;
        }
        let key = self.keyring.get_or_create(user).await?;
        let _ = self.client.ssh_key_remove(&key.fingerprint).await;
        self.keyring.remove(user).await?;
        Ok(())
    }

    /// Mint a VM-scoped exe0 token for this user. Signed with the user's key,
    /// so revoking the key revokes the token.
    pub fn mint_vm_token(
        &self,
        user: &UserId,
        key: &UserSshKey,
        exp: DateTime<Utc>,
        extra_ctx: Option<serde_json::Value>,
    ) -> Result<Exe0Token, Error> {
        let name = vm_name(user);
        let ctx = ClawTokenCtx {
            user_id: user.to_string(),
            tenant: None,
            scopes: vec![],
        };
        let mut ctx_json = serde_json::to_value(&ctx)?;
        if let Some(extra) = extra_ctx
            && let (Some(ctx_map), Some(extra_map)) = (ctx_json.as_object_mut(), extra.as_object())
        {
            for (k, v) in extra_map {
                ctx_map.insert(k.clone(), v.clone());
            }
        }
        let perms = Permissions::default()
            .with_exp(exp.timestamp())
            .with_ctx(&ctx_json)?
            .to_json()?;
        Ok(Exe0Token::mint_for_vm(&name, &perms, &key.private_pem)?)
    }

    /// Build an HTTPS client pointed at a user's VM, authed with a short-lived
    /// VM-scoped exe0 token. For repeated calls, reuse the returned client.
    pub async fn http_client(
        &self,
        user: &UserId,
        opts: ClawCallOptions,
    ) -> Result<VmHttpClient, Error> {
        let key = self.keyring.get_or_create(user).await?;
        let token = self.mint_vm_token(user, &key, opts.exp, opts.extra_ctx)?;
        Ok(VmHttpClient::for_vm(
            &vm_name(user),
            VmAuth::Bearer(token.into_string()),
        )?)
    }

    /// One-shot typed HTTP call against a user's VM.
    pub async fn call<Req: Serialize, Res: DeserializeOwned>(
        &self,
        user: &UserId,
        path: &str,
        body: &Req,
        opts: ClawCallOptions,
    ) -> Result<Res, Error> {
        let http = self.http_client(user, opts).await?;
        Ok(http.post_json_for(path, body).await?)
    }

    // --- internals ---

    fn build_new_args(&self, user: &UserId, name: &str) -> VmNewArgs {
        let mut args = VmNewArgs::new()
            .name(name)
            .image(&self.spec.image)
            .disk(&self.spec.disk)
            .no_email(true);
        for (k, v) in &self.spec.env {
            args = args.env(k, v);
        }
        for (k, v) in (self.per_user_env)(user) {
            args = args.env(k, v);
        }
        args
    }

    async fn ensure_ssh_key_registered(
        &self,
        key: &UserSshKey,
        _user: &UserId,
    ) -> Result<(), Error> {
        let existing = self.client.ssh_key_list().await?;
        if existing
            .iter()
            .any(|k| k.fingerprint == key.fingerprint || k.public_key == key.public_openssh)
        {
            return Ok(());
        }
        self.client.ssh_key_add(&key.public_openssh).await?;
        Ok(())
    }

    async fn configure_proxy(&self, name: &str) -> Result<(), Error> {
        self.client.share_port(name, self.spec.port).await?;
        self.client
            .share_visibility(name, ShareVisibility::Private)
            .await?;
        Ok(())
    }
}

/// Options for a single VM call. `exp` is an absolute time, not a duration,
/// so callers are explicit about token lifetime.
#[derive(Debug, Clone)]
pub struct ClawCallOptions {
    pub exp: DateTime<Utc>,
    pub extra_ctx: Option<serde_json::Value>,
}

impl ClawCallOptions {
    /// Token valid for `seconds` from now. Use short values: the token is
    /// re-minted every time `call` runs, so minutes are usually plenty.
    pub fn ttl_seconds(seconds: i64) -> Self {
        Self {
            exp: Utc::now() + chrono::Duration::seconds(seconds),
            extra_ctx: None,
        }
    }

    pub fn with_ctx(mut self, ctx: serde_json::Value) -> Self {
        self.extra_ctx = Some(ctx);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyring::InMemoryKeyring;

    fn noop_manager() -> ClawManager<InMemoryKeyring> {
        let client = ExedevClient::builder().token("exe1.dummy").build().unwrap();
        ClawManager::new(client, ClawProvisionSpec::default(), InMemoryKeyring::new())
    }

    #[test]
    fn vm_name_matches_naming_module() {
        let m = noop_manager();
        let u = UserId::new("u1");
        assert_eq!(m.vm_name(&u), vm_name(&u));
    }

    #[tokio::test]
    async fn mint_vm_token_includes_user_in_ctx() {
        let m = noop_manager();
        let user = UserId::new("user-42");
        let key = m.keyring.get_or_create(&user).await.unwrap();
        let exp = Utc::now() + chrono::Duration::minutes(5);
        let token = m.mint_vm_token(&user, &key, exp, None).unwrap();

        let parts: Vec<&str> = token.as_str().split('.').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "exe0");

        use base64::Engine;
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(parts[1])
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(v["ctx"]["user_id"], "user-42");
    }

    #[tokio::test]
    async fn mint_vm_token_merges_extra_ctx() {
        let m = noop_manager();
        let user = UserId::new("u");
        let key = m.keyring.get_or_create(&user).await.unwrap();
        let token = m
            .mint_vm_token(
                &user,
                &key,
                Utc::now() + chrono::Duration::minutes(5),
                Some(serde_json::json!({ "session": "abc" })),
            )
            .unwrap();

        use base64::Engine;
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(token.as_str().split('.').nth(1).unwrap())
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(v["ctx"]["user_id"], "u");
        assert_eq!(v["ctx"]["session"], "abc");
    }

    #[tokio::test]
    async fn build_new_args_injects_spec_and_per_user_env() {
        let client = ExedevClient::builder().token("t").build().unwrap();
        let spec = ClawProvisionSpec {
            image: "img".into(),
            disk: "20GB".into(),
            port: 8080,
            env: BTreeMap::from([("GLOBAL".into(), "g".into())]),
        };
        let m = ClawManager::new(client, spec, InMemoryKeyring::new()).with_per_user_env(Box::new(
            |u: &UserId| BTreeMap::from([("USER_ID".into(), u.to_string())]),
        ));
        let user = UserId::new("bob");
        let args = m.build_new_args(&user, "claw-abc");
        assert_eq!(args.name.as_deref(), Some("claw-abc"));
        assert_eq!(args.image.as_deref(), Some("img"));
        assert_eq!(args.disk.as_deref(), Some("20GB"));
        assert!(args.env.contains(&("GLOBAL".into(), "g".into())));
        assert!(args.env.contains(&("USER_ID".into(), "bob".into())));
        assert!(args.no_email);
    }
}
