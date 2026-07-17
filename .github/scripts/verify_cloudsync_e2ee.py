#!/usr/bin/env python3

import base64
import datetime
import json
import os
import re
import secrets
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
import uuid


CLOUDSYNC_MANAGEMENT_URL = "https://cloudsync.sqlite.ai"
REQUIRED_SECRETS = (
    "ANARLOG_CLOUDSYNC_DATABASE_ID",
    "ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID",
    "SQLITECLOUD_CLOUDSYNC_MANAGEMENT_API_KEY",
    "SQLITECLOUD_PROJECT_URL",
    "SQLITECLOUD_TOKEN_ISSUER_API_KEY",
)
SENSITIVE_PARENT_ENV = {
    "ACTIONS_ID_TOKEN_REQUEST_TOKEN",
    "ACTIONS_RUNTIME_TOKEN",
    "FLY_API_TOKEN",
    "GITHUB_TOKEN",
    "INFISICAL_PROJECT_ID",
    "INFISICAL_TOKEN",
}


class NoRedirectHandler(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, file_pointer, code, message, headers, new_url):
        return None


HTTP = urllib.request.build_opener(NoRedirectHandler())


def load_secrets(path: str) -> dict[str, str]:
    with open(path) as source:
        exported = json.load(source)

    values = {secret["key"]: secret.get("value", "") for secret in exported}
    missing = [key for key in REQUIRED_SECRETS if not values.get(key)]
    if missing:
        raise ValueError("Missing required CloudSync secrets: " + ", ".join(missing))
    return values


def validate_https_url(url: str, label: str) -> None:
    parsed = urllib.parse.urlsplit(url)
    if (
        parsed.scheme != "https"
        or not parsed.hostname
        or parsed.username
        or parsed.password
        or parsed.fragment
    ):
        raise ValueError(f"{label} must use HTTPS without embedded credentials")


def validate_https_origin(url: str, label: str) -> None:
    validate_https_url(url, label)
    parsed = urllib.parse.urlsplit(url)
    if parsed.path not in ("", "/") or parsed.query:
        raise ValueError(f"{label} must be an HTTPS origin")


def request_json(
    url: str,
    bearer: str,
    label: str,
    body: dict[str, object] | None = None,
) -> dict[str, object]:
    validate_https_url(url, label)
    encoded_body = json.dumps(body).encode() if body is not None else None
    request = urllib.request.Request(
        url,
        data=encoded_body,
        headers={
            "Accept": "application/json",
            "Authorization": f"Bearer {bearer}",
            **({"Content-Type": "application/json"} if body is not None else {}),
        },
        method="POST" if body is not None else "GET",
    )
    try:
        with HTTP.open(request, timeout=30) as response:
            envelope = json.load(response)
    except urllib.error.HTTPError as error:
        raise RuntimeError(f"{label} failed with HTTP {error.code}") from error
    except urllib.error.URLError as error:
        raise RuntimeError(f"{label} could not connect") from error
    if not isinstance(envelope, dict):
        raise ValueError(f"{label} returned an invalid response")
    return envelope


def response_data(envelope: dict[str, object], label: str) -> object:
    if "data" not in envelope:
        raise ValueError(f"{label} returned no data")
    return envelope["data"]


def management_get(path: str, management_key: str, label: str) -> object:
    envelope = request_json(
        CLOUDSYNC_MANAGEMENT_URL + path,
        management_key,
        label,
    )
    return response_data(envelope, label)


def run_sql(
    project_url: str,
    issuer_key: str,
    database_name: str,
    sql: str,
    label: str,
) -> list[dict[str, object]]:
    envelope = request_json(
        project_url.rstrip("/") + "/v2/weblite/sql",
        issuer_key,
        label,
        {"database": database_name, "sql": sql},
    )
    rows = response_data(envelope, label)
    if not isinstance(rows, list) or not all(isinstance(row, dict) for row in rows):
        raise ValueError(f"{label} returned invalid rows")
    return rows


def verify_remote_database(values: dict[str, str]) -> None:
    legacy_id = values["ANARLOG_CLOUDSYNC_DATABASE_ID"].strip()
    database_id = values["ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID"].strip()
    if database_id == legacy_id:
        raise ValueError(
            "E2EE CloudSync database reuses the legacy managed database ID"
        )

    management_key = values["SQLITECLOUD_CLOUDSYNC_MANAGEMENT_API_KEY"]
    encoded_database_id = urllib.parse.quote(database_id, safe="")
    encoded_legacy_id = urllib.parse.quote(legacy_id, safe="")
    database = management_get(
        f"/v1/databases/{encoded_database_id}",
        management_key,
        "E2EE database registration check",
    )
    legacy_database = management_get(
        f"/v1/databases/{encoded_legacy_id}",
        management_key,
        "legacy database registration check",
    )
    if not isinstance(database, dict) or not isinstance(legacy_database, dict):
        raise ValueError("CloudSync database registration metadata is invalid")

    database_name = str(database.get("databaseName", "")).strip()
    legacy_database_name = str(legacy_database.get("databaseName", "")).strip()
    project_id = str(database.get("projectId", "")).strip()
    legacy_project_id = str(legacy_database.get("projectId", "")).strip()
    if not all((database_name, legacy_database_name, project_id, legacy_project_id)):
        raise ValueError(
            "CloudSync database registration is missing its physical target"
        )
    if (project_id, database_name) == (legacy_project_id, legacy_database_name):
        raise ValueError("E2EE CloudSync database reuses the legacy physical database")

    connection = management_get(
        f"/v1/databases/{encoded_database_id}/connection",
        management_key,
        "E2EE database connection check",
    )
    if not isinstance(connection, dict) or connection.get("ok") is not True:
        raise ValueError("E2EE CloudSync database connection check failed")

    tables = management_get(
        f"/v1/databases/{encoded_database_id}/cloudsync/tables",
        management_key,
        "E2EE CloudSync table check",
    )
    if not isinstance(tables, list) or not all(
        isinstance(table, dict) for table in tables
    ):
        raise ValueError("E2EE CloudSync table metadata is invalid")
    if (
        len(tables) != 1
        or tables[0].get("name") != "e2ee_records"
        or tables[0].get("enabled") is not True
    ):
        raise ValueError("E2EE database must enable only the e2ee_records user table")

    project_url = values["SQLITECLOUD_PROJECT_URL"]
    validate_https_origin(project_url, "SQLITECLOUD_PROJECT_URL")
    issuer_key = values["SQLITECLOUD_TOKEN_ISSUER_API_KEY"]
    ddl_rows = run_sql(
        project_url,
        issuer_key,
        database_name,
        "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'e2ee_records'",
        "E2EE table definition check",
    )
    if len(ddl_rows) != 1 or not isinstance(ddl_rows[0].get("sql"), str):
        raise ValueError("E2EE database is missing the e2ee_records table definition")
    if not re.search(r"\)\s*STRICT\s*;?\s*$", str(ddl_rows[0]["sql"]), re.IGNORECASE):
        raise ValueError("e2ee_records must be a STRICT table")

    columns = run_sql(
        project_url,
        issuer_key,
        database_name,
        "PRAGMA table_xinfo(e2ee_records)",
        "E2EE table column check",
    )
    column_shape = [
        (
            str(column.get("name", "")),
            str(column.get("type", "")).upper(),
            int(column.get("notnull", 0)),
            int(column.get("pk", 0)),
            int(column.get("hidden", 0)),
        )
        for column in columns
    ]
    expected_columns = [
        ("id", "TEXT", 1, 1, 0),
        ("workspace_id", "TEXT", 1, 0, 0),
        ("payload", "TEXT", 1, 0, 0),
        ("created_at", "TEXT", 1, 0, 0),
        ("updated_at", "TEXT", 1, 0, 0),
    ]
    if column_shape != expected_columns:
        raise ValueError("e2ee_records has an unexpected column contract")
    defaults = [column.get("dflt_value") for column in columns]
    timestamp_default = "strftime('%y-%m-%dt%h:%m:%fz','now')"
    if (
        defaults[0] is not None
        or defaults[1:3] != ["''", "''"]
        or any(
            timestamp_default not in "".join(str(value).lower().split())
            for value in defaults[3:]
        )
    ):
        raise ValueError("e2ee_records has unexpected column defaults")

    indexes = run_sql(
        project_url,
        issuer_key,
        database_name,
        "PRAGMA index_list(e2ee_records)",
        "E2EE table index check",
    )
    workspace_index = next(
        (
            index
            for index in indexes
            if index.get("name") == "idx_e2ee_records_workspace"
        ),
        None,
    )
    if (
        workspace_index is None
        or int(workspace_index.get("unique", 1)) != 0
        or int(workspace_index.get("partial", 1)) != 0
    ):
        raise ValueError("e2ee_records is missing its workspace index")
    indexed_columns = run_sql(
        project_url,
        issuer_key,
        database_name,
        "PRAGMA index_info(idx_e2ee_records_workspace)",
        "E2EE workspace index column check",
    )
    indexed_columns.sort(key=lambda column: int(column.get("seqno", -1)))
    if [column.get("name") for column in indexed_columns] != ["workspace_id", "id"]:
        raise ValueError("e2ee_records workspace index has unexpected columns")


def mint_token(
    project_url: str,
    issuer_key: str,
    user_id: str,
    workspace_ids: list[str] | None = None,
) -> str:
    validate_https_origin(project_url, "SQLITECLOUD_PROJECT_URL")
    workspace_ids = workspace_ids or [user_id]
    if user_id not in workspace_ids:
        raise ValueError(
            "verification token attributes must include the personal workspace"
        )
    expires_at = datetime.datetime.now(datetime.UTC) + datetime.timedelta(minutes=30)
    envelope = request_json(
        project_url.rstrip("/") + "/v2/tokens",
        issuer_key,
        "SQLite Cloud verification token request",
        {
            "name": "anarlog-cloudsync-deploy-verification",
            "userId": user_id,
            "expiresAt": expires_at.isoformat(timespec="seconds").replace(
                "+00:00", "Z"
            ),
            "attributes": json.dumps(
                {"workspace_ids": workspace_ids}, separators=(",", ":")
            ),
        },
    )
    data = response_data(envelope, "SQLite Cloud verification token request")
    token = data.get("token", "").strip() if isinstance(data, dict) else ""
    if not token:
        raise ValueError("SQLite Cloud returned an empty verification token")
    return token


def recovery_key() -> str:
    encoded = base64.urlsafe_b64encode(secrets.token_bytes(32)).decode().rstrip("=")
    return f"anarlog-e2ee-v1:{encoded}"


def sanitized_cargo_env() -> dict[str, str]:
    environment = os.environ.copy()
    for key in list(environment):
        upper_key = key.upper()
        if key in SENSITIVE_PARENT_ENV or upper_key.endswith(
            ("_API_KEY", "_PASSWORD", "_SECRET", "_TOKEN")
        ):
            environment.pop(key)
    return environment


def run_test(test: str, environment: dict[str, str]) -> None:
    subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "db-app",
            "--test",
            "cloudsync",
            test,
            "--",
            "--ignored",
            "--exact",
        ],
        check=True,
        env=environment,
        timeout=20 * 60,
    )


def run_test_with_cleanup(test: str, environment: dict[str, str]) -> None:
    test_error = None
    try:
        run_test(test, environment)
    except subprocess.SubprocessError as error:
        test_error = error

    cleanup_error = None
    try:
        run_test("cleanup_e2ee_verification_workspaces", environment)
    except subprocess.SubprocessError as error:
        cleanup_error = error

    if test_error and cleanup_error:
        raise RuntimeError(
            f"{test} failed and its remote cleanup also failed"
        ) from test_error
    if test_error:
        raise test_error
    if cleanup_error:
        raise cleanup_error


def main() -> None:
    if len(sys.argv) != 2:
        raise ValueError("usage: verify_cloudsync_e2ee.py <infisical-export.json>")

    values = load_secrets(sys.argv[1])
    verify_remote_database(values)

    cargo_env = sanitized_cargo_env()
    subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "db-app",
            "--test",
            "cloudsync",
            "--no-run",
        ],
        check=True,
        env=cargo_env,
        timeout=20 * 60,
    )

    project_url = values["SQLITECLOUD_PROJECT_URL"]
    issuer_key = values["SQLITECLOUD_TOKEN_ISSUER_API_KEY"]
    database_id = values["ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID"]

    sync_workspace_a = f"deploy-e2ee-a-{uuid.uuid4()}"
    sync_workspace_b = f"deploy-e2ee-b-{uuid.uuid4()}"
    sync_env = cargo_env | {
        "ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID": database_id,
        "ANARLOG_CLOUDSYNC_WORKSPACE_A": sync_workspace_a,
        "ANARLOG_CLOUDSYNC_WORKSPACE_B": sync_workspace_b,
        "ANARLOG_CLOUDSYNC_TOKEN_A": mint_token(
            project_url, issuer_key, sync_workspace_a
        ),
        "ANARLOG_CLOUDSYNC_TOKEN_B": mint_token(
            project_url, issuer_key, sync_workspace_b
        ),
        "ANARLOG_CLOUDSYNC_RECOVERY_KEY_A": recovery_key(),
    }
    run_test_with_cleanup(
        "same_personal_workspace_syncs_and_decrypts_a_real_note",
        sync_env,
    )

    workspace_a = f"deploy-e2ee-a-{uuid.uuid4()}"
    workspace_b = f"deploy-e2ee-b-{uuid.uuid4()}"
    # Production tokens include shared memberships; personal-only RLS must not trust them yet.
    policy_env = cargo_env | {
        "ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID": database_id,
        "ANARLOG_CLOUDSYNC_WORKSPACE_A": workspace_a,
        "ANARLOG_CLOUDSYNC_WORKSPACE_B": workspace_b,
        "ANARLOG_CLOUDSYNC_TOKEN_A": mint_token(
            project_url,
            issuer_key,
            workspace_a,
            [workspace_a, workspace_b],
        ),
        "ANARLOG_CLOUDSYNC_TOKEN_B": mint_token(project_url, issuer_key, workspace_b),
        "ANARLOG_CLOUDSYNC_RECOVERY_KEY_B": recovery_key(),
    }
    run_test_with_cleanup(
        "personal_workspace_tokens_block_foreign_encrypted_writes",
        policy_env,
    )


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(
            f"CloudSync E2EE deployment verification failed: {error}", file=sys.stderr
        )
        raise SystemExit(1) from error
