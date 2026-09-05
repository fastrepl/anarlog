//! Developers page back end: the embedded CLI (`src-tauri/src/embedded_cli.rs`,
//! Unix paths), agent skills (`src-tauri/src/agent_skills.rs`), the MCP
//! configuration snippet, and webhook endpoints (`plugins/local-api`).

use std::path::{Path, PathBuf};

const MANAGED_CLI_DIR: &str = ".anarlog-cli";
const SKILL_DIR_NAME: &str = "anarlog";

/// The published skill package from `skills/anarlog`, embedded at build time.
const SKILL_FILES: &[(&str, &str)] = &[
    ("SKILL.md", include_str!("../../../skills/anarlog/SKILL.md")),
    (
        "references/cli.md",
        include_str!("../../../skills/anarlog/references/cli.md"),
    ),
    (
        "references/errors.md",
        include_str!("../../../skills/anarlog/references/errors.md"),
    ),
    (
        "references/mcp.md",
        include_str!("../../../skills/anarlog/references/mcp.md"),
    ),
    (
        "references/setup.md",
        include_str!("../../../skills/anarlog/references/setup.md"),
    ),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliState {
    Installed,
    Missing,
    Conflict,
    Unsupported,
    ResourceMissing,
}

#[derive(Clone, Debug)]
pub struct CliStatus {
    pub supported: bool,
    pub command_name: &'static str,
    pub install_path: String,
    pub state: CliState,
    pub details: Option<String>,
}

fn command_name_from_identifier(identifier: &str) -> &'static str {
    match identifier {
        "com.hyprnote.stable" | "com.hyprnote.Hyprnote" | "so.anarlog.Anarlog" => "anarlog",
        "com.hyprnote.staging" => "anarlog-staging",
        "com.hyprnote.dev" => "anarlog-dev",
        _ => "anarlog",
    }
}

fn install_path_for_command(command_name: &str) -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".local/bin").join(command_name))
}

fn bundled_binary_name() -> Option<&'static str> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("anarlog-cli-x86_64-unknown-linux-gnu")
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        Some("anarlog-cli-aarch64-unknown-linux-gnu")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("anarlog-cli-aarch64-apple-darwin")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("anarlog-cli-x86_64-apple-darwin")
    } else {
        None
    }
}

/// `resolve_resource_path`: a sidecar next to the executable, then the
/// bundled `resources/cli/<name>` beside it, then the desktop crate's debug
/// resources.
fn resolve_resource_path() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));
    if let Some(sidecar) = exe_dir
        .as_ref()
        .map(|dir| dir.join("anarlog-cli"))
        .filter(|path| path.is_file())
    {
        return Some(sidecar);
    }
    let file_name = bundled_binary_name()?;
    if let Some(bundled) = exe_dir
        .as_ref()
        .map(|dir| dir.join("resources").join("cli").join(file_name))
        .filter(|path| path.exists())
    {
        return Some(bundled);
    }
    let debug_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../desktop/src-tauri/resources/cli")
        .join(file_name);
    debug_path.exists().then_some(debug_path)
}

fn unavailable(command_name: &'static str, details: &str) -> CliStatus {
    CliStatus {
        supported: false,
        command_name,
        install_path: String::new(),
        state: CliState::Unsupported,
        details: Some(details.to_string()),
    }
}

/// `embedded_cli::check`
pub fn check_cli(identifier: &str, app_version: &str) -> CliStatus {
    let command_name = command_name_from_identifier(identifier);
    let Some(install_path) = install_path_for_command(command_name) else {
        return unavailable(command_name, "Anarlog could not find your home directory.");
    };
    if resolve_resource_path().is_none() {
        return CliStatus {
            supported: true,
            command_name,
            install_path: install_path.display().to_string(),
            state: CliState::ResourceMissing,
            details: Some("The CLI is not included in this build of Anarlog.".to_string()),
        };
    }
    classify_status(command_name, install_path, app_version)
}

/// `embedded_cli::install` (Unix): copy the bundled binary into the managed
/// directory and point `~/.local/bin/<command>` at it.
pub fn install_cli(identifier: &str, app_version: &str) -> Result<CliStatus, String> {
    let status = check_cli(identifier, app_version);
    match status.state {
        CliState::Unsupported | CliState::ResourceMissing => return Ok(status),
        CliState::Conflict => {
            return Err(format!(
                "Another file already exists at {}. Move it before installing the Anarlog CLI.",
                status.install_path
            ));
        }
        CliState::Installed | CliState::Missing => {}
    }
    let resource_path =
        resolve_resource_path().ok_or_else(|| "The bundled CLI could not be found.".to_string())?;
    let install_path = PathBuf::from(&status.install_path);
    let managed_path = managed_binary_path(&install_path, status.command_name, app_version)?;
    install_managed_cli(&resource_path, &managed_path, &install_path)?;
    Ok(classify_status(
        status.command_name,
        install_path,
        app_version,
    ))
}

fn classify_status(
    command_name: &'static str,
    install_path: PathBuf,
    app_version: &str,
) -> CliStatus {
    let state = managed_binary_path(&install_path, command_name, app_version)
        .and_then(|managed_path| classify_installation(&install_path, &managed_path));
    match state {
        Ok(state) => CliStatus {
            supported: true,
            command_name,
            install_path: install_path.display().to_string(),
            state,
            details: details_for_state(state, &install_path),
        },
        Err(error) => CliStatus {
            supported: true,
            command_name,
            install_path: install_path.display().to_string(),
            state: CliState::Conflict,
            details: Some(error),
        },
    }
}

fn details_for_state(state: CliState, install_path: &Path) -> Option<String> {
    match state {
        CliState::Installed => Some(format!(
            "Installed at {} and managed by Anarlog.",
            install_path.display()
        )),
        CliState::Missing => Some(format!(
            "Install the command at {}.",
            install_path.display()
        )),
        CliState::Conflict => Some(format!(
            "Another file already exists at {}.",
            install_path.display()
        )),
        CliState::Unsupported | CliState::ResourceMissing => None,
    }
}

fn managed_binary_path(
    install_path: &Path,
    command_name: &str,
    app_version: &str,
) -> Result<PathBuf, String> {
    let install_dir = install_path
        .parent()
        .ok_or_else(|| "The CLI install directory is invalid.".to_string())?;
    Ok(install_dir
        .join(MANAGED_CLI_DIR)
        .join(command_name)
        .join(app_version))
}

#[cfg(unix)]
fn classify_installation(install_path: &Path, managed_path: &Path) -> Result<CliState, String> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = match std::fs::symlink_metadata(install_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(CliState::Missing),
        Err(error) => {
            return Err(format!(
                "Failed to inspect {}: {error}",
                install_path.display()
            ));
        }
    };
    if !metadata.file_type().is_symlink() {
        return Ok(CliState::Conflict);
    }
    let installed_target = std::fs::read_link(install_path).map_err(|error| {
        format!(
            "Failed to inspect the installed command at {}: {error}",
            install_path.display()
        )
    })?;
    if !is_replaceable_symlink_target(&installed_target, managed_path) {
        return Ok(CliState::Conflict);
    }
    if installed_target != managed_path {
        return Ok(CliState::Missing);
    }
    let managed_metadata = match std::fs::symlink_metadata(managed_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(CliState::Missing),
        Err(error) => {
            return Err(format!(
                "Failed to resolve the managed CLI at {}: {error}",
                managed_path.display()
            ));
        }
    };
    if !managed_metadata.file_type().is_file() || managed_metadata.permissions().mode() & 0o100 == 0
    {
        return Ok(CliState::Missing);
    }
    Ok(CliState::Installed)
}

#[cfg(not(unix))]
fn classify_installation(_install_path: &Path, _managed_path: &Path) -> Result<CliState, String> {
    Ok(CliState::Missing)
}

fn is_replaceable_symlink_target(target: &Path, managed_path: &Path) -> bool {
    managed_path
        .parent()
        .is_some_and(|managed_dir| target.parent() == Some(managed_dir))
}

#[cfg(unix)]
fn install_managed_cli(
    resource_path: &Path,
    managed_path: &Path,
    install_path: &Path,
) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let managed_dir = managed_path
        .parent()
        .ok_or_else(|| "The managed CLI directory is invalid.".to_string())?;
    std::fs::create_dir_all(managed_dir)
        .map_err(|error| format!("Could not create {}: {error}", managed_dir.display()))?;
    let file_name = managed_path
        .file_name()
        .ok_or_else(|| "The managed CLI path is invalid.".to_string())?;
    let temp_path = managed_path.with_file_name(format!(
        ".{}.tmp-{}",
        file_name.to_string_lossy(),
        std::process::id()
    ));
    if std::fs::symlink_metadata(&temp_path).is_ok() {
        std::fs::remove_file(&temp_path).map_err(|error| {
            format!(
                "Could not prepare the CLI update at {}: {error}",
                temp_path.display()
            )
        })?;
    }
    std::fs::copy(resource_path, &temp_path).map_err(|error| {
        format!(
            "Could not copy the bundled CLI to {}: {error}",
            temp_path.display()
        )
    })?;
    let mut permissions = std::fs::metadata(&temp_path)
        .map_err(|error| {
            format!(
                "Could not inspect the CLI update at {}: {error}",
                temp_path.display()
            )
        })?
        .permissions();
    permissions.set_mode(permissions.mode() | 0o100);
    if let Err(error) = std::fs::set_permissions(&temp_path, permissions) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!(
            "Could not make the CLI executable at {}: {error}",
            temp_path.display()
        ));
    }
    if let Err(error) = std::fs::rename(&temp_path, managed_path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!(
            "Could not install the managed CLI at {}: {error}",
            managed_path.display()
        ));
    }
    install_symlink(managed_path, install_path)
}

#[cfg(not(unix))]
fn install_managed_cli(
    _resource_path: &Path,
    _managed_path: &Path,
    _install_path: &Path,
) -> Result<(), String> {
    Err("Bundled CLI installation is not yet available on this platform.".to_string())
}

#[cfg(unix)]
fn install_symlink(managed_path: &Path, install_path: &Path) -> Result<(), String> {
    let install_dir = install_path
        .parent()
        .ok_or_else(|| "The CLI install directory is invalid.".to_string())?;
    std::fs::create_dir_all(install_dir)
        .map_err(|error| format!("Could not create {}: {error}", install_dir.display()))?;
    let file_name = install_path
        .file_name()
        .ok_or_else(|| "The CLI install path is invalid.".to_string())?;
    let temp_path = install_path.with_file_name(format!(
        ".{}.tmp-{}",
        file_name.to_string_lossy(),
        std::process::id()
    ));
    if std::fs::symlink_metadata(&temp_path).is_ok() {
        std::fs::remove_file(&temp_path).map_err(|error| {
            format!(
                "Could not prepare the command update at {}: {error}",
                temp_path.display()
            )
        })?;
    }
    std::os::unix::fs::symlink(managed_path, &temp_path).map_err(|error| {
        format!(
            "Could not prepare the command at {}: {error}",
            temp_path.display()
        )
    })?;
    if let Err(error) = ensure_install_path_replaceable(install_path, managed_path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&temp_path, install_path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!(
            "Could not install the command at {}: {error}",
            install_path.display()
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn ensure_install_path_replaceable(install_path: &Path, managed_path: &Path) -> Result<(), String> {
    let metadata = match std::fs::symlink_metadata(install_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "Failed to inspect {}: {error}",
                install_path.display()
            ));
        }
    };
    if metadata.file_type().is_symlink() {
        let target = std::fs::read_link(install_path).map_err(|error| {
            format!(
                "Failed to inspect the installed command at {}: {error}",
                install_path.display()
            )
        })?;
        if is_replaceable_symlink_target(&target, managed_path) {
            return Ok(());
        }
    }
    Err(format!(
        "Another file already exists at {}.",
        install_path.display()
    ))
}

/// `buildMcpConfiguration`
pub fn mcp_configuration(command: &str) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": {
            "anarlog": {
                "command": command,
                "args": ["mcp"],
            }
        }
    }))
    .expect("static JSON")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillAgent {
    ClaudeCode,
    Codex,
    Cursor,
    Opencode,
}

pub const SKILL_AGENTS: [SkillAgent; 4] = [
    SkillAgent::ClaudeCode,
    SkillAgent::Codex,
    SkillAgent::Cursor,
    SkillAgent::Opencode,
];

impl SkillAgent {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
            Self::Cursor => "Cursor",
            Self::Opencode => "OpenCode",
        }
    }

    /// The config directory doubles as the detection signal.
    fn config_dir(self, home: &Path) -> PathBuf {
        match self {
            Self::ClaudeCode => home.join(".claude"),
            Self::Codex => home.join(".codex"),
            Self::Cursor => home.join(".cursor"),
            Self::Opencode => std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .unwrap_or_else(|| home.join(".config"))
                .join("opencode"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SkillAgentStatus {
    pub agent: SkillAgent,
    pub detected: bool,
    pub installed: bool,
}

fn home_dir() -> Result<PathBuf, String> {
    dirs::home_dir().ok_or_else(|| "Anarlog could not find your home directory.".to_string())
}

fn skill_dir(agent: SkillAgent, home: &Path) -> PathBuf {
    agent.config_dir(home).join("skills").join(SKILL_DIR_NAME)
}

fn skill_status(agent: SkillAgent, home: &Path) -> SkillAgentStatus {
    SkillAgentStatus {
        agent,
        detected: agent.config_dir(home).is_dir(),
        installed: skill_dir(agent, home).join("SKILL.md").is_file(),
    }
}

/// `agent_skills::list`
pub fn list_skill_agents() -> Result<Vec<SkillAgentStatus>, String> {
    let home = home_dir()?;
    Ok(SKILL_AGENTS
        .into_iter()
        .map(|agent| skill_status(agent, &home))
        .collect())
}

/// `agent_skills::install`
pub fn install_skill(agent: SkillAgent) -> Result<SkillAgentStatus, String> {
    let home = home_dir()?;
    if !agent.config_dir(&home).is_dir() {
        return Err(format!(
            "{} was not found on this machine.",
            agent.display_name()
        ));
    }
    let dir = skill_dir(agent, &home);
    for (relative_path, content) in SKILL_FILES {
        let target = dir.join(relative_path);
        let parent = target
            .parent()
            .ok_or_else(|| "The skill install path is invalid.".to_string())?;
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
        std::fs::write(&target, content)
            .map_err(|error| format!("Could not write {}: {error}", target.display()))?;
    }
    Ok(skill_status(agent, &home))
}

/// `WebhookInfo` from `plugins/local-api`.
#[derive(Debug, Clone)]
pub struct Webhook {
    pub id: String,
    pub url: String,
    pub events: Vec<String>,
    pub active: bool,
    pub last_delivery_at: Option<String>,
    pub last_delivery_status: String,
}

impl From<anlg_db_app::WebhookEndpointRow> for Webhook {
    fn from(row: anlg_db_app::WebhookEndpointRow) -> Self {
        Self {
            id: row.id,
            url: row.url,
            events: serde_json::from_str(&row.events_json).unwrap_or_default(),
            active: row.active,
            last_delivery_at: row.last_delivery_at,
            last_delivery_status: row.last_delivery_status,
        }
    }
}

pub const MAX_WEBHOOK_ENDPOINTS: usize = 64;
const DELIVERY_TIMEOUT_SECS: u64 = 10;
const MAX_STATUS_LEN: usize = 200;

/// `dispatch::create_endpoint` for the settings form (no event filter).
pub async fn create_webhook(
    pool: &sqlx::SqlitePool,
    url: &str,
) -> Result<(Webhook, String), String> {
    let url = url.trim();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("url must start with http:// or https://".to_string());
    }
    let count = anlg_db_app::list_webhook_endpoints(pool)
        .await
        .map_err(|error| error.to_string())?
        .len();
    if count >= MAX_WEBHOOK_ENDPOINTS {
        return Err(format!(
            "at most {MAX_WEBHOOK_ENDPOINTS} webhook endpoints can be configured"
        ));
    }
    let secret = anlg_db_app::generate_webhook_secret();
    let row = anlg_db_app::insert_webhook_endpoint(
        pool,
        &uuid::Uuid::new_v4().to_string(),
        url,
        &secret,
        "[]",
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok((Webhook::from(row), secret))
}

fn sign_payload(secret: &str, body: &[u8]) -> String {
    use hmac::{Hmac, KeyInit, Mac};
    let mut mac = <Hmac<sha2::Sha256> as KeyInit>::new_from_slice(secret.as_bytes())
        .expect("hmac accepts keys of any length");
    mac.update(body);
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// `dispatch::send_test`: one signed `webhook.test` delivery, recorded on the
/// endpoint.
pub async fn send_test_webhook(
    pool: &sqlx::SqlitePool,
    endpoint: &anlg_db_app::WebhookEndpointRow,
) -> Result<(bool, String), String> {
    let body = serde_json::json!({
        "id": format!("evt_{}", uuid::Uuid::new_v4().simple()),
        "event": "webhook.test",
        "created_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "data": { "message": "This is a test delivery from Anarlog." },
    })
    .to_string();
    let delivery_id = format!("dlv_{}", uuid::Uuid::new_v4().simple());
    let (delivered, status) = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(DELIVERY_TIMEOUT_SECS))
        .build()
    {
        Ok(client) => {
            let response = client
                .post(&endpoint.url)
                .header("content-type", "application/json")
                .header("x-anarlog-event", "webhook.test")
                .header("x-anarlog-delivery", delivery_id)
                .header(
                    "x-anarlog-timestamp",
                    chrono::Utc::now().timestamp().to_string(),
                )
                .header(
                    "x-anarlog-signature",
                    format!("sha256={}", sign_payload(&endpoint.secret, body.as_bytes())),
                )
                .body(body)
                .send()
                .await;
            match response {
                Ok(response) => (
                    response.status().is_success(),
                    response.status().to_string(),
                ),
                Err(error) => (false, format!("error: {error}")),
            }
        }
        Err(error) => (false, format!("error: {error}")),
    };
    let status: String = status.chars().take(MAX_STATUS_LEN).collect();
    if let Err(error) = anlg_db_app::record_webhook_delivery(pool, &endpoint.id, &status).await {
        tracing::warn!("failed to record webhook delivery: {error}");
    }
    Ok((delivered, status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_configuration_matches_the_app() {
        assert_eq!(
            mcp_configuration("anarlog"),
            "{\n  \"mcpServers\": {\n    \"anarlog\": {\n      \"command\": \"anarlog\",\n      \"args\": [\n        \"mcp\"\n      ]\n    }\n  }\n}"
        );
    }

    #[test]
    fn command_names_follow_the_bundle_identifier() {
        assert_eq!(
            command_name_from_identifier("com.hyprnote.dev"),
            "anarlog-dev"
        );
        assert_eq!(
            command_name_from_identifier("com.hyprnote.staging"),
            "anarlog-staging"
        );
        assert_eq!(
            command_name_from_identifier("so.anarlog.Anarlog"),
            "anarlog"
        );
    }

    #[test]
    fn signature_matches_reference_hmac() {
        // Precomputed with python hmac: key=whsec_test, message={"a":1}
        assert_eq!(
            sign_payload("whsec_test", br#"{"a":1}"#),
            "51426af50a41dd7ff2cd3f116594734766d4018d15d6fb07169aee5d2959adf5"
        );
    }

    #[test]
    fn embeds_the_complete_skill_package() {
        assert!(SKILL_FILES.iter().any(|(path, _)| *path == "SKILL.md"));
        for (path, content) in SKILL_FILES {
            assert!(!content.trim().is_empty(), "{path} is empty");
        }
    }
}
