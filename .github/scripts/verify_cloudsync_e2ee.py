#!/usr/bin/env python3

from __future__ import annotations

import json
import os
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request


# Two dedicated Pro accounts. The recovery key must stay the same between runs:
# an account is bound to its first key and every run reads the earlier history.
REPLICA_SECRETS = (
    "ANARLOG_REPLICA_VERIFY_API_URL",
    "ANARLOG_REPLICA_VERIFY_SUPABASE_URL",
    "ANARLOG_REPLICA_VERIFY_SUPABASE_SERVICE_ROLE_KEY",
    "ANARLOG_REPLICA_VERIFY_EMAIL_A",
    "ANARLOG_REPLICA_VERIFY_PASSWORD_A",
    "ANARLOG_REPLICA_VERIFY_RECOVERY_KEY_A",
    "ANARLOG_REPLICA_VERIFY_EMAIL_B",
    "ANARLOG_REPLICA_VERIFY_PASSWORD_B",
)
REPLICA_TEST = "fresh_devices_converge_through_the_replica_and_other_accounts_stay_out"
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
    missing = [key for key in REPLICA_SECRETS if not values.get(key)]
    if missing:
        raise ValueError("Missing replica verification secrets: " + ", ".join(missing))
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


def validate_service_origin(url: str, label: str) -> None:
    parsed = urllib.parse.urlsplit(url)
    # Isolated staging runs the API and Supabase on this machine.
    if parsed.scheme == "http" and parsed.hostname in ("127.0.0.1", "localhost"):
        if (
            parsed.path in ("", "/")
            and not parsed.query
            and not parsed.username
            and not parsed.password
            and not parsed.fragment
        ):
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
        raise RuntimeError(f"{label} failed with HTTP {error.code}") from error
    except urllib.error.URLError as error:
        raise RuntimeError(f"{label} could not connect") from error
    if not isinstance(envelope, dict):
        raise ValueError(f"{label} returned an invalid response")
    return envelope


def sanitized_cargo_env() -> dict[str, str]:
    environment = os.environ.copy()
    for key in list(environment):
        upper_key = key.upper()
        if key in SENSITIVE_PARENT_ENV or upper_key.endswith(
            ("_API_KEY", "_PASSWORD", "_SECRET", "_TOKEN")
        ):
            environment.pop(key)
    return environment


def sign_in(supabase_url: str, service_role_key: str, email: str, password: str) -> str:
    # Production sign-in sits behind a captcha that an unattended job cannot
    # solve. Supabase Auth skips it for requests carrying service credentials;
    # the session returned is still the account's own.
    session = request_json(
        supabase_url.rstrip("/") + "/auth/v1/token?grant_type=password",
        "",
        "replica verification sign-in",
        {"email": email, "password": password},
        {"apikey": service_role_key, "Authorization": f"Bearer {service_role_key}"},
    )
    token = session.get("access_token")
    if not isinstance(token, str) or not token:
        raise ValueError("replica verification sign-in returned no access token")
    return token


def verify_replica(values: dict[str, str]) -> None:
    api_url = values["ANARLOG_REPLICA_VERIFY_API_URL"].rstrip("/")
    supabase_url = values["ANARLOG_REPLICA_VERIFY_SUPABASE_URL"]
    validate_service_origin(api_url, "ANARLOG_REPLICA_VERIFY_API_URL")
    validate_service_origin(supabase_url, "ANARLOG_REPLICA_VERIFY_SUPABASE_URL")
    service_role_key = values["ANARLOG_REPLICA_VERIFY_SUPABASE_SERVICE_ROLE_KEY"]
    environment = (
        sanitized_cargo_env()
        | {
            "ANARLOG_REPLICA_API_URL": api_url,
            "ANARLOG_REPLICA_RECOVERY_KEY_A": values[
                "ANARLOG_REPLICA_VERIFY_RECOVERY_KEY_A"
            ],
        }
        | {
            f"ANARLOG_REPLICA_ACCESS_TOKEN_{account}": sign_in(
                supabase_url,
                service_role_key,
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


def main() -> None:
    if len(sys.argv) != 2:
        raise ValueError("usage: verify_cloudsync_e2ee.py <infisical-export.json>")

    values = load_secrets(sys.argv[1])
    verify_replica(values)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(
            f"CloudSync E2EE deployment verification failed: {error}", file=sys.stderr
        )
        raise SystemExit(1) from error
