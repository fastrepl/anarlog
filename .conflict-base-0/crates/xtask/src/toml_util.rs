use anyhow::{Context, Result};
use std::{env, fs, path::Path};

type TomlTable = toml::map::Map<String, toml::Value>;

pub(crate) fn supabase_patch() -> Result<()> {
    let path = crate::repo_root().join("supabase/config.toml");
    let mut config: toml::Table = fs::read_to_string(&path)
        .context("read supabase/config.toml")?
        .parse()
        .context("parse supabase/config.toml")?;

    toml_set_key(&mut config, "db.major_version", 17.into());
    toml_set_key(&mut config, "auth.site_url", "http://localhost:3000".into());
    toml_set_key(
        &mut config,
        "auth.additional_redirect_urls",
        toml::Value::Array(vec!["http://localhost:3000/callback/auth".into()]),
    );

    toml_ensure_table(&mut config, "auth.external.github");
    toml_ensure_table(&mut config, "auth.external.google");
    toml_ensure_table(&mut config, "auth.hook.custom_access_token");

    if let (Ok(id), Ok(secret)) = (
        env::var("GITHUB_CLIENT_ID"),
        env::var("GITHUB_CLIENT_SECRET"),
    ) {
        toml_set_key(&mut config, "auth.external.github.enabled", true.into());
        toml_set_key(&mut config, "auth.external.github.client_id", id.into());
        toml_set_key(&mut config, "auth.external.github.secret", secret.into());
        toml_set_key(&mut config, "auth.external.github.redirect_uri", "".into());
    }

    if let (Ok(id), Ok(secret)) = (
        env::var("GOOGLE_CLIENT_ID"),
        env::var("GOOGLE_CLIENT_SECRET"),
    ) {
        toml_set_key(&mut config, "auth.external.google.enabled", true.into());
        toml_set_key(&mut config, "auth.external.google.client_id", id.into());
        toml_set_key(&mut config, "auth.external.google.secret", secret.into());
        toml_set_key(
            &mut config,
            "auth.external.google.skip_nonce_check",
            false.into(),
        );
    }

    toml_set_key(
        &mut config,
        "auth.hook.custom_access_token.enabled",
        true.into(),
    );
    toml_set_key(
        &mut config,
        "auth.hook.custom_access_token.uri",
        "pg-functions://postgres/public/custom_access_token_hook".into(),
    );

    fs::write(
        &path,
        toml::to_string_pretty(&config).context("serialize TOML")?,
    )
    .context("write supabase/config.toml")
}

pub(crate) fn toml_set(args: &[String]) -> Result<()> {
    let (path_str, pairs) = args
        .split_first()
        .context("usage: toml-set <file> <key> <value> ...")?;
    anyhow::ensure!(pairs.len() % 2 == 0, "key/value args must come in pairs");

    let path = Path::new(path_str);
    let mut config: toml::Table = fs::read_to_string(path)
        .with_context(|| format!("read {path_str}"))?
        .parse()
        .with_context(|| format!("parse {path_str}"))?;

    for chunk in pairs.chunks_exact(2) {
        let (key, val_str) = (&chunk[0], &chunk[1]);
        let value = format!("x={val_str}")
            .parse::<toml::Table>()
            .with_context(|| format!("invalid TOML value: {val_str}"))?
            .remove("x")
            .unwrap();
        toml_set_key(&mut config, key, value);
    }

    fs::write(
        path,
        toml::to_string_pretty(&config).context("serialize TOML")?,
    )
    .with_context(|| format!("write {path_str}"))
}

fn toml_set_key(table: &mut TomlTable, key: &str, value: toml::Value) {
    match key.split_once('.') {
        None => {
            table.insert(key.to_string(), value);
        }
        Some((k, rest)) => {
            let sub = table
                .entry(k)
                .or_insert_with(|| toml::Value::Table(toml::Table::new()));
            if let toml::Value::Table(t) = sub {
                toml_set_key(t, rest, value);
            }
        }
    }
}

fn toml_ensure_table(table: &mut TomlTable, key: &str) {
    match key.split_once('.') {
        None => {
            table
                .entry(key)
                .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        }
        Some((k, rest)) => {
            let sub = table
                .entry(k)
                .or_insert_with(|| toml::Value::Table(toml::Table::new()));
            if let toml::Value::Table(t) = sub {
                toml_ensure_table(t, rest);
            }
        }
    }
}
