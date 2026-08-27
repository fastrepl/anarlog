#!/usr/bin/env python3
"""Deploy anarlog-ai without cutting live STT meetings.

Fly blue/green cordons old machines and then SIGTERMs them immediately.
`kill_timeout` maxes out at 300s, which is shorter than a meeting, so this
script instead:

1. Builds and pushes a new image
2. Starts cordoned replacements with the current machine configs and new image
3. Adds the replacements to Fly Proxy and waits for routing to propagate
4. Cordons the previous serving set and waits for routing to propagate
5. Sends SIGUSR1 so those machines reject new STT and exit once idle
6. Destroys leftover stopped+cordoned machines from earlier drains
"""

from __future__ import annotations

import argparse
import copy
import json
import os
import subprocess
import sys
import time
import tomllib
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urlencode
from urllib.request import Request, urlopen


FLY_API_TIMEOUT_SECONDS = 60
FLY_API_RETRY_ATTEMPTS = 5
FLY_API_RETRY_INITIAL_DELAY_SECONDS = 0.5
PROXY_PROPAGATION_SECONDS = 10
DRAIN_PROTOCOL_METADATA_KEY = "anarlog_drain_protocol"
DRAIN_PROTOCOL_METADATA_VALUE = "sigusr1-v1"


class DeployError(RuntimeError):
    pass


class FlyApiError(DeployError):
    def __init__(self, message: str, status_code: int | None = None):
        super().__init__(message)
        self.status_code = status_code


def is_cordoned(machine: dict[str, Any]) -> bool:
    if "cordoned" in machine:
        return bool(machine["cordoned"])
    config = machine.get("config") or {}
    metadata = config.get("metadata") or {}
    return str(metadata.get("fly_cordoned", "")).lower() in {"1", "true"}


def is_stopped(machine: dict[str, Any]) -> bool:
    return machine.get("state") in {"stopped", "suspended"}


def is_started(machine: dict[str, Any]) -> bool:
    return machine.get("state") == "started"


def supports_session_drain(machine: dict[str, Any]) -> bool:
    config = machine.get("config") or {}
    metadata = config.get("metadata") or {}
    return metadata.get(DRAIN_PROTOCOL_METADATA_KEY) == DRAIN_PROTOCOL_METADATA_VALUE


def serving_machines(machines: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [machine for machine in machines if not is_cordoned(machine)]


def stopped_cordoned_machines(machines: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [
        machine for machine in machines if is_cordoned(machine) and is_stopped(machine)
    ]


def checks_passing(machine: dict[str, Any]) -> bool:
    if not is_started(machine) or machine.get("host_status") not in {None, "ok"}:
        return False

    raw_checks = machine.get("checks") or machine.get("Checks")
    if isinstance(raw_checks, dict):
        checks = list(raw_checks.values())
    elif isinstance(raw_checks, list):
        checks = raw_checks
    else:
        return False

    statuses = [
        str(
            check.get("status") or check.get("Status") or ""
            if isinstance(check, dict)
            else check
        ).lower()
        for check in checks
    ]
    return bool(statuses) and all(
        status in {"passing", "success"} for status in statuses
    )


def image_ref(app: str, version: str) -> str:
    return f"registry.fly.io/{app}:api-{version}"


def fly(*args: str) -> None:
    command = ["flyctl", *args]
    print("+", " ".join(command), file=sys.stderr)
    subprocess.run(command, check=True)


def api_request(
    method: str,
    path: str,
    payload: dict[str, Any] | None = None,
    query: dict[str, str] | None = None,
) -> Any:
    token = os.environ.get("FLY_API_TOKEN")
    if not token:
        raise DeployError("FLY_API_TOKEN is required")

    hostname = os.environ.get("FLY_API_HOSTNAME", "https://api.machines.dev")
    if "://" not in hostname:
        hostname = f"https://{hostname}"
    url = f"{hostname.rstrip('/')}/v1{path}"
    if query:
        url = f"{url}?{urlencode(query)}"

    data = json.dumps(payload).encode() if payload is not None else None
    request = Request(
        url,
        data=data,
        method=method,
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
            "User-Agent": "anarlog-drain-deploy/1",
        },
    )
    print(f"+ Fly Machines API {method} {path}", file=sys.stderr)

    try:
        with urlopen(request, timeout=FLY_API_TIMEOUT_SECONDS) as response:
            body = response.read()
    except HTTPError as error:
        body = error.read().decode(errors="replace").strip()
        detail = f": {body}" if body else ""
        raise FlyApiError(
            f"Fly Machines API {method} {path} failed with HTTP {error.code}{detail}",
            error.code,
        ) from error
    except URLError as error:
        raise FlyApiError(
            f"Fly Machines API {method} {path} failed: {error.reason}"
        ) from error

    if not body:
        return None
    return json.loads(body)


def app_path(app: str) -> str:
    return f"/apps/{quote(app, safe='')}"


def machine_path(app: str, machine_id: str) -> str:
    return f"{app_path(app)}/machines/{quote(machine_id, safe='')}"


def list_machines(app: str) -> list[dict[str, Any]]:
    machines = api_request("GET", f"{app_path(app)}/machines")
    if machines is None:
        return []
    if isinstance(machines, dict):
        machines = machines.get("machines") or []
    return list(machines)


def get_machine(app: str, machine_id: str) -> dict[str, Any]:
    machine = api_request("GET", machine_path(app, machine_id))
    if not isinstance(machine, dict):
        raise DeployError(f"Fly returned no status for machine {machine_id}")
    return machine


def destroy_machine(app: str, machine_id: str) -> None:
    api_request(
        "DELETE",
        machine_path(app, machine_id),
        query={"force": "true"},
    )


def change_machine_routing(method: str, path: str) -> None:
    delay = FLY_API_RETRY_INITIAL_DELAY_SECONDS
    for attempt in range(1, FLY_API_RETRY_ATTEMPTS + 1):
        try:
            api_request(method, path)
            return
        except FlyApiError as error:
            retryable = (
                error.status_code is None
                or error.status_code
                in {
                    408,
                    425,
                    429,
                }
                or (error.status_code is not None and error.status_code >= 500)
            )
            if not retryable or attempt == FLY_API_RETRY_ATTEMPTS:
                raise
            print(
                f"routing change failed (attempt {attempt}/{FLY_API_RETRY_ATTEMPTS}); "
                f"retrying in {delay:g}s: {error}",
                file=sys.stderr,
            )
            time.sleep(delay)
            delay *= 2

    raise AssertionError("unreachable")


def cordon_machine(app: str, machine_id: str) -> None:
    change_machine_routing("POST", f"{machine_path(app, machine_id)}/cordon")


def uncordon_machine(app: str, machine_id: str) -> None:
    change_machine_routing("POST", f"{machine_path(app, machine_id)}/uncordon")


def signal_machine(app: str, machine_id: str) -> None:
    api_request(
        "POST",
        f"{machine_path(app, machine_id)}/signal",
        {"signal": "SIGUSR1"},
    )


def destroy_drained_machines(app: str) -> None:
    for machine in stopped_cordoned_machines(list_machines(app)):
        print(
            f"destroying drained machine {machine['id']} ({machine.get('state')})",
            file=sys.stderr,
        )
        destroy_machine(app, machine["id"])


def resume_draining_machines(app: str) -> None:
    for machine in list_machines(app):
        if (
            is_cordoned(machine)
            and is_started(machine)
            and supports_session_drain(machine)
        ):
            print(f"resuming drain for machine {machine['id']}", file=sys.stderr)
            signal_machine(app, machine["id"])


def wait_until_healthy(app: str, machine_id: str, timeout_seconds: int = 180) -> None:
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        status = get_machine(app, machine_id)
        if checks_passing(status):
            print(f"machine {machine_id} is healthy", file=sys.stderr)
            return
        time.sleep(5)
    raise DeployError(f"machine {machine_id} did not become healthy")


def stop_config(config_path: str) -> dict[str, str]:
    with open(config_path, "rb") as config_file:
        config = tomllib.load(config_file)

    signal = str(config.get("kill_signal", "SIGINT"))
    timeout = config.get("kill_timeout")
    result = {"signal": signal}
    if timeout is not None:
        result["timeout"] = f"{timeout}s" if isinstance(timeout, int) else str(timeout)
    return result


def replacement_config(
    machine: dict[str, Any],
    image: str,
    desired_stop_config: dict[str, str],
) -> dict[str, Any]:
    host_status = machine.get("host_status")
    if host_status not in {None, "ok"}:
        raise DeployError(
            f"machine {machine.get('id')} is on an unhealthy host ({host_status})"
        )
    config = machine.get("config")
    if not isinstance(config, dict):
        raise DeployError(f"machine {machine.get('id')} has no reusable config")
    if config.get("mounts"):
        raise DeployError(
            f"machine {machine.get('id')} has volume mounts; drain deploy cannot safely duplicate it"
        )

    replacement = copy.deepcopy(config)
    replacement["image"] = image
    replacement["restart"] = {"policy": "on-failure"}
    replacement["stop_config"] = desired_stop_config
    metadata = replacement.get("metadata")
    if not isinstance(metadata, dict):
        metadata = {}
        replacement["metadata"] = metadata
    metadata.pop("fly_cordoned", None)
    metadata[DRAIN_PROTOCOL_METADATA_KEY] = DRAIN_PROTOCOL_METADATA_VALUE
    return replacement


def create_replacement_machine(
    app: str,
    machine: dict[str, Any],
    image: str,
    desired_stop_config: dict[str, str],
) -> str:
    payload: dict[str, Any] = {
        "config": replacement_config(machine, image, desired_stop_config),
        "skip_service_registration": True,
    }
    if machine.get("region"):
        payload["region"] = machine["region"]

    created = api_request("POST", f"{app_path(app)}/machines", payload)
    if not isinstance(created, dict) or not created.get("id"):
        raise DeployError(f"Fly returned no id for replacement of {machine.get('id')}")
    machine_id = str(created["id"])
    print(
        f"created cordoned replacement {machine_id} for {machine['id']}",
        file=sys.stderr,
    )
    return machine_id


def validate_serving_set(
    app: str,
    expected_ids: set[str],
    replacement_ids: set[str],
) -> None:
    machines = list_machines(app)
    actual_ids = {
        machine["id"]
        for machine in serving_machines(machines)
        if machine["id"] not in replacement_ids
    }
    if actual_ids != expected_ids:
        raise DeployError(
            "serving machine set changed during deploy "
            f"(expected {sorted(expected_ids)}, found {sorted(actual_ids)})"
        )

    machines_by_id = {str(machine["id"]): machine for machine in machines}
    unsafe_replacements = [
        machine_id
        for machine_id in replacement_ids
        if machine_id not in machines_by_id
        or not is_cordoned(machines_by_id[machine_id])
    ]
    if unsafe_replacements:
        raise DeployError(
            "replacement machines were not safely cordoned: "
            + ", ".join(sorted(unsafe_replacements))
        )


def cut_over(
    app: str,
    old_ids: list[str],
    new_ids: list[str],
    propagation_seconds: float = PROXY_PROPAGATION_SECONDS,
) -> None:
    attempted_new_ids: list[str] = []
    attempted_old_ids: list[str] = []
    try:
        for machine_id in new_ids:
            print(f"uncordoning {machine_id}", file=sys.stderr)
            attempted_new_ids.append(machine_id)
            uncordon_machine(app, machine_id)
        if propagation_seconds:
            print(
                f"waiting {propagation_seconds:g}s for Fly Proxy to register replacements",
                file=sys.stderr,
            )
            time.sleep(propagation_seconds)
        for machine_id in old_ids:
            print(f"cordoning {machine_id}", file=sys.stderr)
            attempted_old_ids.append(machine_id)
            cordon_machine(app, machine_id)
        if propagation_seconds:
            print(
                f"waiting {propagation_seconds:g}s for Fly Proxy to drain old routes",
                file=sys.stderr,
            )
            time.sleep(propagation_seconds)
    except Exception:
        for machine_id in attempted_old_ids:
            try:
                uncordon_machine(app, machine_id)
            except Exception as rollback_error:
                print(
                    f"failed to uncordon {machine_id} during rollback: {rollback_error}",
                    file=sys.stderr,
                )
        if attempted_old_ids and propagation_seconds:
            time.sleep(propagation_seconds)
        for machine_id in attempted_new_ids:
            try:
                cordon_machine(app, machine_id)
            except Exception as rollback_error:
                print(
                    f"failed to cordon replacement {machine_id} during rollback: {rollback_error}",
                    file=sys.stderr,
                )
            try:
                signal_machine(app, machine_id)
            except Exception as rollback_error:
                print(
                    f"failed to signal replacement {machine_id} during rollback: {rollback_error}",
                    file=sys.stderr,
                )
        destroy_replacements(
            app,
            [
                machine_id
                for machine_id in new_ids
                if machine_id not in attempted_new_ids
            ],
        )
        raise


def destroy_replacements(app: str, machine_ids: list[str]) -> None:
    for machine_id in machine_ids:
        try:
            destroy_machine(app, machine_id)
        except Exception as error:
            print(
                f"failed to destroy replacement {machine_id}: {error}",
                file=sys.stderr,
            )


def drain_old_machines(app: str, machine_ids: list[str]) -> None:
    failures: list[str] = []
    for machine_id in machine_ids:
        try:
            machine = get_machine(app, machine_id)
            if is_started(machine) and supports_session_drain(machine):
                signal_machine(app, machine_id)
            elif is_started(machine):
                print(
                    f"leaving legacy machine {machine_id} cordoned until Fly auto-stops it",
                    file=sys.stderr,
                )
            elif is_stopped(machine):
                destroy_machine(app, machine_id)
            else:
                failures.append(f"{machine_id} is {machine.get('state')}")
        except Exception as error:
            failures.append(f"{machine_id}: {error}")

    if failures:
        raise DeployError(
            "traffic cut over, but old machine drain failed: " + "; ".join(failures)
        )


def bootstrap_deploy(app: str, config: str, dockerfile: str, version: str) -> None:
    fly(
        "deploy",
        "--app",
        app,
        "--config",
        config,
        "--dockerfile",
        dockerfile,
        "--remote-only",
        "--build-arg",
        f"APP_VERSION={version}",
    )


def build_and_push_image(app: str, config: str, dockerfile: str, version: str) -> str:
    image = image_ref(app, version)
    fly(
        "deploy",
        "--app",
        app,
        "--config",
        config,
        "--dockerfile",
        dockerfile,
        "--remote-only",
        "--build-only",
        "--push",
        "--image-label",
        f"api-{version}",
        "--build-arg",
        f"APP_VERSION={version}",
    )
    return image


def deploy(app: str, config: str, dockerfile: str, version: str) -> None:
    destroy_drained_machines(app)
    resume_draining_machines(app)
    machines = list_machines(app)
    serving = serving_machines(machines)
    if not machines:
        print("no machines present; running a bootstrap fly deploy", file=sys.stderr)
        bootstrap_deploy(app, config, dockerfile, version)
        return

    if not serving:
        raise SystemExit(
            "all machines are cordoned; wait for in-meeting drains to finish before deploying"
        )

    desired_stop_config = stop_config(config)
    image = build_and_push_image(app, config, dockerfile, version)
    old_ids = [machine["id"] for machine in serving]
    replacement_ids: list[str] = []
    try:
        for machine in serving:
            replacement_ids.append(
                create_replacement_machine(app, machine, image, desired_stop_config)
            )
        for machine_id in replacement_ids:
            wait_until_healthy(app, machine_id)
        validate_serving_set(app, set(old_ids), set(replacement_ids))
    except Exception:
        destroy_replacements(app, replacement_ids)
        raise

    cut_over(app, old_ids, replacement_ids)
    drain_old_machines(app, old_ids)
    destroy_drained_machines(app)
    print("deployed new machines; old meetings will keep their current connections")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--app", required=True)
    parser.add_argument("--config", required=True)
    parser.add_argument("--dockerfile", required=True)
    parser.add_argument("--version", required=True)
    args = parser.parse_args()
    deploy(args.app, args.config, args.dockerfile, args.version)


if __name__ == "__main__":
    main()
