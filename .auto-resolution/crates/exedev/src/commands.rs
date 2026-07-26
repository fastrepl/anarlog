use serde::{Deserialize, Serialize};
use shell_escape::unix::escape;

use crate::client::ExedevClient;
use crate::models::{GeneratedApiKey, SshKey, SshKeyList, Vm, VmList, VmStat, WhoAmI};
use crate::token::Exe1Token;

/// Arguments for `new`. Build with the chainable setters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VmNewArgs {
    pub name: Option<String>,
    pub image: Option<String>,
    pub disk: Option<String>,
    pub command: Option<String>,
    pub integrations: Vec<String>,
    pub env: Vec<(String, String)>,
    pub no_email: bool,
}

impl VmNewArgs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn image(mut self, image: impl Into<String>) -> Self {
        self.image = Some(image.into());
        self
    }

    pub fn disk(mut self, disk: impl Into<String>) -> Self {
        self.disk = Some(disk.into());
        self
    }

    pub fn command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }

    pub fn integration(mut self, integration: impl Into<String>) -> Self {
        self.integrations.push(integration.into());
        self
    }

    pub fn env(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.env.push((k.into(), v.into()));
        self
    }

    pub fn envs<I, K, V>(mut self, vars: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        for (k, v) in vars {
            self.env.push((k.into(), v.into()));
        }
        self
    }

    pub fn no_email(mut self, yes: bool) -> Self {
        self.no_email = yes;
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VmResizeSpec {
    pub disk: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VmCopySpec {
    pub new_name: Option<String>,
    pub copy_tags: Option<bool>,
    pub disk: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareVisibility {
    Public,
    Private,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GenerateApiKeyArgs {
    pub label: Option<String>,
    pub vm: Option<String>,
    pub cmds: Vec<String>,
    pub exp: Option<String>,
}

fn quote(s: &str) -> String {
    escape(std::borrow::Cow::Borrowed(s)).into_owned()
}

fn validate_vm_name(name: &str) -> Result<(), crate::Error> {
    if name.is_empty() {
        return Err(crate::Error::InvalidVmName("empty".into()));
    }
    if name
        .chars()
        .any(|c| !(c.is_ascii_alphanumeric() || c == '-'))
        || name.starts_with('-')
        || name.ends_with('-')
    {
        return Err(crate::Error::InvalidVmName(name.to_string()));
    }
    Ok(())
}

impl ExedevClient {
    // ===== VM lifecycle =====

    pub async fn vm_list(&self) -> Result<Vec<Vm>, crate::Error> {
        let list: VmList = self.exec_json("ls -l -a --json").await?;
        Ok(list.vms)
    }

    /// List VMs matching a name or pattern. exe.dev's `ls` accepts either.
    pub async fn vm_list_matching(&self, name_or_pattern: &str) -> Result<Vec<Vm>, crate::Error> {
        let cmd = format!("ls {} -l -a --json", quote(name_or_pattern));
        let list: VmList = self.exec_json(&cmd).await?;
        Ok(list.vms)
    }

    /// Look up a VM by exact name. Returns `None` if not found.
    pub async fn vm_get(&self, name: &str) -> Result<Option<Vm>, crate::Error> {
        validate_vm_name(name)?;
        let vms = self.vm_list_matching(name).await?;
        Ok(vms.into_iter().find(|v| v.name == name))
    }

    /// Create a VM. Returns the parsed `new --json` response.
    ///
    /// To get a strongly-typed `Vm`, follow up with `vm_get(name)` once the
    /// VM reaches `running`.
    pub async fn vm_new(&self, args: VmNewArgs) -> Result<serde_json::Value, crate::Error> {
        let cmd = vm_new_command(&args)?;
        self.exec_json(&cmd).await
    }

    /// Create a VM and block on a subsequent `ls` lookup to return the typed record.
    ///
    /// Requires that `args.name` is set — otherwise we can't look the VM up
    /// deterministically.
    pub async fn vm_new_typed(&self, args: VmNewArgs) -> Result<Vm, crate::Error> {
        let name = args.name.clone().ok_or_else(|| {
            crate::Error::InvalidVmName("name is required for vm_new_typed".into())
        })?;
        self.vm_new(args).await?;
        self.vm_get(&name)
            .await?
            .ok_or_else(|| crate::Error::NotFound(format!("vm {name} not found after create")))
    }

    pub async fn vm_remove(&self, names: &[&str]) -> Result<(), crate::Error> {
        if names.is_empty() {
            return Ok(());
        }
        let mut cmd = String::from("rm");
        for n in names {
            validate_vm_name(n)?;
            cmd.push(' ');
            cmd.push_str(&quote(n));
        }
        self.exec_raw(&cmd).await.map(|_| ())
    }

    pub async fn vm_restart(&self, name: &str) -> Result<serde_json::Value, crate::Error> {
        validate_vm_name(name)?;
        let cmd = format!("restart {} --json", quote(name));
        self.exec_json(&cmd).await
    }

    pub async fn vm_rename(&self, old: &str, new: &str) -> Result<(), crate::Error> {
        validate_vm_name(old)?;
        validate_vm_name(new)?;
        let cmd = format!("rename {} {}", quote(old), quote(new));
        self.exec_raw(&cmd).await.map(|_| ())
    }

    pub async fn vm_tag_add(&self, name: &str, tag: &str) -> Result<(), crate::Error> {
        validate_vm_name(name)?;
        let cmd = format!("tag {} {}", quote(name), quote(tag));
        self.exec_raw(&cmd).await.map(|_| ())
    }

    pub async fn vm_tag_remove(&self, name: &str, tag: &str) -> Result<(), crate::Error> {
        validate_vm_name(name)?;
        let cmd = format!("tag -d {} {}", quote(name), quote(tag));
        self.exec_raw(&cmd).await.map(|_| ())
    }

    pub async fn vm_resize(
        &self,
        name: &str,
        spec: VmResizeSpec,
    ) -> Result<serde_json::Value, crate::Error> {
        validate_vm_name(name)?;
        let mut cmd = format!("resize {}", quote(name));
        if let Some(disk) = &spec.disk {
            cmd.push_str(&format!(" --disk {}", quote(disk)));
        }
        cmd.push_str(" --json");
        self.exec_json(&cmd).await
    }

    pub async fn vm_copy(&self, source: &str, spec: VmCopySpec) -> Result<Vm, crate::Error> {
        validate_vm_name(source)?;
        if let Some(n) = &spec.new_name {
            validate_vm_name(n)?;
        }
        let mut cmd = format!("cp {}", quote(source));
        if let Some(new_name) = &spec.new_name {
            cmd.push(' ');
            cmd.push_str(&quote(new_name));
        }
        if let Some(copy_tags) = spec.copy_tags {
            cmd.push_str(&format!(" --copy-tags={copy_tags}"));
        }
        if let Some(disk) = &spec.disk {
            cmd.push_str(&format!(" --disk {}", quote(disk)));
        }
        cmd.push_str(" --json");
        let value: serde_json::Value = self.exec_json(&cmd).await?;
        if let Ok(vm) = serde_json::from_value::<Vm>(value.clone()) {
            return Ok(vm);
        }
        if let Some(name) = spec.new_name.as_deref()
            && let Some(vm) = self.vm_get(name).await?
        {
            return Ok(vm);
        }
        Err(crate::Error::NotFound(format!(
            "cp response did not include vm record: {value}"
        )))
    }

    pub async fn vm_stat(&self, name: &str) -> Result<VmStat, crate::Error> {
        validate_vm_name(name)?;
        let cmd = format!("stat {}", quote(name));
        self.exec_json(&cmd).await
    }

    // ===== Sharing / proxy =====

    pub async fn share_port(&self, name: &str, port: u16) -> Result<(), crate::Error> {
        validate_vm_name(name)?;
        let cmd = format!("share port {} {port}", quote(name));
        self.exec_raw(&cmd).await.map(|_| ())
    }

    pub async fn share_visibility(
        &self,
        name: &str,
        visibility: ShareVisibility,
    ) -> Result<(), crate::Error> {
        validate_vm_name(name)?;
        let verb = match visibility {
            ShareVisibility::Public => "set-public",
            ShareVisibility::Private => "set-private",
        };
        let cmd = format!("share {verb} {}", quote(name));
        self.exec_raw(&cmd).await.map(|_| ())
    }

    pub async fn share_show(&self, name: &str) -> Result<serde_json::Value, crate::Error> {
        validate_vm_name(name)?;
        let cmd = format!("share show {} --json", quote(name));
        self.exec_json(&cmd).await
    }

    // ===== SSH keys =====

    pub async fn ssh_key_list(&self) -> Result<Vec<SshKey>, crate::Error> {
        let list: SshKeyList = self.exec_json("ssh-key list --json").await?;
        Ok(list.ssh_keys)
    }

    pub async fn ssh_key_add(&self, public_key: &str) -> Result<(), crate::Error> {
        let cmd = format!("ssh-key add {}", quote(public_key));
        self.exec_raw(&cmd).await.map(|_| ())
    }

    pub async fn ssh_key_remove(&self, name_or_fingerprint: &str) -> Result<(), crate::Error> {
        let cmd = format!("ssh-key remove {}", quote(name_or_fingerprint));
        self.exec_raw(&cmd).await.map(|_| ())
    }

    pub async fn ssh_key_rename(&self, old: &str, new: &str) -> Result<(), crate::Error> {
        let cmd = format!("ssh-key rename {} {}", quote(old), quote(new));
        self.exec_raw(&cmd).await.map(|_| ())
    }

    /// Thin wrapper around `ssh-key generate-api-key`. The server generates a
    /// key and returns a token; the exact response shape is surfaced through
    /// `GeneratedApiKey::extra` as well as the typed `token` field.
    pub async fn ssh_key_generate_api_key(
        &self,
        args: GenerateApiKeyArgs,
    ) -> Result<GeneratedApiKey, crate::Error> {
        let mut cmd = String::from("ssh-key generate-api-key");
        if let Some(label) = &args.label {
            cmd.push_str(&format!(" --label={}", quote(label)));
        }
        if let Some(vm) = &args.vm {
            validate_vm_name(vm)?;
            cmd.push_str(&format!(" --vm={}", quote(vm)));
        }
        if !args.cmds.is_empty() {
            cmd.push_str(&format!(" --cmds={}", quote(&args.cmds.join(","))));
        }
        if let Some(exp) = &args.exp {
            cmd.push_str(&format!(" --exp={}", quote(exp)));
        }
        cmd.push_str(" --json");
        self.exec_json(&cmd).await
    }

    // ===== Identity / utility =====

    pub async fn whoami(&self) -> Result<WhoAmI, crate::Error> {
        self.exec_json("whoami --json").await
    }

    pub async fn help(&self) -> Result<serde_json::Value, crate::Error> {
        self.exec_json("help --json").await
    }

    pub async fn set_region(&self, region: &str) -> Result<(), crate::Error> {
        let cmd = format!("set-region {}", quote(region));
        self.exec_raw(&cmd).await.map(|_| ())
    }

    /// Convert an exe0 token to a shorter exe1 handle.
    ///
    /// Pass `Some(vm)` if the exe0 token is VM-scoped.
    pub async fn exe0_to_exe1(
        &self,
        exe0: &str,
        vm: Option<&str>,
    ) -> Result<Exe1Token, crate::Error> {
        let mut cmd = String::from("exe0-to-exe1");
        if let Some(vm) = vm {
            validate_vm_name(vm)?;
            cmd.push_str(&format!(" --vm={}", quote(vm)));
        }
        cmd.push(' ');
        cmd.push_str(&quote(exe0));
        let text = self.exec_raw(&cmd).await?;
        Ok(Exe1Token::new(text.trim().to_string()))
    }
}

fn vm_new_command(args: &VmNewArgs) -> Result<String, crate::Error> {
    let mut cmd = String::from("new");
    if let Some(name) = &args.name {
        validate_vm_name(name)?;
        cmd.push_str(&format!(" --name {}", quote(name)));
    }
    if let Some(image) = &args.image {
        cmd.push_str(&format!(" --image {}", quote(image)));
    }
    if let Some(disk) = &args.disk {
        cmd.push_str(&format!(" --disk {}", quote(disk)));
    }
    if let Some(command) = &args.command {
        cmd.push_str(&format!(" --command {}", quote(command)));
    }
    for integration in &args.integrations {
        cmd.push_str(&format!(" --integration {}", quote(integration)));
    }
    for (k, v) in &args.env {
        cmd.push_str(&format!(" --env {}", quote(&format!("{k}={v}"))));
    }
    if args.no_email {
        cmd.push_str(" --no-email");
    }
    cmd.push_str(" --json");
    Ok(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_handles_spaces_and_quotes() {
        assert_eq!(quote("simple"), "simple");
        let q = quote("with space");
        assert!(q.contains(' '));
        assert!(q.starts_with('\'') || q.starts_with('"'));
        let q = quote("it's");
        assert!(q.contains("it"));
    }

    #[test]
    fn rejects_bad_vm_names() {
        assert!(validate_vm_name("").is_err());
        assert!(validate_vm_name("-foo").is_err());
        assert!(validate_vm_name("foo-").is_err());
        assert!(validate_vm_name("foo bar").is_err());
        assert!(validate_vm_name("foo_bar").is_err());
        assert!(validate_vm_name("foo.bar").is_err());
        assert!(validate_vm_name("claw-ab12cd34").is_ok());
        assert!(validate_vm_name("a").is_ok());
    }

    #[test]
    fn vm_new_command_with_everything() {
        let args = VmNewArgs::new()
            .name("claw-ab12")
            .image("ghcr.io/fastrepl/char-claw:latest")
            .disk("20GB")
            .env("OPENAI_API_KEY", "sk with space")
            .env("FOO", "bar")
            .integration("proxy")
            .no_email(true);
        let cmd = vm_new_command(&args).unwrap();
        assert!(cmd.starts_with("new "));
        assert!(cmd.contains("--name claw-ab12"), "cmd: {cmd}");
        assert!(
            cmd.contains("ghcr.io/fastrepl/char-claw:latest"),
            "cmd: {cmd}"
        );
        assert!(cmd.contains("--disk 20GB"), "cmd: {cmd}");
        assert!(cmd.contains("--integration proxy"), "cmd: {cmd}");
        assert!(cmd.contains("OPENAI_API_KEY=sk with space"), "cmd: {cmd}");
        assert!(cmd.contains("FOO=bar"), "cmd: {cmd}");
        assert!(cmd.contains("--no-email"), "cmd: {cmd}");
        assert!(cmd.ends_with(" --json"), "cmd: {cmd}");
    }

    #[test]
    fn vm_new_command_rejects_bad_name() {
        let args = VmNewArgs::new().name("bad name!");
        assert!(vm_new_command(&args).is_err());
    }

    #[test]
    fn vm_list_parses_live_response_shape() {
        let body = r#"{"vms":[{"created_at":"2026-04-15T12:52:12Z","https_url":"https://openclaw-yujong.exe.xyz","image":"boldsoftware/exeuntu","region":"pdx","region_display":"Oregon, USA","shelley_url":"https://openclaw-yujong.shelley.exe.xyz","ssh_dest":"openclaw-yujong.exe.xyz","status":"running","vm_name":"openclaw-yujong"}]}"#;
        let list: VmList = serde_json::from_str(body).unwrap();
        assert_eq!(list.vms.len(), 1);
        let vm = &list.vms[0];
        assert_eq!(vm.name, "openclaw-yujong");
        assert_eq!(vm.image, "boldsoftware/exeuntu");
        assert!(matches!(vm.status, crate::models::VmStatus::Running));
        assert_eq!(
            vm.https_url.as_deref(),
            Some("https://openclaw-yujong.exe.xyz")
        );
    }

    #[test]
    fn whoami_parses_live_response_shape() {
        let body = r#"{"email":"yujonglee.dev@gmail.com","region":"","region_display":"","ssh_keys":[{"public_key":"ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIMRZFrRLmFuyKeAegOReWZSy3Hc+3PUlzPzz+1emJHqP","fingerprint":"SHA256:cOXaFfYsFLGAryWgjY2JFz7IFpqQg6Zu5FOqJmi1tXo","name":"dev","current":false}]}"#;
        let whoami: WhoAmI = serde_json::from_str(body).unwrap();
        assert_eq!(whoami.email, "yujonglee.dev@gmail.com");
        assert_eq!(whoami.ssh_keys.len(), 1);
        assert_eq!(whoami.ssh_keys[0].name, "dev");
    }
}
