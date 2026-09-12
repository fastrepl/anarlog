"""Test rollout orchestration without provider credentials or network calls."""

import asyncio
import json
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import AsyncMock, MagicMock, patch

with patch.dict(
    sys.modules, {"httpx": MagicMock(), "websockets.asyncio.client": MagicMock()}
):
    import verify_api_continuity as verify


A = "registry.fly.io/anarlog-ai@sha256:" + "a" * 64
B = "registry.fly.io/anarlog-core@sha256:" + "b" * 64


def machine(identifier, image, *, cordoned=False, state="started"):
    return {
        "id": identifier,
        "state": state,
        "cordoned": cordoned,
        "image_ref": {"digest": image.split("@")[1]},
        "config": {
            "image": image,
            "metadata": {"anarlog_drain_protocol": "sigusr1-v1"},
            "services": [{"checks": [{"type": "http", "path": "/health"}]}],
        },
    }


class ContinuityTests(unittest.IsolatedAsyncioTestCase):
    async def exercise(self, *, explicit=False, wrong_image=False, fail_rollback=False):
        registry = [machine("original", B if explicit else A)]
        if explicit:
            registry.append(machine("retained", A, cordoned=True, state="stopped"))
        calls = []
        traffic_instances = []

        class Traffic:
            def __init__(self, *args, **kwargs):
                self.streams = []
                self.errors = []
                self.llms = self.health = self.business_reads = 1
                self.closed = asyncio.Event()
                self.monitor_finished = False
                traffic_instances.append(self)

            async def record(self, identifier):
                self.streams.append((None, {"id": identifier}))

            def check(self):
                pass

            async def requests(self):
                await self.closed.wait()
                self.monitor_finished = True

            async def hold(self, seconds, stage):
                pass

            async def close(self):
                for item in registry:
                    if item["cordoned"]:
                        item["state"] = "stopped"
                self.closed.set()

        def deploy(app, config, dockerfile, version, image, verified):
            profile = tomllib.loads(Path(config).read_text())
            calls.append((image, verified, profile))
            if fail_rollback and len(calls) == 2:
                raise verify.deploy.DeployError("replacement failed readiness")
            for item in registry:
                item["cordoned"] = True
            actual = A if wrong_image else image
            registry.extend(
                machine(f"stage-{len(calls)}-{n}", actual) for n in range(2)
            )

        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "result.json"
            args = SimpleNamespace(
                app="anarlog-ai",
                config="apps/api/fly.gateway.toml",
                dockerfile="unused",
                version="test",
                image=B,
                verified_image_digest=B.split("@")[1],
                rollback_image=A if explicit else None,
                secrets="unused",
                email="unused",
                output=str(output),
            )
            with (
                patch.object(verify, "Traffic", Traffic),
                patch.object(verify, "authenticate", AsyncMock(return_value="unused")),
                patch.object(verify, "event"),
                patch.object(
                    verify,
                    "verify_isolated_drain",
                    AsyncMock(return_value={"passed": True}),
                ) as isolation,
                patch.object(verify.deploy, "adopt_drain_image"),
                patch.object(
                    verify.deploy,
                    "list_machines",
                    side_effect=lambda app: registry.copy(),
                ),
                patch.object(verify.deploy, "deploy", side_effect=deploy),
            ):
                if wrong_image or fail_rollback:
                    with self.assertRaises((RuntimeError, verify.deploy.DeployError)):
                        await verify.run(args)
                else:
                    await verify.run(args)
            result = json.loads(output.read_text())
            if wrong_image or fail_rollback:
                isolation.assert_not_awaited()
            else:
                isolation.assert_awaited_once()
                self.assertTrue(result["isolated_drain"]["passed"])
        self.assertTrue(traffic_instances[0].closed.is_set())
        self.assertTrue(traffic_instances[0].monitor_finished)
        return calls, result

    async def test_rolls_back_with_original_health_and_current_ownership(self):
        calls, result = await self.exercise()
        self.assertTrue(result["passed"])
        self.assertEqual([call[0] for call in calls], [B, A, B])
        self.assertEqual(result["stages"], ["replacement", "rollback", "rollforward"])
        self.assertEqual(calls[1][1], A.split("@")[1])
        self.assertEqual(calls[1][2]["http_service"]["checks"][0]["path"], "/health")
        self.assertEqual(
            calls[1][2]["env"]["ANARLOG_ATTACHMENT_BACKUP_GC_ENABLED"], "false"
        )

    async def test_retries_against_a_retained_verified_original_image(self):
        calls, result = await self.exercise(explicit=True)
        self.assertTrue(result["passed"])
        self.assertEqual([call[0] for call in calls], [B, A, B])

    async def test_wrong_serving_image_fails_and_closes_traffic(self):
        calls, result = await self.exercise(wrong_image=True)
        self.assertFalse(result["passed"])
        self.assertEqual(len(calls), 1)
        self.assertEqual(result["stages"], [])

    async def test_failed_rollback_never_reports_success_or_rolls_forward(self):
        calls, result = await self.exercise(fail_rollback=True)
        self.assertFalse(result["passed"])
        self.assertEqual(len(calls), 2)
        self.assertEqual(result["stages"], ["replacement"])


class IsolatedDrainTests(unittest.IsolatedAsyncioTestCase):
    async def exercise(
        self, *, state="false 0", address="127.0.0.1:18080", traffic_error=False
    ):
        calls = []
        instances = []

        async def command(*args):
            calls.append(args)
            if args[:2] == ("docker", "port"):
                return address
            if args[:2] == ("docker", "inspect"):
                return state
            return ""

        class Traffic:
            def __init__(self, *args, **kwargs):
                self.closed = asyncio.Event()
                self.errors = ["LLM failed"] if traffic_error else []
                self.llms = self.health = 2
                instances.append(self)

            async def record(self, identifier):
                pass

            async def requests(self):
                await self.closed.wait()

            async def hold(self, seconds, stage):
                pass

            async def close(self):
                self.closed.set()

        response = MagicMock(status_code=200)
        response.json.return_value = {"draining": True, "active_streams": 1}
        client = AsyncMock()
        client.get.return_value = response
        context = AsyncMock()
        context.__aenter__.return_value = client
        with tempfile.NamedTemporaryFile(mode="w") as secrets:
            secrets.write("[]")
            secrets.flush()
            args = SimpleNamespace(image=B, secrets=secrets.name)
            with (
                patch.object(verify, "command", side_effect=command),
                patch.object(verify, "Traffic", Traffic),
                patch.object(verify, "event"),
                patch.object(verify.httpx, "AsyncClient", return_value=context),
                patch.object(verify.secrets_config, "select", return_value={}),
            ):
                if state != "false 0" or address != "127.0.0.1:18080" or traffic_error:
                    with self.assertRaises(RuntimeError):
                        await verify.verify_isolated_drain(
                            args, "token", b"audio", stop_timeout=0
                        )
                else:
                    result = await verify.verify_isolated_drain(
                        args, "token", b"audio", stop_timeout=0
                    )
                    self.assertTrue(result["passed"])
                    self.assertEqual(result["image"], B)
        self.assertEqual(calls[-1][:3], ("docker", "rm", "--force"))
        self.assertTrue(all(instance.closed.is_set() for instance in instances))
        return calls

    async def test_exact_image_is_loopback_only_and_exits_without_forced_signal(self):
        calls = await self.exercise()
        create = next(call for call in calls if call[:2] == ("docker", "create"))
        self.assertEqual(create[-1], B)
        self.assertEqual(create[create.index("--publish") + 1], "127.0.0.1::3001")
        signals = [call for call in calls if call[:2] == ("docker", "kill")]
        self.assertEqual([call[2:4] for call in signals], [("--signal", "SIGUSR1")])

    async def test_leaked_permit_fails_even_when_production_has_customer_sessions(self):
        await self.exercise(state="true 0")

    async def test_crash_is_not_successful_drain(self):
        await self.exercise(state="false 1")

    async def test_refuses_public_binding_and_removes_only_its_fixture(self):
        await self.exercise(address="0.0.0.0:18080")

    async def test_request_failure_still_closes_traffic_and_removes_fixture(self):
        await self.exercise(traffic_error=True)


if __name__ == "__main__":
    unittest.main()
