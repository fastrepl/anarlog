//! Live smoke tests. Gated behind `EXEDEV_TOKEN`; if unset, tests early-return.
//!
//! ```sh
//! EXEDEV_TOKEN=exe1.XXXXX cargo test -p exedev --tests live_smoke -- --nocapture
//! ```

use exedev::{ExedevClient, VmStatus};

fn client() -> Option<ExedevClient> {
    let token = std::env::var("EXEDEV_TOKEN").ok()?;
    Some(
        ExedevClient::builder()
            .token(token)
            .build()
            .expect("client build"),
    )
}

#[tokio::test]
async fn whoami_is_typed() {
    let Some(client) = client() else {
        eprintln!("EXEDEV_TOKEN not set; skipping");
        return;
    };
    let me = client.whoami().await.expect("whoami");
    assert!(!me.email.is_empty(), "email empty: {me:?}");
    assert!(
        !me.ssh_keys.is_empty(),
        "expected at least one ssh key: {me:?}"
    );
}

#[tokio::test]
async fn vm_list_is_typed() {
    let Some(client) = client() else {
        eprintln!("EXEDEV_TOKEN not set; skipping");
        return;
    };
    let vms = client.vm_list().await.expect("vm_list");
    for vm in &vms {
        assert!(!vm.name.is_empty(), "name empty: {vm:?}");
        // Ensure we don't regress to Unknown for the live statuses we expect.
        if matches!(vm.status, VmStatus::Unknown) {
            eprintln!("WARN: unknown status for {}: extend VmStatus", vm.name);
        }
    }
}

#[tokio::test]
async fn ssh_key_list_is_typed() {
    let Some(client) = client() else {
        eprintln!("EXEDEV_TOKEN not set; skipping");
        return;
    };
    let keys = client.ssh_key_list().await.expect("ssh_key_list");
    assert!(!keys.is_empty());
    for k in &keys {
        assert!(k.public_key.starts_with("ssh-"), "bad pubkey: {k:?}");
        assert!(k.fingerprint.starts_with("SHA256:"), "bad fp: {k:?}");
    }
}
