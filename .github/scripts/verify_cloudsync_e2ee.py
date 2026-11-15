#!/usr/bin/env python3

from __future__ import annotations

import base64
import datetime
import json
import os
import re
import secrets
import subprocess
import sys
import tempfile
import urllib.error
import urllib.parse
import urllib.request
import uuid
from typing import NamedTuple


CLOUDSYNC_MANAGEMENT_URL = "https://cloudsync.sqlite.ai"
REQUIRED_SECRETS = (
    "ANARLOG_CLOUDSYNC_DATABASE_ID",
    "ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID",
    "ANARLOG_CLOUDSYNC_PROTOCOL_MODE",
    "SQLITECLOUD_CLOUDSYNC_MANAGEMENT_API_KEY",
    "SQLITECLOUD_PROJECT_URL",
    "SQLITECLOUD_TOKEN_ISSUER_API_KEY",
)
PROTOCOL_MODES = {"dual", "e2ee_only", "e2ee_enforced"}
SQLITECLOUD_MODES = {"enabled", "draining", "retired"}
# Two dedicated Pro accounts. The recovery key must stay the same between runs:
# an account is bound to its first key and every run reads the earlier history.
REPLICA_SECRETS = (
    "ANARLOG_REPLICA_VERIFY_API_URL",
    "ANARLOG_REPLICA_VERIFY_SUPABASE_URL",
    "ANARLOG_REPLICA_VERIFY_SUPABASE_ANON_KEY",
    "ANARLOG_REPLICA_VERIFY_EMAIL_A",
    "ANARLOG_REPLICA_VERIFY_PASSWORD_A",
    "ANARLOG_REPLICA_VERIFY_RECOVERY_KEY_A",
    "ANARLOG_REPLICA_VERIFY_EMAIL_B",
    "ANARLOG_REPLICA_VERIFY_PASSWORD_B",
)
REPLICA_TEST = "fresh_devices_converge_through_the_replica_and_other_accounts_stay_out"
LATENCY_LOG_ENV = "ANARLOG_CLOUDSYNC_LATENCY_LOG"
# A fresh device's first download waits on provider snapshot preparation over the whole
# tenant history, which deployments cannot change. Report it instead of failing on it;
# the tests themselves only fail when a bootstrap never completes.
FRESH_DEVICE_LATENCY_TARGET_SECONDS = 90
# Per-test subprocess budgets. Fresh-site waits stop at 180s inside the tests: the sync
# test waits for one bootstrap, the policy test for one and then two concurrent ones
# plus bounded attack syncs, and cleanup never waits for a snapshot. Their 40-minute
# total leaves the 60-minute deploy job room for the build and remote cleanup.
TEST_TIMEOUT_MINUTES = {
    "same_personal_workspace_syncs_and_decrypts_a_real_note": 10,
    "personal_workspace_tokens_block_foreign_encrypted_writes": 20,
    "cleanup_e2ee_verification_workspaces": 5,
}
LEGACY_PLAINTEXT_TABLES = (
    "action_items",
    "events",
    "humans",
    "organizations",
    "session_documents",
    "session_participants",
    "sessions",
    "transcripts",
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


class ResourceNotFound(RuntimeError):
    pass


def sqlitecloud_mode(values: dict[str, str]) -> str:
    mode = values.get("ANARLOG_SQLITECLOUD_MODE", "").strip() or "enabled"
    if mode not in SQLITECLOUD_MODES:
        raise ValueError(
            "ANARLOG_SQLITECLOUD_MODE must be enabled, draining, or retired"
        )
    return mode


def load_secrets(path: str) -> dict[str, str]:
    with open(path) as source:
        exported = json.load(source)

    values = {secret["key"]: secret.get("value", "") for secret in exported}
    if sqlitecloud_mode(values) == "retired":
        return values
    missing = [key for key in REQUIRED_SECRETS if not values.get(key)]
    if missing:
        raise ValueError("Missing required CloudSync secrets: " + ", ".join(missing))
    return values


def replica_verification_configured(values: dict[str, str], mode: str) -> bool:
    missing = [key for key in REPLICA_SECRETS if not values.get(key)]
    if not missing:
        return True
    # Once the provider is retired nothing else verifies sync, so the replica
    # check is required. A partial set is a mistake in every mode.
    if mode == "retired" or len(missing) < len(REPLICA_SECRETS):
        raise ValueError("Missing replica verification secrets: " + ", ".join(missing))
    return False


def protocol_mode(values: dict[str, str]) -> str:
    mode = values["ANARLOG_CLOUDSYNC_PROTOCOL_MODE"].strip()
    if mode not in PROTOCOL_MODES:
        raise ValueError(
            "ANARLOG_CLOUDSYNC_PROTOCOL_MODE must be dual, e2ee_only, or e2ee_enforced"
        )
    return mode


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


def validate_service_origin(url: str, label: str) -> None:
    parsed = urllib.parse.urlsplit(url)
    # Isolated staging runs the API and Supabase on this machine.
    if parsed.scheme == "http" and parsed.hostname in ("127.0.0.1", "localhost"):
        if parsed.path in ("", "/") and not parsed.query and not parsed.username:
            return
    validate_https_origin(url, label)


def request_json(
    url: str,
    bearer: str,
    label: str,
    body: dict[str, object] | None = None,
    headers: dict[str, str] | None = None,
) -> dict[str, object]:
    if headers is None:
        validate_https_url(url, label)
        headers = {"Authorization": f"Bearer {bearer}"}
    encoded_body = json.dumps(body).encode() if body is not None else None
    request = urllib.request.Request(
        url,
        data=encoded_body,
        headers={
            "Accept": "application/json",
            **headers,
            **({"Content-Type": "application/json"} if body is not None else {}),
        },
        method="POST" if body is not None else "GET",
    )
    try:
        with HTTP.open(request, timeout=30) as response:
            envelope = json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            raise ResourceNotFound(f"{label} returned HTTP 404") from error
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


def verify_legacy_database_retired(legacy_id: str, management_key: str) -> None:
    encoded_legacy_id = urllib.parse.quote(legacy_id, safe="")
    try:
        management_get(
            f"/v1/databases/{encoded_legacy_id}",
            management_key,
            "legacy database retirement check",
        )
    except ResourceNotFound:
        return

    raise ValueError(
        "Legacy plaintext CloudSync database must be deleted before E2EE deployment"
    )


def verify_legacy_database_transition(
    mode: str,
    legacy_id: str,
    management_key: str,
    e2ee_target: tuple[str, str],
) -> None:
    if mode == "e2ee_enforced":
        verify_legacy_database_retired(legacy_id, management_key)
        return

    encoded_legacy_id = urllib.parse.quote(legacy_id, safe="")
    try:
        database = management_get(
            f"/v1/databases/{encoded_legacy_id}",
            management_key,
            "legacy database transition check",
        )
    except ResourceNotFound:
        if mode == "dual":
            raise ValueError(
                "Legacy plaintext CloudSync database must exist in dual protocol mode"
            )
        return

    if not isinstance(database, dict):
        raise ValueError("Legacy CloudSync database registration metadata is invalid")
    database_name = str(database.get("databaseName", "")).strip()
    project_id = str(database.get("projectId", "")).strip()
    if not all((database_name, project_id)):
        raise ValueError(
            "Legacy CloudSync database registration is missing its physical target"
        )
    if (project_id, database_name) == e2ee_target:
        raise ValueError("E2EE CloudSync database reuses the legacy physical database")
    connection = management_get(
        f"/v1/databases/{encoded_legacy_id}/connection",
        management_key,
        "legacy database connection check",
    )
    if not isinstance(connection, dict) or connection.get("ok") is not True:
        raise ValueError("Legacy CloudSync database connection check failed")


def verify_no_plaintext_tables(
    project_url: str,
    issuer_key: str,
    database_name: str,
) -> None:
    quoted_names = ", ".join(f"'{name}'" for name in LEGACY_PLAINTEXT_TABLES)
    rows = run_sql(
        project_url,
        issuer_key,
        database_name,
        "SELECT name FROM sqlite_schema "
        f"WHERE type = 'table' AND name IN ({quoted_names}) ORDER BY name",
        "E2EE plaintext table check",
    )
    plaintext_tables = [str(row.get("name", "")) for row in rows if row.get("name")]
    if plaintext_tables:
        raise ValueError(
            "E2EE database contains legacy plaintext tables: "
            + ", ".join(plaintext_tables)
        )


def verify_e2ee_record_columns(columns: list[dict[str, object]]) -> None:
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
            for value in defaults[3:5]
        )
    ):
        raise ValueError("e2ee_records has unexpected column defaults")


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


def verify_remote_database(values: dict[str, str]) -> str:
    mode = protocol_mode(values)
    legacy_id = values["ANARLOG_CLOUDSYNC_DATABASE_ID"].strip()
    database_id = values["ANARLOG_CLOUDSYNC_E2EE_DATABASE_ID"].strip()
    if database_id == legacy_id:
        raise ValueError(
            "E2EE CloudSync database reuses the legacy managed database ID"
        )

    management_key = values["SQLITECLOUD_CLOUDSYNC_MANAGEMENT_API_KEY"]
    encoded_database_id = urllib.parse.quote(database_id, safe="")
    database = management_get(
        f"/v1/databases/{encoded_database_id}",
        management_key,
        "E2EE database registration check",
    )
    if not isinstance(database, dict):
        raise ValueError("CloudSync database registration metadata is invalid")

    database_name = str(database.get("databaseName", "")).strip()
    project_id = str(database.get("projectId", "")).strip()
    if not all((database_name, project_id)):
        raise ValueError(
            "CloudSync database registration is missing its physical target"
        )
    verify_legacy_database_transition(
        mode,
        legacy_id,
        management_key,
        (project_id, database_name),
    )

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
    verify_no_plaintext_tables(project_url, issuer_key, database_name)
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
    verify_e2ee_record_columns(columns)

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

    return database_name


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
        timeout=TEST_TIMEOUT_MINUTES.get(test, 20) * 60,
    )


def cleanup_remote_workspaces(
    environment: dict[str, str], project_url: str, database_name: str
) -> None:
    for suffix in ("A", "B"):
        workspace = environment[f"ANARLOG_CLOUDSYNC_WORKSPACE_{suffix}"]
        prefix = f"deploy-e2ee-{suffix.lower()}-"
        if not workspace.startswith(prefix):
            raise ValueError("cleanup requires a deployment verification workspace")
        uuid.UUID(workspace.removeprefix(prefix))
        token = environment[f"ANARLOG_CLOUDSYNC_TOKEN_{suffix}"]
        where = f"workspace_id = '{workspace}'"
        # A fresh native client cannot clean up when the failing check is download visibility.
        request_json(
            project_url.rstrip("/") + "/v2/weblite/sql",
            token,
            "verification workspace cleanup",
            {
                "database": database_name,
                "sql": f"DELETE FROM e2ee_records WHERE {where}",
            },
        )
        remaining = run_sql(
            project_url,
            token,
            database_name,
            f"SELECT count(*) AS remaining FROM e2ee_records WHERE {where}",
            "verification workspace cleanup check",
        )
        if remaining != [{"remaining": 0}]:
            raise RuntimeError("verification workspace cleanup left remote records")


class LatencyEntry(NamedTuple):
    check: str
    seconds: float
    completed: bool
    # Time until the first non-empty receive; None when no page arrived or the test
    # predates the breakdown. Earlier calls came back empty while the provider prepared.
    first_page_seconds: float | None = None
    empty_calls_before_first_page: int | None = None
    sync_calls: int | None = None
    sync_call_seconds: float | None = None
    snapshot_resets: int | None = None
    rows: int | None = None
    bytes: int | None = None


def read_latency_log(path: str) -> list[LatencyEntry]:
    try:
        with open(path) as log:
            lines = log.read().splitlines()
    except FileNotFoundError:
        return []
    entries = []
    for line in lines:
        if line.strip():
            entry = json.loads(line)
            first_page = entry.get("first_page_seconds")
            entries.append(
                LatencyEntry(
                    check=str(entry["check"]),
                    seconds=float(entry["seconds"]),
                    completed=bool(entry.get("completed", True)),
                    first_page_seconds=None
                    if first_page is None
                    else float(first_page),
                    empty_calls_before_first_page=entry.get(
                        "empty_calls_before_first_page"
                    ),
                    sync_calls=entry.get("sync_calls"),
                    sync_call_seconds=entry.get("sync_call_seconds"),
                    snapshot_resets=entry.get("snapshot_resets"),
                    rows=entry.get("rows"),
                    bytes=entry.get("bytes"),
                )
            )
    return entries


def latency_status(seconds: float, completed: bool) -> str:
    if not completed:
        return "timed out"
    if seconds > FRESH_DEVICE_LATENCY_TARGET_SECONDS:
        return "over target"
    return "ok"


def describe_first_page(entry: LatencyEntry) -> str:
    if entry.sync_calls is None:
        return ""
    if entry.first_page_seconds is None:
        return f"; no page after {entry.empty_calls_before_first_page} empty sync calls"
    return (
        f"; first page at {entry.first_page_seconds:.1f}s"
        f" after {entry.empty_calls_before_first_page} empty sync calls"
    )


def latency_summary_row(entry: LatencyEntry) -> str:
    status = latency_status(entry.seconds, entry.completed)
    if entry.sync_calls is None:
        return f"| {entry.check} | {entry.seconds:.1f}s | {status} | n/a | n/a | n/a | n/a | n/a |"
    if entry.first_page_seconds is None:
        first_page = after_first_page = "none"
    else:
        first_page = f"{entry.first_page_seconds:.1f}s"
        after_first_page = f"{entry.seconds - entry.first_page_seconds:.1f}s"
    return (
        f"| {entry.check} | {entry.seconds:.1f}s | {status} | {first_page}"
        f" | {after_first_page} | {entry.empty_calls_before_first_page}"
        f" | {entry.sync_calls} ({entry.sync_call_seconds:.1f}s)"
        f" | {entry.snapshot_resets} | {entry.rows} rows, {entry.bytes} B |"
    )


def report_fresh_device_latency(path: str) -> None:
    entries = read_latency_log(path)
    target = FRESH_DEVICE_LATENCY_TARGET_SECONDS
    for entry in entries:
        status = latency_status(entry.seconds, entry.completed)
        level = "notice" if status == "ok" else "warning"
        print(
            f"::{level} title=CloudSync fresh-device latency::"
            f"{entry.check}: {entry.seconds:.1f}s, {status} (target {target}s)"
            f"{describe_first_page(entry)}"
        )

    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if not entries or not summary_path:
        return
    rows = [latency_summary_row(entry) for entry in entries]
    with open(summary_path, "a") as summary:
        summary.write(
            "\n".join(
                [
                    "",
                    "### CloudSync fresh-device latency",
                    "",
                    f"Target: {target}s. Reported only; it does not fail the deployment.",
                    "Empty sync calls before the first page are the provider preparing"
                    " its snapshot; time after it is download, apply, and waiting for"
                    " the expected record.",
                    "",
                    "| Check | Latency | Status | First page | After first page"
                    " | Empty calls before first page | Sync calls (time in calls)"
                    " | Snapshot resets | Received |",
                    "| --- | --- | --- | --- | --- | --- | --- | --- | --- |",
                    *rows,
                    "",
                ]
            )
        )


def run_test_with_cleanup(
    test: str,
    environment: dict[str, str],
    project_url: str,
    database_name: str,
) -> None:
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

    try:
        cleanup_remote_workspaces(environment, project_url, database_name)
    except Exception as error:
        cleanup_error = error

    if test_error and cleanup_error:
        raise RuntimeError(
            f"{test} failed and its remote cleanup also failed"
        ) from test_error
    if test_error:
        raise test_error
    if cleanup_error:
        raise cleanup_error


def sign_in(supabase_url: str, anon_key: str, email: str, password: str) -> str:
    session = request_json(
        supabase_url.rstrip("/") + "/auth/v1/token?grant_type=password",
        "",
        "replica verification sign-in",
        {"email": email, "password": password},
        {"apikey": anon_key},
    )
    token = session.get("access_token")
    if not isinstance(token, str) or not token:
        raise ValueError("replica verification sign-in returned no access token")
    return token


def serving_sqlitecloud_mode(api_url: str) -> str:
    # The deployment being verified may be the one that changes the mode, so the
    # credential gate is checked against what the serving API reports. Images
    # from before the lifecycle setting report nothing and always issue tokens.
    health = request_json(api_url + "/health/sync", "", "sync health", headers={})
    return sqlitecloud_mode(
        {"ANARLOG_SQLITECLOUD_MODE": str(health.get("sqlitecloud_mode") or "")}
    )


def verify_replica(values: dict[str, str]) -> None:
    api_url = values["ANARLOG_REPLICA_VERIFY_API_URL"].rstrip("/")
    supabase_url = values["ANARLOG_REPLICA_VERIFY_SUPABASE_URL"]
    validate_service_origin(api_url, "ANARLOG_REPLICA_VERIFY_API_URL")
    validate_service_origin(supabase_url, "ANARLOG_REPLICA_VERIFY_SUPABASE_URL")
    anon_key = values["ANARLOG_REPLICA_VERIFY_SUPABASE_ANON_KEY"]
    environment = (
        sanitized_cargo_env()
        | {
            "ANARLOG_REPLICA_API_URL": api_url,
            "ANARLOG_REPLICA_SQLITECLOUD_MODE": serving_sqlitecloud_mode(api_url),
            "ANARLOG_REPLICA_RECOVERY_KEY_A": values[
                "ANARLOG_REPLICA_VERIFY_RECOVERY_KEY_A"
            ],
        }
        | {
            f"ANARLOG_REPLICA_ACCESS_TOKEN_{account}": sign_in(
                supabase_url,
                anon_key,
                values[f"ANARLOG_REPLICA_VERIFY_EMAIL_{account}"],
                values[f"ANARLOG_REPLICA_VERIFY_PASSWORD_{account}"],
            )
            for account in ("A", "B")
        }
    )
    subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "db-sync",
            "--test",
            "replica",
            REPLICA_TEST,
            "--",
            "--ignored",
            "--exact",
        ],
        check=True,
        env=environment,
        timeout=20 * 60,
    )


def verify_provider(values: dict[str, str], mode: str) -> None:
    database_name = verify_remote_database(values)
    if mode == "draining":
        # The sync tests mint provider tokens and write fixture rows, which
        # would move the write boundary the final reconciliation compares against.
        print(
            "::notice title=SQLite Cloud draining::"
            "Verified the provider registration and schema read-only; skipped sync tests"
        )
        return

    latency_log_fd, latency_log = tempfile.mkstemp(
        prefix="cloudsync-latency-", suffix=".jsonl"
    )
    os.close(latency_log_fd)
    try:
        run_verification(values, database_name, latency_log)
    finally:
        report_fresh_device_latency(latency_log)
        os.remove(latency_log)


def main() -> None:
    if len(sys.argv) != 2:
        raise ValueError("usage: verify_cloudsync_e2ee.py <infisical-export.json>")

    values = load_secrets(sys.argv[1])
    mode = sqlitecloud_mode(values)
    verifies_replica = replica_verification_configured(values, mode)
    # Retired means no provider request may be made at all.
    if mode != "retired":
        verify_provider(values, mode)
    if verifies_replica:
        verify_replica(values)
    else:
        print(
            "::warning title=Encrypted replica not verified::"
            "Replica verification accounts are not configured"
        )


def run_verification(
    values: dict[str, str], database_name: str, latency_log: str
) -> None:
    cargo_env = sanitized_cargo_env() | {LATENCY_LOG_ENV: latency_log}
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
        project_url,
        database_name,
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
        project_url,
        database_name,
    )


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(
            f"CloudSync E2EE deployment verification failed: {error}", file=sys.stderr
        )
        raise SystemExit(1) from error
