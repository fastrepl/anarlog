import contextlib
import importlib.util
import io
import os
import pathlib
import sys
import tempfile
import unittest
from unittest import mock


MODULE_PATH = pathlib.Path(__file__).with_name("verify_cloudsync_e2ee.py")
SPEC = importlib.util.spec_from_file_location("verify_cloudsync_e2ee", MODULE_PATH)
assert SPEC and SPEC.loader
verify = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(verify)


class VerifyCloudSyncE2eeTests(unittest.TestCase):
    def test_requires_all_replica_verification_secrets(self) -> None:
        all_secrets = [
            {"key": key, "value": "set"} for key in verify.REPLICA_SECRETS
        ] + [{"key": "ANARLOG_SQLITECLOUD_MODE", "value": "retired"}]
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory, "secrets.json")
            path.write_text(verify.json.dumps(all_secrets))
            with (
                mock.patch.object(sys, "argv", ["verify", str(path)]),
                mock.patch.object(verify, "verify_replica") as replica,
                contextlib.redirect_stdout(io.StringIO()),
            ):
                verify.main()
            self.assertTrue(replica.called)

            path.write_text(verify.json.dumps(all_secrets[:-2]))
            with (
                mock.patch.object(sys, "argv", ["verify", str(path)]),
                self.assertRaisesRegex(ValueError, "ANARLOG_REPLICA_VERIFY_PASSWORD_B"),
            ):
                verify.main()

            path.write_text(verify.json.dumps([]))
            with (
                mock.patch.object(sys, "argv", ["verify", str(path)]),
                self.assertRaisesRegex(ValueError, "ANARLOG_REPLICA_VERIFY_API_URL"),
            ):
                verify.main()

    def test_rejects_missing_or_extra_arguments(self) -> None:
        for argv in (["verify"], ["verify", "--replica-only", "secrets.json"]):
            with (
                mock.patch.object(sys, "argv", argv),
                self.assertRaisesRegex(ValueError, "usage:"),
            ):
                verify.main()

    def test_replica_verification_signs_in_with_service_role_and_sanitizes_env(
        self,
    ) -> None:
        values = {key: "set" for key in verify.REPLICA_SECRETS} | {
            "ANARLOG_REPLICA_VERIFY_API_URL": "https://sync.example/",
            "ANARLOG_REPLICA_VERIFY_SUPABASE_URL": "http://127.0.0.1:54321",
        }

        def respond(url, _bearer, _label, body=None, headers=None):
            self.assertEqual(
                url, "http://127.0.0.1:54321/auth/v1/token?grant_type=password"
            )
            # Service credentials are what let an unattended job past the captcha.
            self.assertEqual(headers, {"apikey": "set", "Authorization": "Bearer set"})
            return {"access_token": "token-for-" + body["email"]}

        with (
            mock.patch.object(verify, "request_json", side_effect=respond) as request,
            mock.patch.object(verify.subprocess, "run") as cargo,
            mock.patch.dict(
                os.environ,
                {"FLY_API_TOKEN": "deploy-token", "CUSTOM_SECRET": "secret"},
            ),
        ):
            verify.verify_replica(values)

        self.assertEqual(request.call_count, 2)
        environment = cargo.call_args.kwargs["env"]
        self.assertEqual(environment["ANARLOG_REPLICA_API_URL"], "https://sync.example")
        self.assertNotIn("ANARLOG_REPLICA_SQLITECLOUD_MODE", environment)
        self.assertEqual(environment["ANARLOG_REPLICA_ACCESS_TOKEN_A"], "token-for-set")
        self.assertEqual(environment["ANARLOG_REPLICA_ACCESS_TOKEN_B"], "token-for-set")
        self.assertNotIn("FLY_API_TOKEN", environment)
        self.assertNotIn("CUSTOM_SECRET", environment)
        self.assertIn(verify.REPLICA_TEST, cargo.call_args.args[0])

        for bad_url in (
            "http://sync.example",
            "https://sync.example/path",
            "https://user:pass@sync.example",
        ):
            with self.assertRaisesRegex(ValueError, "HTTPS"):
                verify.verify_replica(
                    values | {"ANARLOG_REPLICA_VERIFY_API_URL": bad_url}
                )


if __name__ == "__main__":
    unittest.main()
