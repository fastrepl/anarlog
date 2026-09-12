#!/usr/bin/env python3

import sys
from pathlib import Path
from tempfile import NamedTemporaryFile
from unittest.mock import call, patch

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
    first = image_ref("anarlog-ai", "1.4.14")
    second = image_ref("anarlog-ai", "1.4.14")
    assert first.startswith("registry.fly.io/anarlog-ai:api-1.4.14-")
    assert first != second
    with patch.object(deploy_api_drain, "fly") as build:
        image = deploy_api_drain.build_and_push_image(
            "anarlog-ai", "config", "Dockerfile", "1.4.14"
        )
    args = build.call_args.args
    assert args[args.index("--image-label") + 1] == image.rsplit(":", 1)[1]


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
        drain_supported=True,
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
            drain_supported=True,
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


def test_create_replacement_retries_until_image_manifest_is_available():
    machine = {
        "id": "old",
        "region": "sjc",
        "config": {"image": "registry.fly.io/anarlog-ai:old"},
    }
    calls = []

    def fake_api_request(method, path, payload=None, query=None):
        calls.append((method, path, payload, query))
        if len(calls) == 1:
            raise FlyApiError(
                "failed to get manifest: MANIFEST_UNKNOWN manifest unknown",
                400,
            )
        return {"id": "new"}

    with (
        patch.object(deploy_api_drain, "api_request", fake_api_request),
        patch.object(deploy_api_drain.time, "sleep") as sleep,
    ):
        created = create_replacement_machine(
            "anarlog-ai",
            machine,
            "registry.fly.io/anarlog-ai:new",
            {"signal": "SIGTERM", "timeout": "300s"},
        )

    assert created == "new"
    assert len(calls) == 2
    sleep.assert_called_once_with(0.5)


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
            "get_machine",
            side_effect=AssertionError("Rollback must not depend on a status lookup"),
        ),
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
                verified_drain_ids={"new"},
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


def test_cut_over_never_signals_an_unverified_replacement():
    with (
        patch.object(
            deploy_api_drain, "uncordon_machine", side_effect=DeployError("timeout")
        ),
        patch.object(deploy_api_drain, "cordon_machine") as cordon,
        patch.object(deploy_api_drain, "get_machine", return_value={"config": {}}),
        patch.object(deploy_api_drain, "signal_machine") as signal,
        patch.object(deploy_api_drain, "destroy_machine") as destroy,
    ):
        try:
            cut_over("anarlog-ai", ["old"], ["new"], propagation_seconds=0)
        except DeployError:
            pass
        else:
            raise AssertionError("Expected failed activation")
        cordon.assert_called_once_with("anarlog-ai", "new")
        signal.assert_not_called()
        destroy.assert_not_called()


def test_cut_over_restores_old_routing_before_draining_replacements():
    operations = []

    def cordon(_app, machine_id):
        operations.append(("cordon", machine_id))
        if machine_id in {"old", "new"}:
            raise DeployError("cordon failed")

    with (
        patch.object(
            deploy_api_drain,
            "get_machine",
            side_effect=AssertionError("Rollback must not depend on a status lookup"),
        ),
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
                verified_drain_ids={"new"},
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

    def create_replacement(
        _app, machine, _image, _stop_config, _runtime_config, _supported
    ):
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


def test_drain_leaves_already_stopping_cordoned_machines_alone():
    with (
        patch.object(
            deploy_api_drain,
            "get_machine",
            return_value={"state": "stopping", "cordoned": True},
        ),
        patch.object(deploy_api_drain, "signal_machine") as signal,
        patch.object(deploy_api_drain, "destroy_machine") as destroy,
    ):
        deploy_api_drain.drain_old_machines("anarlog-ai", ["old"])

    signal.assert_not_called()
    destroy.assert_not_called()


def test_drain_rejects_unexpected_machine_states():
    for machine in (
        {"state": "stopping", "cordoned": False},
        {"state": "starting", "cordoned": True},
    ):
        with patch.object(deploy_api_drain, "get_machine", return_value=machine):
            try:
                deploy_api_drain.drain_old_machines("anarlog-ai", ["old"])
            except DeployError as error:
                assert f"old is {machine['state']}" in str(error)
            else:
                raise AssertionError("expected the unexpected state to fail deployment")


def test_desired_runtime_replaces_stale_machine_settings():
    desired = deploy_api_drain.desired_runtime_config("anarlog-ai", "apps/api/fly.toml")
    old = {
        "id": "old",
        "config": {
            "env": {"ANARLOG_SERVICE": "sync", "REMOVED_SETTING": "stale"},
            "guest": {"memory_mb": 256},
            "checks": {"stale": {"path": "/wrong"}},
            "services": [{"internal_port": 9999}],
            "metadata": {"fly_process_group": "app"},
        },
    }
    result = replacement_config(old, "new", {"signal": "SIGTERM"}, desired)
    assert result["env"] == {
        "ANARLOG_ATTACHMENT_BACKUP_GC_ENABLED": "false",
        "ANARLOG_AI_ORIGIN": "https://anarlog-inference.fly.dev",
        "ANARLOG_SYNC_ORIGIN": "https://anarlog-sync.fly.dev",
        "ANARLOG_CORE_ORIGIN": "https://anarlog-core.fly.dev",
        "ANARLOG_BILLING_ORIGIN": "https://anarlog-billing-api.fly.dev",
        "PORT": "3001",
        "PRIMARY_REGION": "sjc",
    }
    assert result["guest"] == {"memory_mb": 1024, "cpu_kind": "shared", "cpus": 1}
    assert result["checks"] == {}
    (service,) = result["services"]
    assert service["internal_port"] == 3001
    assert service["checks"][0]["path"] == "/health/ready/api"
    assert service["checks"][0]["type"] == "http"
    assert service["ports"][1]["http_options"]["idle_timeout"] == 660
    assert service["autostop"] == "off"
    assert result["swap_size_mb"] == 512
    assert old["config"]["env"]["REMOVED_SETTING"] == "stale"
    result["env"]["PORT"] = "1234"
    assert desired["env"]["PORT"] == "3001"


def test_bootstrap_marks_healthy_machines_for_future_drains():
    with (
        patch.object(deploy_api_drain, "fly"),
        patch.object(deploy_api_drain, "list_machines", return_value=[{"id": "new"}]),
        patch.object(deploy_api_drain, "wait_until_healthy") as healthy,
        patch.object(deploy_api_drain, "api_request") as api,
    ):
        deploy_api_drain.bootstrap_deploy(
            "anarlog-inference", "config", "Dockerfile", "test"
        )
        healthy.assert_called_once_with("anarlog-inference", "new")
        api.assert_called_once_with(
            "POST",
            "/apps/anarlog-inference/machines/new/metadata/anarlog_drain_protocol",
            {"value": "sigusr1-v1"},
        )


def test_drain_adoption_only_marks_the_verified_image():
    digest = "sha256:" + "a" * 64
    with (
        patch.object(
            deploy_api_drain,
            "list_machines",
            return_value=[
                {"id": "verified", "image_ref": {"digest": digest}},
                {"id": "legacy", "image_ref": {"digest": "sha256:" + "b" * 64}},
            ],
        ),
        patch.object(deploy_api_drain, "mark_drain_supported") as mark,
    ):
        deploy_api_drain.adopt_drain_image("anarlog-inference", digest)
        mark.assert_called_once_with("anarlog-inference", "verified")


def test_stripe_replacement_migrates_process_group_and_keeps_capacity():
    desired = deploy_api_drain.desired_runtime_config(
        "hyprnote-stripe", "apps/stripe/fly.toml"
    )
    old = {"config": {"metadata": {"fly_process_group": "web", "custom": "keep"}}}
    result = replacement_config(old, "new", {"signal": "SIGTERM"}, desired)
    assert result["metadata"]["fly_process_group"] == "app"
    assert result["metadata"]["custom"] == "keep"
    assert old["config"]["metadata"]["fly_process_group"] == "web"
    (service,) = result["services"]
    assert service["internal_port"] == 8080
    assert service["min_machines_running"] == 2
    assert service["autostop"] == "off"
    assert service["checks"][0]["path"] == "/health"


def test_invalid_config_fails_before_any_machine_mutation():
    base = Path("apps/api/fly.toml").read_text()
    invalid_configs = [
        base.replace("anarlog-ai", "different-app"),
        base + "\n[deploy]\nrelease_command = 'unsafe-migration'\n",
        base.replace(
            "internal_port = 3001", "internal_port = 3001\nunknown_setting = true"
        ),
        base.replace("processes = ['app']", "processes = ['other']"),
    ]
    for invalid in invalid_configs:
        with (
            NamedTemporaryFile("w") as config,
            patch.object(deploy_api_drain, "api_request") as api,
            patch.object(deploy_api_drain, "fly") as fly,
        ):
            config.write(invalid)
            config.flush()
            try:
                deploy_api_drain.deploy("anarlog-ai", config.name, "Dockerfile", "test")
            except DeployError:
                pass
            else:
                raise AssertionError("invalid config was accepted")
            api.assert_not_called()
            fly.assert_not_called()


def test_standalone_profiles_have_role_checks_and_no_duplicate_cleanup():
    for role, app, health in [
        ("ai", "anarlog-inference", "ai"),
        ("sync", "anarlog-sync", "sync"),
        ("core", "anarlog-core", "core"),
        ("billing", "anarlog-billing-api", "billing-api"),
    ]:
        config = deploy_api_drain.desired_runtime_config(
            app, f"apps/api/fly.{role}.toml"
        )
        assert config["env"]["ANARLOG_SERVICE"] == role
        assert config["env"]["ANARLOG_ATTACHMENT_BACKUP_GC_ENABLED"] == (
            "true" if role == "core" else "false"
        )
        (service,) = config["services"]
        assert service["checks"][0]["path"] == f"/health/ready/{health}"
        assert service["min_machines_running"] == 2
        assert service["autostop"] == "off"


def test_cutover_preflight_rechecks_candidate_readiness():
    machines = [{"id": "old", "cordoned": False}, {"id": "new", "cordoned": True}]
    with (
        patch.object(deploy_api_drain, "list_machines", return_value=machines),
        patch.object(deploy_api_drain, "get_machine") as get,
    ):
        get.return_value = {"state": "started", "checks": [{"status": "passing"}]}
        validate_serving_set("anarlog-ai", {"old"}, {"new"})
        get.return_value = {"state": "started", "checks": [{"status": "critical"}]}
        try:
            validate_serving_set("anarlog-ai", {"old"}, {"new"})
        except DeployError as error:
            assert "lost readiness" in str(error)
        else:
            raise AssertionError("unhealthy candidate was accepted")


def test_deploy_rejects_stale_image_before_cutover():
    old = {"id": "old", "region": "sjc", "cordoned": False, "config": {"image": "old"}}
    with (
        patch.object(deploy_api_drain, "destroy_drained_machines"),
        patch.object(deploy_api_drain, "resume_draining_machines"),
        patch.object(deploy_api_drain, "list_machines", return_value=[old]),
        patch.object(
            deploy_api_drain,
            "build_and_push_image",
            return_value="registry.fly.io/anarlog-core:new",
        ),
        patch.object(
            deploy_api_drain,
            "create_replacement_machine",
            side_effect=["new-a", "new-b"],
        ),
        patch.object(deploy_api_drain, "wait_until_healthy"),
        patch.object(
            deploy_api_drain,
            "get_machine",
            return_value={
                "image_ref": {
                    "tag": "new",
                    "digest": "sha256:" + "a" * 64,
                    "labels": {"GH_SHA": "old"},
                }
            },
        ),
        patch.dict(deploy_api_drain.os.environ, {"GITHUB_SHA": "new"}),
        patch.object(deploy_api_drain, "destroy_replacements") as cleanup,
        patch.object(deploy_api_drain, "cut_over") as cutover,
    ):
        try:
            deploy_api_drain.deploy(
                "anarlog-core", "apps/api/fly.core.toml", "Dockerfile", "test"
            )
        except DeployError as error:
            assert "unexpected image" in str(error)
        else:
            raise AssertionError("Stale source image was accepted")
    cleanup.assert_called_once_with("anarlog-core", ["new-a", "new-b"])
    cutover.assert_not_called()


def test_replacement_digest_is_checked_for_immutable_deploys():
    digest = "sha256:" + "a" * 64
    with patch.object(
        deploy_api_drain, "get_machine", return_value={"image_ref": {"digest": digest}}
    ):
        deploy_api_drain.verify_replacement_image(
            "anarlog-core", "new", "registry.fly.io/anarlog-core@" + digest
        )
        for wrong in ["sha256:" + "b" * 64, "invalid"]:
            try:
                deploy_api_drain.verify_replacement_image(
                    "anarlog-core", "new", "registry.fly.io/anarlog-core@" + wrong
                )
            except DeployError:
                pass
            else:
                raise AssertionError("Mismatched image digest was accepted")


def test_deploy_restores_minimum_primary_region_capacity():
    old = {"id": "old", "region": "nrt", "cordoned": False, "config": {"image": "old"}}
    regions = []

    def create(_app, machine, _image, _stop, _runtime, _supported):
        regions.append(machine["region"])
        return f"new-{len(regions)}"

    with (
        patch.object(deploy_api_drain, "destroy_drained_machines"),
        patch.object(deploy_api_drain, "resume_draining_machines"),
        patch.object(deploy_api_drain, "list_machines", return_value=[old]),
        patch.object(deploy_api_drain, "build_and_push_image", return_value="new"),
        patch.object(
            deploy_api_drain, "create_replacement_machine", side_effect=create
        ),
        patch.object(deploy_api_drain, "wait_until_healthy"),
        patch.object(deploy_api_drain, "verify_replacement_image"),
        patch.object(deploy_api_drain, "validate_serving_set"),
        patch.object(deploy_api_drain, "cut_over") as cutover,
        patch.object(deploy_api_drain, "drain_old_machines"),
    ):
        deploy_api_drain.deploy(
            "anarlog-sync", "apps/api/fly.sync.toml", "Dockerfile", "test"
        )
    assert regions == ["nrt", "sjc", "sjc"]
    cutover.assert_called_once_with(
        "anarlog-sync",
        ["old"],
        ["new-1", "new-2", "new-3"],
        verified_drain_ids={"new-1", "new-2", "new-3"},
    )


def test_rollback_requires_an_immutable_api_image_before_mutating_machines():
    for image in [
        "registry.fly.io/anarlog-sync:latest",
        "registry.fly.io/unrelated@sha256:" + "a" * 64,
    ]:
        with patch.object(deploy_api_drain, "api_request") as api:
            try:
                deploy_api_drain.deploy(
                    "anarlog-sync",
                    "apps/api/fly.sync.toml",
                    "Dockerfile",
                    "test",
                    image,
                )
            except DeployError as error:
                assert "immutable digest" in str(error)
            else:
                raise AssertionError("unsafe rollback image accepted")
            api.assert_not_called()


def test_idle_legacy_stripe_retirement_requires_cordon_and_healthy_capacity():
    target = {"id": "old", "state": "started", "cordoned": True}
    healthy = {"state": "started", "checks": [{"status": "passing"}]}
    machines = [
        target,
        {"id": "new-a", "state": "started"},
        {"id": "new-b", "state": "started"},
    ]
    with (
        patch.object(deploy_api_drain, "list_machines", return_value=machines),
        patch.object(
            deploy_api_drain,
            "get_machine",
            side_effect=lambda app, machine_id: {"state": "stopped"}
            if machine_id == "old"
            else healthy,
        ),
        patch.object(deploy_api_drain, "api_request") as api,
    ):
        deploy_api_drain.retire_idle_stripe_machine("old")
        assert api.call_args_list == [
            call(
                "POST",
                "/apps/hyprnote-stripe/machines/old/signal",
                {"signal": "SIGTERM"},
            ),
            call(
                "DELETE", "/apps/hyprnote-stripe/machines/old", query={"force": "false"}
            ),
        ]
        api.reset_mock()
        with patch.object(
            deploy_api_drain,
            "list_machines",
            return_value=[dict(target, state="stopped"), *machines[1:]],
        ):
            deploy_api_drain.retire_idle_stripe_machine("old")
        api.assert_called_once_with(
            "DELETE", "/apps/hyprnote-stripe/machines/old", query={"force": "false"}
        )
        with patch.object(deploy_api_drain, "list_machines", return_value=machines[1:]):
            api.reset_mock()
            deploy_api_drain.retire_idle_stripe_machine("old")
            api.assert_not_called()
        for unsafe in [
            [target, machines[1]],
            [dict(target, cordoned=False), *machines[1:]],
        ]:
            api.reset_mock()
            with patch.object(deploy_api_drain, "list_machines", return_value=unsafe):
                try:
                    deploy_api_drain.retire_idle_stripe_machine("old")
                except deploy_api_drain.DeployError:
                    pass
                else:
                    raise AssertionError("Unsafe retirement was accepted")
            api.assert_not_called()


def test_unknown_override_does_not_inherit_drain_support():
    machine = {
        "id": "old",
        "config": {"metadata": {"anarlog_drain_protocol": "sigusr1-v1"}},
    }
    config = replacement_config(machine, "unknown", {"signal": "SIGTERM"})
    assert not supports_session_drain({"config": config})
    with (
        patch.object(deploy_api_drain, "fly"),
        patch.object(deploy_api_drain, "list_machines", return_value=[{"id": "new"}]),
        patch.object(deploy_api_drain, "wait_until_healthy"),
        patch.object(deploy_api_drain, "mark_drain_supported") as mark,
    ):
        deploy_api_drain.bootstrap_deploy(
            "anarlog-inference", "config", "Dockerfile", "test", "unknown"
        )
        mark.assert_not_called()


def test_override_support_is_verified_before_cleanup():
    digest = "sha256:" + "a" * 64
    image = "registry.fly.io/anarlog-inference@" + digest
    for verified, explicit in [(False, False), (True, False), (False, True)]:
        old = {"id": "old", "region": "sjc", "config": {"image": "old"}}
        known = {
            "id": "retired",
            "state": "stopped",
            "cordoned": True,
            "image_ref": {"digest": digest},
            "config": {
                "metadata": {"anarlog_drain_protocol": "sigusr1-v1"} if verified else {}
            },
        }
        with (
            patch.object(
                deploy_api_drain, "list_machines", side_effect=[[old, known], [old]]
            ),
            patch.object(deploy_api_drain, "destroy_drained_machines") as cleanup,
            patch.object(deploy_api_drain, "resume_draining_machines") as resume,
            patch.object(
                deploy_api_drain,
                "create_replacement_machine",
                side_effect=["new-a", "new-b"],
            ) as create,
            patch.object(deploy_api_drain, "wait_until_healthy"),
            patch.object(deploy_api_drain, "verify_replacement_image"),
            patch.object(deploy_api_drain, "validate_serving_set"),
            patch.object(deploy_api_drain, "cut_over"),
            patch.object(deploy_api_drain, "drain_old_machines"),
        ):
            if not (verified or explicit):
                try:
                    deploy_api_drain.deploy(
                        "anarlog-inference",
                        "apps/api/fly.ai.toml",
                        "Dockerfile",
                        "test",
                        image,
                    )
                except DeployError as error:
                    assert "no verified session drain support" in str(error)
                else:
                    raise AssertionError("Unverified image accepted")
                cleanup.assert_not_called()
                resume.assert_not_called()
                create.assert_not_called()
                continue
            deploy_api_drain.deploy(
                "anarlog-inference",
                "apps/api/fly.ai.toml",
                "Dockerfile",
                "test",
                image,
                digest if explicit else None,
            )
            assert all(
                call.args[-1] is (verified or explicit)
                for call in create.call_args_list
            )


def test_idle_legacy_api_retirement_sends_only_graceful_signal():
    target = {"id": "old", "state": "started", "cordoned": True}
    healthy = {"state": "started", "checks": [{"status": "passing"}]}
    with (
        patch.object(
            deploy_api_drain,
            "list_machines",
            return_value=[target, {"id": "a"}, {"id": "b"}],
        ),
        patch.object(
            deploy_api_drain,
            "get_machine",
            side_effect=[healthy, healthy, {"state": "stopped"}],
        ),
        patch.object(deploy_api_drain, "api_request") as api,
    ):
        deploy_api_drain.retire_idle_legacy_machine("hyprnote-ai", "old")
        assert api.call_args_list == [
            call(
                "POST", "/apps/hyprnote-ai/machines/old/signal", {"signal": "SIGTERM"}
            ),
            call("DELETE", "/apps/hyprnote-ai/machines/old", query={"force": "false"}),
        ]
        api.reset_mock()
        try:
            deploy_api_drain.retire_idle_legacy_machine("anarlog-inference", "old")
        except DeployError:
            pass
        else:
            raise AssertionError("Non-legacy application accepted")
        api.assert_not_called()


def test_adoption_of_a_verified_candidate_requires_the_exact_digest():
    digest = "sha256:" + "a" * 64
    with (
        patch.object(deploy_api_drain, "list_machines", return_value=[]),
        patch.object(deploy_api_drain, "mark_drain_supported") as mark,
    ):
        deploy_api_drain.adopt_drain_image(
            "anarlog-ai", digest, "registry.fly.io/anarlog-core@" + digest
        )
        mark.assert_not_called()
        try:
            deploy_api_drain.adopt_drain_image(
                "anarlog-ai", digest, "registry.fly.io/anarlog-core@sha256:" + "b" * 64
            )
        except DeployError:
            pass
        else:
            raise AssertionError("Mismatched candidate was accepted")


def test_rollback_health_preserves_capacity_and_worker_ownership():
    import tomllib

    machine = {
        "config": {
            "env": {"ANARLOG_ATTACHMENT_BACKUP_GC_ENABLED": "true"},
            "services": [{"checks": [{"type": "http", "path": "/health"}]}],
        }
    }
    text = deploy_api_drain.rollback_health_config("apps/api/fly.gateway.toml", machine)
    profile = tomllib.loads(text)
    assert profile["http_service"]["checks"][0]["path"] == "/health"
    assert profile["http_service"]["min_machines_running"] == 2
    assert profile["env"]["ANARLOG_ATTACHMENT_BACKUP_GC_ENABLED"] == "false"
    assert profile["env"]["ANARLOG_AI_ORIGIN"] == "https://anarlog-inference.fly.dev"
    try:
        deploy_api_drain.rollback_health_config(
            "apps/api/fly.gateway.toml", {"config": {}}
        )
    except DeployError:
        pass
    else:
        raise AssertionError("Unknown original health contract accepted")


if __name__ == "__main__":
    test_rollback_health_preserves_capacity_and_worker_ownership()
    test_adoption_of_a_verified_candidate_requires_the_exact_digest()
    test_idle_legacy_api_retirement_sends_only_graceful_signal()
    test_override_support_is_verified_before_cleanup()
    test_unknown_override_does_not_inherit_drain_support()
    test_idle_legacy_stripe_retirement_requires_cordon_and_healthy_capacity()
    test_bootstrap_marks_healthy_machines_for_future_drains()
    test_drain_adoption_only_marks_the_verified_image()
    test_stripe_replacement_migrates_process_group_and_keeps_capacity()
    test_rollback_requires_an_immutable_api_image_before_mutating_machines()
    test_deploy_restores_minimum_primary_region_capacity()
    test_cutover_preflight_rechecks_candidate_readiness()
    test_standalone_profiles_have_role_checks_and_no_duplicate_cleanup()
    test_desired_runtime_replaces_stale_machine_settings()
    test_invalid_config_fails_before_any_machine_mutation()
    test_classifies_serving_and_drained_machines()
    test_reads_cordon_from_metadata_when_top_level_flag_is_absent()
    test_checks_passing_requires_every_reported_check()
    test_image_ref_uses_the_fly_registry_tag()
    test_deploy_rejects_stale_image_before_cutover()
    test_replacement_digest_is_checked_for_immutable_deploys()
    test_stop_config_reads_graceful_shutdown_settings()
    test_replacement_config_updates_image_without_mutating_source()
    test_replacement_config_rejects_volume_mounts()
    test_replacement_config_rejects_unhealthy_hosts()
    test_create_replacement_starts_cordoned_in_the_source_region()
    test_create_replacement_retries_until_image_manifest_is_available()
    test_validate_serving_set_requires_cordoned_replacements()
    test_cut_over_registers_new_machines_before_cordoning_old_machines()
    test_cut_over_drains_an_attempted_replacement_when_activation_fails()
    test_cut_over_never_signals_an_unverified_replacement()
    test_cut_over_restores_old_routing_before_draining_replacements()
    test_routing_changes_retry_transient_api_failures()
    test_partial_replacement_failure_destroys_created_machines()
    test_drain_only_signals_machines_with_protocol_support()
    test_resume_only_signals_supported_draining_machines()
    test_drain_leaves_already_stopping_cordoned_machines_alone()
    test_drain_rejects_unexpected_machine_states()
    print("ok")
