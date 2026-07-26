import importlib.util
import pathlib
import unittest
from unittest import mock


MODULE_PATH = pathlib.Path(__file__).with_name("verify_cloudsync_e2ee.py")
SPEC = importlib.util.spec_from_file_location("verify_cloudsync_e2ee", MODULE_PATH)
assert SPEC and SPEC.loader
verify = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(verify)


class VerifyCloudSyncE2eeTests(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
