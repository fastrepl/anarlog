#!/usr/bin/env python3

import sys
from pathlib import Path
from tempfile import NamedTemporaryFile
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

import deploy_api_drain
from deploy_api_drain import (
    DeployError,
    FlyApiError,
    change_machine_routing,
    checks_passing,
    create_replacement_machine,
    cut_over,
    image_ref,
    is_cordoned,
    is_started,
    is_stopped,
    replacement_config,
    serving_machines,
    stop_config,
    stopped_cordoned_machines,
    supports_session_drain,
    validate_serving_set,
)


def test_classifies_serving_and_drained_machines():
    machines = [
        {"id": "serving-started", "state": "started", "cordoned": False},
        {"id": "serving-idle", "state": "stopped", "cordoned": False},
        {"id": "draining", "state": "started", "cordoned": True},
        {"id": "drained", "state": "stopped", "cordoned": True},
        {"id": "deleted", "state": "destroyed", "cordoned": True},
    ]

    assert [machine["id"] for machine in serving_machines(machines)] == [
        "serving-started",
        "serving-idle",
    ]
    assert [machine["id"] for machine in stopped_cordoned_machines(machines)] == [
        "drained"
    ]
    assert is_started(machines[0])
    assert is_stopped(machines[1])
    assert not is_stopped(machines[4])
    assert is_cordoned(machines[2])
    assert not is_cordoned(machines[0])


def test_reads_cordon_from_metadata_when_top_level_flag_is_absent():
    machine = {
        "id": "legacy",
        "state": "stopped",
        "config": {"metadata": {"fly_cordoned": "true"}},
    }
    assert is_cordoned(machine)
    assert stopped_cordoned_machines([machine]) == [machine]


def test_checks_passing_requires_every_reported_check():
    assert not checks_passing({"state": "started"})
    assert checks_passing(
        {
            "state": "started",
            "checks": {
                "http": {"status": "passing"},
                "tcp": {"Status": "success"},
            },
        }
    )
    assert not checks_passing(
        {"state": "started", "checks": [{"status": "passing"}, {"status": "critical"}]}
    )
    assert not checks_passing(
        {
            "state": "started",
            "host_status": "unreachable",
            "checks": [{"status": "passing"}],
        }
    )
    assert not checks_passing({"state": "stopped", "checks": [{"status": "passing"}]})


def test_image_ref_uses_the_fly_registry_tag():
    assert image_ref("anarlog-ai", "1.4.14") == "registry.fly.io/anarlog-ai:api-1.4.14"


def test_stop_config_reads_graceful_shutdown_settings():
    with NamedTemporaryFile("wb") as config:
        config.write(b"kill_signal = 'SIGTERM'\nkill_timeout = 300\n")
        config.flush()
        assert stop_config(config.name) == {
            "signal": "SIGTERM",
            "timeout": "300s",
        }


def test_replacement_config_updates_image_without_mutating_source():
    machine = {
        "id": "old",
        "config": {
            "image": "registry.fly.io/anarlog-ai:old",
            "metadata": {
                "fly_cordoned": "true",
                "fly_process_group": "app",
            },
        },
    }

    config = replacement_config(
        machine,
        "registry.fly.io/anarlog-ai:new",
        {"signal": "SIGTERM", "timeout": "300s"},
    )

    assert config["image"] == "registry.fly.io/anarlog-ai:new"
    assert config["metadata"] == {
        "anarlog_drain_protocol": "sigusr1-v1",
        "fly_process_group": "app",
    }
    assert config["restart"] == {"policy": "on-failure"}
    assert config["stop_config"] == {"signal": "SIGTERM", "timeout": "300s"}
    assert supports_session_drain({"config": config})
    assert machine["config"]["image"] == "registry.fly.io/anarlog-ai:old"
    assert machine["config"]["metadata"]["fly_cordoned"] == "true"


def test_replacement_config_rejects_volume_mounts():
    machine = {
        "id": "old",
        "config": {
            "image": "registry.fly.io/anarlog-ai:old",
            "mounts": [{"volume": "vol_123", "path": "/data"}],
        },
    }

    try:
        replacement_config(
            machine,
            "registry.fly.io/anarlog-ai:new",
            {"signal": "SIGTERM", "timeout": "300s"},
        )
    except DeployError as error:
        assert "volume mounts" in str(error)
    else:
        raise AssertionError("expected volume-backed machine to be rejected")


def test_replacement_config_rejects_unhealthy_hosts():
    machine = {
        "id": "old",
        "host_status": "unreachable",
        "config": {"image": "registry.fly.io/anarlog-ai:old"},
    }

    try:
        replacement_config(
            machine,
            "registry.fly.io/anarlog-ai:new",
            {"signal": "SIGTERM", "timeout": "300s"},
        )
    except DeployError as error:
        assert "unhealthy host" in str(error)
    else:
        raise AssertionError("expected an unreachable machine host to be rejected")


def test_create_replacement_starts_cordoned_in_the_source_region():
    machine = {
        "id": "old",
        "region": "sjc",
        "config": {"image": "registry.fly.io/anarlog-ai:old"},
    }
    calls = []

    def fake_api_request(method, path, payload=None, query=None):
        calls.append((method, path, payload, query))
        return {"id": "new"}

    with patch.object(deploy_api_drain, "api_request", fake_api_request):
        created = create_replacement_machine(
            "anarlog-ai",
            machine,
            "registry.fly.io/anarlog-ai:new",
            {"signal": "SIGTERM", "timeout": "300s"},
        )

    assert created == "new"
    assert calls == [
        (
            "POST",
            "/apps/anarlog-ai/machines",
            {
                "config": {
                    "image": "registry.fly.io/anarlog-ai:new",
                    "metadata": {
                        "anarlog_drain_protocol": "sigusr1-v1",
                    },
                    "restart": {"policy": "on-failure"},
                    "stop_config": {"signal": "SIGTERM", "timeout": "300s"},
                },
                "skip_service_registration": True,
                "region": "sjc",
            },
            None,
        )
    ]


def test_validate_serving_set_requires_cordoned_replacements():
    machines = [
        {"id": "old", "state": "started", "cordoned": False},
        {"id": "new", "state": "started", "cordoned": False},
    ]

    with patch.object(deploy_api_drain, "list_machines", lambda _app: machines):
        try:
            validate_serving_set("anarlog-ai", {"old"}, {"new"})
        except DeployError as error:
            assert "not safely cordoned" in str(error)
        else:
            raise AssertionError("expected an uncordoned replacement to be rejected")


def test_cut_over_registers_new_machines_before_cordoning_old_machines():
    operations = []

    with (
        patch.object(
            deploy_api_drain,
            "cordon_machine",
            lambda _app, machine_id: operations.append(("cordon", machine_id)),
        ),
        patch.object(
            deploy_api_drain,
            "uncordon_machine",
            lambda _app, machine_id: operations.append(("uncordon", machine_id)),
        ),
    ):
        cut_over(
            "anarlog-ai",
            ["old-a", "old-b"],
            ["new-a", "new-b"],
            propagation_seconds=0,
        )

    assert operations == [
        ("uncordon", "new-a"),
        ("uncordon", "new-b"),
        ("cordon", "old-a"),
        ("cordon", "old-b"),
    ]


def test_cut_over_drains_an_attempted_replacement_when_activation_fails():
    operations = []

    def uncordon(_app, machine_id):
        operations.append(("uncordon", machine_id))
        if machine_id == "new":
            raise DeployError("uncordon failed")

    with (
        patch.object(
            deploy_api_drain,
            "cordon_machine",
            lambda _app, machine_id: operations.append(("cordon", machine_id)),
        ),
        patch.object(deploy_api_drain, "uncordon_machine", uncordon),
        patch.object(
            deploy_api_drain,
            "signal_machine",
            lambda _app, machine_id: operations.append(("signal", machine_id)),
        ),
    ):
        try:
            cut_over(
                "anarlog-ai",
                ["old"],
                ["new"],
                propagation_seconds=0,
            )
        except DeployError:
            pass
        else:
            raise AssertionError("expected cutover to fail")

    assert operations == [
        ("uncordon", "new"),
        ("cordon", "new"),
        ("signal", "new"),
    ]


def test_cut_over_restores_old_routing_before_draining_replacements():
    operations = []

    def cordon(_app, machine_id):
        operations.append(("cordon", machine_id))
        if machine_id == "old":
            raise DeployError("cordon failed")

    with (
        patch.object(deploy_api_drain, "cordon_machine", cordon),
        patch.object(
            deploy_api_drain,
            "uncordon_machine",
            lambda _app, machine_id: operations.append(("uncordon", machine_id)),
        ),
        patch.object(
            deploy_api_drain,
            "signal_machine",
            lambda _app, machine_id: operations.append(("signal", machine_id)),
        ),
    ):
        try:
            cut_over(
                "anarlog-ai",
                ["old"],
                ["new"],
                propagation_seconds=0,
            )
        except DeployError:
            pass
        else:
            raise AssertionError("expected cutover to fail")

    assert operations == [
        ("uncordon", "new"),
        ("cordon", "old"),
        ("uncordon", "old"),
        ("cordon", "new"),
        ("signal", "new"),
    ]


def test_routing_changes_retry_transient_api_failures():
    calls = []

    def fake_api_request(method, path):
        calls.append((method, path))
        if len(calls) == 1:
            raise FlyApiError("timeout", 408)

    with (
        patch.object(deploy_api_drain, "api_request", fake_api_request),
        patch.object(deploy_api_drain.time, "sleep") as sleep,
    ):
        change_machine_routing("POST", "/machines/new/uncordon")

    assert calls == [
        ("POST", "/machines/new/uncordon"),
        ("POST", "/machines/new/uncordon"),
    ]
    sleep.assert_called_once_with(0.5)


def test_partial_replacement_failure_destroys_created_machines():
    machines = [
        {
            "id": "old-a",
            "state": "started",
            "cordoned": False,
            "config": {"image": "old"},
        },
        {
            "id": "old-b",
            "state": "started",
            "cordoned": False,
            "config": {"image": "old"},
        },
    ]
    destroyed = []

    def create_replacement(_app, machine, _image, _stop_config):
        if machine["id"] == "old-b":
            raise DeployError("launch failed")
        return "new-a"

    with (
        patch.object(deploy_api_drain, "destroy_drained_machines"),
        patch.object(deploy_api_drain, "resume_draining_machines"),
        patch.object(deploy_api_drain, "list_machines", return_value=machines),
        patch.object(
            deploy_api_drain,
            "stop_config",
            return_value={"signal": "SIGTERM", "timeout": "300s"},
        ),
        patch.object(
            deploy_api_drain,
            "build_and_push_image",
            return_value="registry.fly.io/anarlog-ai:new",
        ),
        patch.object(
            deploy_api_drain,
            "create_replacement_machine",
            side_effect=create_replacement,
        ),
        patch.object(
            deploy_api_drain,
            "destroy_machine",
            side_effect=lambda _app, machine_id: destroyed.append(machine_id),
        ),
        patch.object(deploy_api_drain, "cut_over") as cut_over_mock,
    ):
        try:
            deploy_api_drain.deploy(
                "anarlog-ai",
                "apps/api/fly.toml",
                "apps/api/Dockerfile",
                "1.4.14",
            )
        except DeployError as error:
            assert "launch failed" in str(error)
        else:
            raise AssertionError("expected replacement launch to fail")

    assert destroyed == ["new-a"]
    cut_over_mock.assert_not_called()


def test_drain_only_signals_machines_with_protocol_support():
    machines = {
        "legacy": {
            "id": "legacy",
            "state": "started",
            "cordoned": True,
            "config": {},
        },
        "supported": {
            "id": "supported",
            "state": "started",
            "cordoned": True,
            "config": {
                "metadata": {"anarlog_drain_protocol": "sigusr1-v1"},
            },
        },
        "stopped": {
            "id": "stopped",
            "state": "stopped",
            "cordoned": True,
            "config": {},
        },
    }
    signaled = []
    destroyed = []

    with (
        patch.object(
            deploy_api_drain,
            "get_machine",
            side_effect=lambda _app, machine_id: machines[machine_id],
        ),
        patch.object(
            deploy_api_drain,
            "signal_machine",
            side_effect=lambda _app, machine_id: signaled.append(machine_id),
        ),
        patch.object(
            deploy_api_drain,
            "destroy_machine",
            side_effect=lambda _app, machine_id: destroyed.append(machine_id),
        ),
    ):
        deploy_api_drain.drain_old_machines(
            "anarlog-ai",
            ["legacy", "supported", "stopped"],
        )

    assert signaled == ["supported"]
    assert destroyed == ["stopped"]


def test_resume_only_signals_supported_draining_machines():
    machines = [
        {
            "id": "legacy",
            "state": "started",
            "cordoned": True,
            "config": {},
        },
        {
            "id": "supported",
            "state": "started",
            "cordoned": True,
            "config": {
                "metadata": {"anarlog_drain_protocol": "sigusr1-v1"},
            },
        },
    ]
    signaled = []

    with (
        patch.object(deploy_api_drain, "list_machines", return_value=machines),
        patch.object(
            deploy_api_drain,
            "signal_machine",
            side_effect=lambda _app, machine_id: signaled.append(machine_id),
        ),
    ):
        deploy_api_drain.resume_draining_machines("anarlog-ai")

    assert signaled == ["supported"]


if __name__ == "__main__":
    test_classifies_serving_and_drained_machines()
    test_reads_cordon_from_metadata_when_top_level_flag_is_absent()
    test_checks_passing_requires_every_reported_check()
    test_image_ref_uses_the_fly_registry_tag()
    test_stop_config_reads_graceful_shutdown_settings()
    test_replacement_config_updates_image_without_mutating_source()
    test_replacement_config_rejects_volume_mounts()
    test_replacement_config_rejects_unhealthy_hosts()
    test_create_replacement_starts_cordoned_in_the_source_region()
    test_validate_serving_set_requires_cordoned_replacements()
    test_cut_over_registers_new_machines_before_cordoning_old_machines()
    test_cut_over_drains_an_attempted_replacement_when_activation_fails()
    test_cut_over_restores_old_routing_before_draining_replacements()
    test_routing_changes_retry_transient_api_failures()
    test_partial_replacement_failure_destroys_created_machines()
    test_drain_only_signals_machines_with_protocol_support()
    test_resume_only_signals_supported_draining_machines()
    print("ok")
