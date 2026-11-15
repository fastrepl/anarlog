import argparse
import importlib.util
import json
import pathlib
import sqlite3
import tempfile
import unittest
import urllib.parse
from unittest import mock


MODULE_PATH = pathlib.Path(__file__).with_name("audit_cloudsync_replica.py")
SPEC = importlib.util.spec_from_file_location("audit_cloudsync_replica", MODULE_PATH)
assert SPEC and SPEC.loader
audit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(audit)

WORKSPACE_A = "00000000-0000-4000-8000-00000000000a"
WORKSPACE_B = "00000000-0000-4000-8000-00000000000b"
FIXTURE_WORKSPACE = "deploy-e2ee-a-00000000-0000-4000-8000-000000000001"


def record_id(name: str) -> str:
    return name.ljust(43, "_")


class Stores:
    """An in-memory provider database and Supabase log behind `http_json`."""

    def __init__(self) -> None:
        self.provider = sqlite3.connect(":memory:")
        self.provider.row_factory = sqlite3.Row
        self.provider.execute(
            "CREATE TABLE e2ee_records (id TEXT PRIMARY KEY, workspace_id TEXT, payload TEXT)"
        )
        self.log: list[dict[str, object]] = []
        self.provider_queries: list[str] = []

    def source(self, workspace: str, name: str, payload: str) -> None:
        self.provider.execute(
            "INSERT INTO e2ee_records VALUES (?, ?, ?)",
            (record_id(name), workspace, payload),
        )

    def accepted(self, workspace: str, name: str, payload: str) -> None:
        self.log.append(
            {
                "sequence": len(self.log) + 1,
                "workspace_id": workspace,
                "record_id": record_id(name),
                "payload_hash": audit.payload_hash(payload),
            }
        )

    def respond(self, url, _headers, _label, body=None):
        if body is not None:
            self.provider_queries.append(body["sql"])
            return {"data": [dict(row) for row in self.provider.execute(body["sql"])]}
        query = dict(urllib.parse.parse_qsl(urllib.parse.urlsplit(url).query))
        rows = [
            row
            for row in self.log
            if row["workspace_id"] == query["workspace_id"].removeprefix("eq.")
        ]
        if "and" in query:
            after, through = (
                int(bound.split(".")[-1])
                for bound in query["and"].strip("()").split(",")
            )
            rows = [row for row in rows if after < row["sequence"] <= through]
        rows.sort(
            key=lambda row: row["sequence"], reverse=query["order"].endswith("desc")
        )
        return rows[: int(query["limit"])]

    def run(self, evidence: str, **overrides):
        arguments = argparse.Namespace(
            source="live", workspace=None, limit=None, archive=True
        )
        vars(arguments).update(overrides)
        with mock.patch.object(audit, "http_json", side_effect=self.respond):
            return audit.run(
                arguments,
                audit.Provider("https://project.example", "key", "e2ee.sqlite", 0),
                audit.Replica("https://supabase.example", "service-key", 0),
                pathlib.Path(evidence),
            )


def read_lines(path: pathlib.Path) -> list[dict[str, object]]:
    return [json.loads(line) for line in path.read_text().splitlines()]


class AuditCloudsyncReplicaTests(unittest.TestCase):
    def test_every_provider_record_gets_a_disposition(self) -> None:
        stores = Stores()
        stores.source(WORKSPACE_A, "current", "v2")
        stores.accepted(WORKSPACE_A, "current", "v1")
        stores.accepted(WORKSPACE_A, "current", "v2")
        stores.source(WORKSPACE_A, "older", "v1")
        stores.accepted(WORKSPACE_A, "older", "v1")
        stores.accepted(WORKSPACE_A, "older", "v2")
        stores.source(WORKSPACE_A, "diverged", "only at the provider")
        stores.accepted(WORKSPACE_A, "diverged", "only in the log")
        stores.source(WORKSPACE_A, "missing", "never accepted")
        # Another workspace's log must not cover this one.
        stores.accepted(WORKSPACE_B, "missing", "never accepted")
        stores.source(FIXTURE_WORKSPACE, "fixture", "leftover")

        with tempfile.TemporaryDirectory() as evidence:
            totals = stores.run(evidence)
            directory = pathlib.Path(evidence)
            unresolved = read_lines(directory / "unresolved.jsonl")
            archived = [
                line
                for path in (directory / "archive").iterdir()
                for line in read_lines(path)
            ]
            summaries = read_lines(directory / "workspaces.jsonl")

        self.assertEqual(
            {name: totals[name] for name in audit.CLASSES},
            {"missing": 1, "diverged": 1, "covered_historical": 1, "current": 1},
        )
        self.assertEqual(totals["unmapped"], 1)
        self.assertEqual(
            {(record["record_id"], record["class"]) for record in unresolved},
            {(record_id("diverged"), "diverged"), (record_id("missing"), "missing")},
        )
        # Only unresolved ciphertext is kept, for an authorized client to reconcile.
        self.assertEqual(
            {record["payload"] for record in archived},
            {"only at the provider", "never accepted"},
        )
        audited = next(row for row in summaries if row["workspace"] == WORKSPACE_A)
        self.assertEqual(audited["through_sequence"], 5)
        self.assertEqual(audited["source"], "live")
        self.assertFalse(any("payload" in row for row in [*unresolved, *summaries]))
        self.assertFalse(
            any(
                query.lstrip().upper().startswith(("INSERT", "UPDATE", "DELETE"))
                for query in stores.provider_queries
            )
        )

    def test_resumes_after_the_last_completed_workspace(self) -> None:
        stores = Stores()
        for workspace in (WORKSPACE_A, WORKSPACE_B):
            stores.source(workspace, "note" + workspace[-1], "v1")
            stores.accepted(workspace, "note" + workspace[-1], "v1")

        with tempfile.TemporaryDirectory() as evidence:
            self.assertEqual(stores.run(evidence, limit=1)["workspaces"], 1)
            self.assertEqual(stores.run(evidence)["workspaces"], 1)
            self.assertEqual(stores.run(evidence)["workspaces"], 0)
            audited = [
                row["workspace"]
                for row in read_lines(pathlib.Path(evidence, "workspaces.jsonl"))
            ]
            # A pilot run does not move the cursor of the full pass.
            self.assertEqual(
                stores.run(evidence, workspace=[WORKSPACE_A])["current"], 1
            )
            self.assertEqual(stores.run(evidence)["workspaces"], 0)
            with self.assertRaisesRegex(ValueError, "different source snapshot"):
                stores.run(evidence, source="fenced")

        self.assertEqual(audited, [WORKSPACE_A, WORKSPACE_B])

    def test_downloads_ciphertext_in_bounded_batches(self) -> None:
        stores = Stores()
        for index in range(5):
            stores.source(WORKSPACE_A, f"note{index}", "x" * 40)
            stores.accepted(WORKSPACE_A, f"note{index}", "x" * 40)

        with (
            tempfile.TemporaryDirectory() as evidence,
            mock.patch.object(audit, "PROVIDER_PAYLOAD_BYTES", 100),
            mock.patch.object(audit, "PROVIDER_PAGE_ROWS", 4),
        ):
            totals = stores.run(evidence)

        self.assertEqual(totals["current"], 5)
        downloads = [
            query.count(record_id("note")[:4])
            for query in stores.provider_queries
            if "SELECT id, payload" in query
        ]
        self.assertEqual(downloads, [2, 2, 1])

    def test_evidence_stays_out_of_git_and_identifiers_stay_literal(self) -> None:
        with self.assertRaisesRegex(ValueError, "outside any Git work tree"):
            audit.evidence_directory(str(MODULE_PATH.parent))
        self.assertEqual(
            audit.sql_literal("a'; DROP TABLE x; --"), "'a''; DROP TABLE x; --'"
        )
        with self.assertRaisesRegex(ValueError, "HTTPS origin"):
            audit.Provider("http://project.example", "key", "db", 0)


if __name__ == "__main__":
    unittest.main()
