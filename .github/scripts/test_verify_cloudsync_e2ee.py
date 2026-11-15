import contextlib
import importlib.util
import io
import os
import pathlib
import sqlite3
import subprocess
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
    def test_failed_download_cleans_own_remote_fixtures_and_preserves_failure(
        self,
    ) -> None:
        workspace_a = "deploy-e2ee-a-00000000-0000-4000-8000-000000000001"
        workspace_b = "deploy-e2ee-b-00000000-0000-4000-8000-000000000002"
        environment = {
            "ANARLOG_CLOUDSYNC_WORKSPACE_A": workspace_a,
            "ANARLOG_CLOUDSYNC_WORKSPACE_B": workspace_b,
            "ANARLOG_CLOUDSYNC_TOKEN_A": "fixture-a-token",
            "ANARLOG_CLOUDSYNC_TOKEN_B": "fixture-b-token",
        }
        failure = subprocess.CalledProcessError(101, ["cargo", "test"])
        with sqlite3.connect(":memory:") as database:
            database.row_factory = sqlite3.Row
            database.execute("CREATE TABLE e2ee_records (workspace_id TEXT)")
            database.executemany(
                "INSERT INTO e2ee_records VALUES (?)",
                [(workspace_a,), (workspace_b,), ("unrelated-workspace",)],
            )

            def sql_request(_url, _token, _label, body):
                cursor = database.execute(body["sql"])
                return {
                    "data": [dict(row) for row in cursor.fetchall()]
                    if cursor.description
                    else {}
                }

            with (
                mock.patch.object(verify, "run_test", side_effect=[failure, None]),
                mock.patch.object(verify, "request_json", side_effect=sql_request),
            ):
                with self.assertRaises(subprocess.CalledProcessError) as raised:
                    verify.run_test_with_cleanup(
                        "replication-check",
                        environment,
                        "https://project.example",
                        "e2ee.sqlite",
                    )
            self.assertIs(raised.exception, failure)
            self.assertEqual(
                [
                    row[0]
                    for row in database.execute("SELECT workspace_id FROM e2ee_records")
                ],
                ["unrelated-workspace"],
            )

    def flagged_latency_checks(self, output: str) -> dict[str, set[str]]:
        levels: dict[str, set[str]] = {"notice": set(), "warning": set()}
        for line in output.splitlines():
            level, _, message = line.removeprefix("::").partition(" ")
            levels[level].add(message.split("::", 1)[1].split(":", 1)[0])
        return levels

    def test_slow_or_timed_out_bootstraps_are_flagged_without_failing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            log = pathlib.Path(directory, "latency.jsonl")
            log.write_text(
                '{"check": "fast", "seconds": 12.0, "completed": true}\n'
                '{"check": "slow", "seconds": 63.0, "completed": true}\n'
                '{"check": "stuck", "seconds": 180.0, "completed": false}\n'
            )
            output = io.StringIO()
            with (
                mock.patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": ""}),
                mock.patch.object(verify, "FRESH_DEVICE_LATENCY_TARGET_SECONDS", 60),
                contextlib.redirect_stdout(output),
            ):
                verify.report_fresh_device_latency(str(log))

        self.assertEqual(
            self.flagged_latency_checks(output.getvalue()),
            {"notice": {"fast"}, "warning": {"slow", "stuck"}},
        )

    def test_reports_wait_before_first_page_separately_from_download(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            log = pathlib.Path(directory, "latency.jsonl")
            summary = pathlib.Path(directory, "summary.md")
            log.write_text(
                '{"check": "prepared", "seconds": 80.0, "completed": true,'
                ' "first_page_seconds": 70.5, "empty_calls_before_first_page": 4,'
                ' "sync_calls": 6, "sync_call_seconds": 66.25, "snapshot_resets": 1,'
                ' "rows": 12, "chunks": 1, "bytes": 2048}\n'
                '{"check": "never", "seconds": 180.0, "completed": false,'
                ' "first_page_seconds": null, "empty_calls_before_first_page": 9,'
                ' "sync_calls": 9, "sync_call_seconds": 160.0, "snapshot_resets": 4,'
                ' "rows": 0, "chunks": 0, "bytes": 0}\n'
                '{"check": "legacy", "seconds": 12.0, "completed": true}\n'
            )
            output = io.StringIO()
            with (
                mock.patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": str(summary)}),
                contextlib.redirect_stdout(output),
            ):
                verify.report_fresh_device_latency(str(log))
            table = summary.read_text()

        annotations = output.getvalue()
        self.assertIn(
            "prepared: 80.0s, ok (target 90s); first page at 70.5s after 4 empty sync calls",
            annotations,
        )
        self.assertIn(
            "never: 180.0s, timed out (target 90s); no page after 9 empty sync calls",
            annotations,
        )
        self.assertIn("legacy: 12.0s, ok (target 90s)\n", annotations)
        self.assertIn(
            "| prepared | 80.0s | ok | 70.5s | 9.5s | 4 | 6 (66.2s) | 1 | 12 rows, 2048 B |",
            table,
        )
        self.assertIn(
            "| never | 180.0s | timed out | none | none | 9 | 9 (160.0s) | 4 | 0 rows, 0 B |",
            table,
        )
        self.assertIn("| legacy | 12.0s | ok | n/a | n/a | n/a | n/a | n/a |", table)

    def test_failed_verification_still_reports_latency_and_removes_log(self) -> None:
        failure = subprocess.CalledProcessError(101, ["cargo", "test"])
        logs = []

        def failing_verification(_values, _database, latency_log):
            logs.append(latency_log)
            pathlib.Path(latency_log).write_text(
                '{"check": "stuck", "seconds": 180.0, "completed": false}\n'
            )
            raise failure

        output = io.StringIO()
        with (
            mock.patch.object(sys, "argv", ["verify", "secrets.json"]),
            mock.patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": ""}),
            mock.patch.object(verify, "load_secrets", return_value={}),
            mock.patch.object(verify, "verify_remote_database", return_value="db"),
            mock.patch.object(
                verify, "run_verification", side_effect=failing_verification
            ),
            contextlib.redirect_stdout(output),
        ):
            with self.assertRaises(subprocess.CalledProcessError):
                verify.main()

        self.assertEqual(
            self.flagged_latency_checks(output.getvalue())["warning"], {"stuck"}
        )
        self.assertFalse(pathlib.Path(logs[0]).exists())

    def run_main_with_mode(self, mode: str | None) -> tuple[mock.Mock, mock.Mock]:
        secrets = (
            [] if mode is None else [{"key": "ANARLOG_SQLITECLOUD_MODE", "value": mode}]
        )
        if mode != "retired":
            secrets += [{"key": key, "value": "set"} for key in verify.REQUIRED_SECRETS]
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory, "secrets.json")
            path.write_text(verify.json.dumps(secrets))
            with (
                mock.patch.object(sys, "argv", ["verify", str(path)]),
                mock.patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": ""}),
                mock.patch.object(
                    verify, "verify_remote_database", return_value="db"
                ) as remote,
                mock.patch.object(verify, "run_verification") as sync_tests,
                mock.patch.object(
                    verify, "request_json", side_effect=AssertionError("provider call")
                ),
                contextlib.redirect_stdout(io.StringIO()),
            ):
                verify.main()
        return remote, sync_tests

    def test_sqlitecloud_mode_limits_what_verification_touches(self) -> None:
        for mode, checks_registration, runs_sync_tests in [
            (None, True, True),
            ("enabled", True, True),
            # Minting tokens or writing fixtures would move the fenced boundary.
            ("draining", True, False),
            ("retired", False, False),
        ]:
            with self.subTest(mode=mode):
                remote, sync_tests = self.run_main_with_mode(mode)
                self.assertEqual(remote.called, checks_registration)
                self.assertEqual(sync_tests.called, runs_sync_tests)

        with self.assertRaisesRegex(ValueError, "ANARLOG_SQLITECLOUD_MODE"):
            self.run_main_with_mode("disabled")

    def e2ee_record_columns(self) -> list[dict[str, object]]:
        timestamp_default = "(strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))"
        return [
            {
                "name": "id",
                "type": "TEXT",
                "notnull": 1,
                "pk": 1,
                "hidden": 0,
                "dflt_value": None,
            },
            {
                "name": "workspace_id",
                "type": "TEXT",
                "notnull": 1,
                "pk": 0,
                "hidden": 0,
                "dflt_value": "''",
            },
            {
                "name": "payload",
                "type": "TEXT",
                "notnull": 1,
                "pk": 0,
                "hidden": 0,
                "dflt_value": "''",
            },
            {
                "name": "created_at",
                "type": "TEXT",
                "notnull": 1,
                "pk": 0,
                "hidden": 0,
                "dflt_value": timestamp_default,
            },
            {
                "name": "updated_at",
                "type": "TEXT",
                "notnull": 1,
                "pk": 0,
                "hidden": 0,
                "dflt_value": timestamp_default,
            },
        ]

    def test_accepts_only_known_protocol_modes(self) -> None:
        for mode in verify.PROTOCOL_MODES:
            self.assertEqual(
                verify.protocol_mode({"ANARLOG_CLOUDSYNC_PROTOCOL_MODE": mode}),
                mode,
            )

        with self.assertRaisesRegex(ValueError, "must be dual"):
            verify.protocol_mode({"ANARLOG_CLOUDSYNC_PROTOCOL_MODE": "legacy"})

    def test_requires_the_legacy_plaintext_database_to_be_deleted(self) -> None:
        with mock.patch.object(verify, "management_get", return_value={"id": "legacy"}):
            with self.assertRaisesRegex(ValueError, "must be deleted"):
                verify.verify_legacy_database_retired("legacy-id", "management-key")

    def test_accepts_a_missing_legacy_plaintext_database(self) -> None:
        with mock.patch.object(
            verify,
            "management_get",
            side_effect=verify.ResourceNotFound("not found"),
        ):
            verify.verify_legacy_database_retired("legacy-id", "management-key")

    def test_dual_mode_requires_the_legacy_database_to_exist(self) -> None:
        with mock.patch.object(
            verify,
            "management_get",
            side_effect=verify.ResourceNotFound("not found"),
        ):
            with self.assertRaisesRegex(ValueError, "must exist"):
                verify.verify_legacy_database_transition(
                    "dual",
                    "legacy-id",
                    "management-key",
                    ("project-e2ee", "e2ee.sqlite"),
                )

    def test_transition_modes_accept_a_distinct_legacy_database(self) -> None:
        legacy = {"projectId": "project-legacy", "databaseName": "legacy.sqlite"}
        for mode in ("dual", "e2ee_only"):
            with self.subTest(mode=mode):
                with mock.patch.object(
                    verify,
                    "management_get",
                    side_effect=[legacy, {"ok": True}],
                ):
                    verify.verify_legacy_database_transition(
                        mode,
                        "legacy-id",
                        "management-key",
                        ("project-e2ee", "e2ee.sqlite"),
                    )

    def test_e2ee_only_mode_allows_the_legacy_database_to_be_missing(self) -> None:
        with mock.patch.object(
            verify,
            "management_get",
            side_effect=verify.ResourceNotFound("not found"),
        ):
            verify.verify_legacy_database_transition(
                "e2ee_only",
                "legacy-id",
                "management-key",
                ("project-e2ee", "e2ee.sqlite"),
            )

    def test_transition_modes_require_a_connected_legacy_database(self) -> None:
        legacy = {"projectId": "project-legacy", "databaseName": "legacy.sqlite"}
        for mode in ("dual", "e2ee_only"):
            with self.subTest(mode=mode):
                with mock.patch.object(
                    verify,
                    "management_get",
                    side_effect=[legacy, {"ok": False}],
                ):
                    with self.assertRaisesRegex(ValueError, "connection check failed"):
                        verify.verify_legacy_database_transition(
                            mode,
                            "legacy-id",
                            "management-key",
                            ("project-e2ee", "e2ee.sqlite"),
                        )

    def test_transition_modes_reject_the_same_physical_database(self) -> None:
        database = {"projectId": "project", "databaseName": "cloud.sqlite"}
        for mode in ("dual", "e2ee_only"):
            with self.subTest(mode=mode):
                with mock.patch.object(verify, "management_get", return_value=database):
                    with self.assertRaisesRegex(ValueError, "physical database"):
                        verify.verify_legacy_database_transition(
                            mode,
                            "legacy-id",
                            "management-key",
                            ("project", "cloud.sqlite"),
                        )

    def test_rejects_plaintext_tables_in_the_encrypted_database(self) -> None:
        with mock.patch.object(
            verify,
            "run_sql",
            return_value=[{"name": "sessions"}, {"name": "transcripts"}],
        ):
            with self.assertRaisesRegex(ValueError, "sessions, transcripts"):
                verify.verify_no_plaintext_tables(
                    "https://sqlite.example",
                    "issuer-key",
                    "encrypted-database",
                )

    def test_accepts_an_encrypted_database_without_plaintext_tables(self) -> None:
        with mock.patch.object(verify, "run_sql", return_value=[]) as run_sql:
            verify.verify_no_plaintext_tables(
                "https://sqlite.example",
                "issuer-key",
                "encrypted-database",
            )

        query = run_sql.call_args.args[3]
        for table in verify.LEGACY_PLAINTEXT_TABLES:
            self.assertIn(f"'{table}'", query)

    def test_accepts_the_current_encrypted_replica_columns(self) -> None:
        verify.verify_e2ee_record_columns(self.e2ee_record_columns())

    def test_rejects_a_client_only_payload_hash_column(self) -> None:
        columns = self.e2ee_record_columns()
        columns.append(
            {
                "name": "payload_hash",
                "type": "TEXT",
                "notnull": 1,
                "pk": 0,
                "hidden": 0,
                "dflt_value": "''",
            }
        )
        with self.assertRaisesRegex(ValueError, "unexpected column contract"):
            verify.verify_e2ee_record_columns(columns)

    def test_requires_the_payload_default(self) -> None:
        columns = self.e2ee_record_columns()
        columns[2]["dflt_value"] = None

        with self.assertRaisesRegex(ValueError, "unexpected column defaults"):
            verify.verify_e2ee_record_columns(columns)


if __name__ == "__main__":
    unittest.main()
