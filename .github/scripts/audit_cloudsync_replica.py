#!/usr/bin/env python3
"""Read-only coverage audit of SQLite Cloud against the Supabase encrypted log.

For each workspace the provider still holds, every provider record must have a
disposition in the Supabase log. The comparison uses opaque record ids and
ciphertext hashes only; nothing is decrypted and nothing is written to either
store. There is deliberately no repair mode: a value missing from the log may
only be reconciled by an authorized client through conditional acceptance, so
this tool archives the ciphertext for that client instead of publishing it.

Supported clients apply a provider record only when it matches the value the
log selects for that record, and the log carries that value's ciphertext. So
`current` and `covered_historical` are values the log already holds (the newest
one, or an earlier one), while `diverged` and `missing` were never witnessed:
clients refuse them today and they still live only on the device that wrote
them. A workspace with provider records and no log at all is counted as
`uninitialized`. Against a live provider, records written during the
comparison show up as `diverged`; only a fenced source gives final numbers.

Evidence (summaries, unresolved record ids, optional ciphertext archive) goes
to a directory outside any Git work tree. Reruns resume after the last
completed workspace.
"""

from __future__ import annotations

import argparse
import base64
import datetime
import hashlib
import json
import os
import pathlib
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request


SUPABASE_PAGE_ROWS = 1000
PROVIDER_PAGE_ROWS = 500
PROVIDER_PAYLOAD_BYTES = 8 * 1024 * 1024
REQUEST_TIMEOUT_SECONDS = 30
BLINDED_ID = re.compile(r"^[A-Za-z0-9_-]{43}$")
WORKSPACE_ID = re.compile(
    r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"
)
# Listed worst first; the first two fail the retirement gate.
CLASSES = ("missing", "diverged", "covered_historical", "current")


class NoRedirectHandler(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, file_pointer, code, message, headers, new_url):
        return None


HTTP = urllib.request.build_opener(NoRedirectHandler())


def load_secrets(path: str) -> dict[str, str]:
    with open(path) as source:
        return {secret["key"]: secret.get("value", "") for secret in json.load(source)}


def https_origin(url: str, label: str) -> str:
    parsed = urllib.parse.urlsplit(url)
    if (
        parsed.scheme != "https"
        or not parsed.hostname
        or parsed.username
        or parsed.path not in ("", "/")
        or parsed.query
    ):
        raise ValueError(f"{label} must be an HTTPS origin")
    return url.rstrip("/")


def http_json(
    url: str, headers: dict[str, str], label: str, body: object | None = None
) -> object:
    request = urllib.request.Request(
        url,
        data=None if body is None else json.dumps(body).encode(),
        headers={
            "Accept": "application/json",
            **headers,
            **({"Content-Type": "application/json"} if body is not None else {}),
        },
        method="GET" if body is None else "POST",
    )
    try:
        with HTTP.open(request, timeout=REQUEST_TIMEOUT_SECONDS) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        # Never retried here: a struggling database is a reason to stop the audit.
        raise RuntimeError(f"{label} failed with HTTP {error.code}") from error
    except urllib.error.URLError as error:
        raise RuntimeError(f"{label} could not connect") from error


def payload_hash(payload: str) -> str:
    digest = hashlib.sha256(payload.encode()).digest()
    return base64.urlsafe_b64encode(digest).decode().rstrip("=")


def sql_literal(value: str) -> str:
    if "\x00" in value:
        raise ValueError("provider identifier contains a NUL byte")
    return "'" + value.replace("'", "''") + "'"


class Provider:
    def __init__(self, project_url: str, key: str, database: str, pause: float):
        self.url = (
            https_origin(project_url, "SQLITECLOUD_PROJECT_URL") + "/v2/weblite/sql"
        )
        self.key, self.database, self.pause = key, database, pause

    def sql(self, sql: str) -> list[dict[str, object]]:
        time.sleep(self.pause)
        envelope = http_json(
            self.url,
            {"Authorization": f"Bearer {self.key}"},
            "SQLite Cloud query",
            {"database": self.database, "sql": sql},
        )
        rows = envelope.get("data") if isinstance(envelope, dict) else None
        if not isinstance(rows, list):
            raise ValueError("SQLite Cloud query returned invalid rows")
        return rows

    def next_workspace(self, after: str) -> str | None:
        # One index seek per workspace instead of a scan over every record.
        rows = self.sql(
            "SELECT min(workspace_id) AS workspace_id FROM e2ee_records"
            f" WHERE workspace_id > {sql_literal(after)}"
        )
        workspace = rows[0].get("workspace_id") if rows else None
        return workspace if isinstance(workspace, str) else None

    def records(self, workspace: str):
        """Yields (record id, ciphertext) in id order, bounded by rows and bytes."""
        where = f"workspace_id = {sql_literal(workspace)}"
        after = ""
        while True:
            page = self.sql(
                f"SELECT id, length(payload) AS size FROM e2ee_records WHERE {where}"
                f" AND id > {sql_literal(after)} ORDER BY id LIMIT {PROVIDER_PAGE_ROWS}"
            )
            if not page:
                return
            batch: list[str] = []
            batch_bytes = 0
            for row in [*page, None]:
                size = int(row["size"]) if row else 0
                if batch and (
                    row is None or batch_bytes + size > PROVIDER_PAYLOAD_BYTES
                ):
                    ids = ", ".join(sql_literal(record) for record in batch)
                    for record in self.sql(
                        f"SELECT id, payload FROM e2ee_records WHERE {where}"
                        f" AND id IN ({ids}) ORDER BY id"
                    ):
                        yield str(record["id"]), str(record["payload"])
                    batch, batch_bytes = [], 0
                if row:
                    batch.append(str(row["id"]))
                    batch_bytes += size
            after = str(page[-1]["id"])


class Replica:
    def __init__(self, supabase_url: str, service_role_key: str, pause: float):
        self.url = (
            https_origin(supabase_url, "SUPABASE_URL")
            + "/rest/v1/e2ee_freshness_events"
        )
        self.headers = {
            "apikey": service_role_key,
            "Authorization": f"Bearer {service_role_key}",
        }
        self.pause = pause

    def rows(self, query: dict[str, str]) -> list[dict[str, object]]:
        time.sleep(self.pause)
        rows = http_json(
            self.url + "?" + urllib.parse.urlencode(query),
            self.headers,
            "Supabase encrypted log query",
        )
        if not isinstance(rows, list):
            raise ValueError("Supabase encrypted log query returned invalid rows")
        return rows

    def manifest(self, workspace: str) -> tuple[int, dict[str, list[str]]]:
        """Returns the boundary sequence and each record's hashes, oldest first."""
        workspace_filter = f"eq.{workspace}"
        head = self.rows(
            {
                "select": "sequence",
                "workspace_id": workspace_filter,
                "order": "sequence.desc",
                "limit": "1",
            }
        )
        through = int(head[0]["sequence"]) if head else 0
        hashes: dict[str, list[str]] = {}
        after = 0
        while after < through:
            # Keyset pages on the (workspace_id, sequence) index, no payloads.
            page = self.rows(
                {
                    "select": "sequence,record_id,payload_hash",
                    "workspace_id": workspace_filter,
                    "and": f"(sequence.gt.{after},sequence.lte.{through})",
                    "order": "sequence.asc",
                    "limit": str(SUPABASE_PAGE_ROWS),
                }
            )
            if not page:
                break
            for row in page:
                hashes.setdefault(str(row["record_id"]), []).append(
                    str(row["payload_hash"])
                )
            after = int(page[-1]["sequence"])
        return through, hashes


def classify(source_hash: str, replica_hashes: list[str] | None) -> str:
    if not replica_hashes:
        return "missing"
    if replica_hashes[-1] == source_hash:
        return "current"
    if source_hash in replica_hashes:
        return "covered_historical"
    # Whether the newer log value supersedes this one needs an authorized client.
    return "diverged"


def audit_workspace(provider, replica, workspace: str, archive) -> dict[str, object]:
    summary: dict[str, object] = {"workspace": workspace}
    if not WORKSPACE_ID.match(workspace):
        # Supabase only holds UUID workspaces, so nothing can cover this one.
        return summary | {"unmapped": True}
    through, hashes = replica.manifest(workspace)
    counts = dict.fromkeys(CLASSES, 0)
    unresolved = []
    seen = 0
    for record_id, payload in provider.records(workspace):
        seen += 1
        if not BLINDED_ID.match(record_id):
            raise ValueError("provider returned an invalid record id")
        source_hash = payload_hash(payload)
        outcome = classify(source_hash, hashes.get(record_id))
        counts[outcome] += 1
        if outcome in ("missing", "diverged"):
            unresolved.append(
                {
                    "workspace": workspace,
                    "record_id": record_id,
                    "source_hash": source_hash,
                    "class": outcome,
                }
            )
            if archive is not None:
                archive(workspace, record_id, source_hash, payload)
    return summary | {
        "through_sequence": through,
        "replica_initialized": through > 0,
        "source_records": seen,
        "replica_records": len(hashes),
        "classes": counts,
        "unresolved": unresolved,
    }


def evidence_directory(path: str) -> pathlib.Path:
    directory = pathlib.Path(path).resolve()
    if not directory.is_dir():
        raise ValueError("the evidence directory must already exist")
    if any((parent / ".git").exists() for parent in [directory, *directory.parents]):
        raise ValueError("the evidence directory must be outside any Git work tree")
    return directory


def append_json(path: pathlib.Path, value: object) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
    with os.fdopen(descriptor, "a") as output:
        output.write(json.dumps(value, separators=(",", ":")) + "\n")


def write_progress(path: pathlib.Path, progress: dict[str, object]) -> None:
    temporary = path.with_suffix(".tmp")
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(descriptor, "w") as output:
        json.dump(progress, output)
    os.replace(temporary, path)


def run(arguments, provider, replica, evidence: pathlib.Path) -> dict[str, int]:
    """Audits the selected workspaces, or continues the full pass, and returns totals.

    The full pass accumulates its totals in `progress.json`, so a resumed run
    still fails on what an earlier run found. A `--workspace` run reports only
    itself and leaves the full pass untouched.
    """
    empty = dict.fromkeys((*CLASSES, "workspaces", "unmapped", "uninitialized"), 0)
    progress_path = evidence / "progress.json"
    progress = {"cursor": "", "source": arguments.source, "totals": dict(empty)}
    if progress_path.exists():
        progress = json.loads(progress_path.read_text())
        # One directory holds one provider state: mixed evidence proves nothing.
        if progress.get("source") != arguments.source:
            raise ValueError(
                "this evidence directory belongs to a different source snapshot"
            )
        if progress.get("totals", {}).keys() != empty.keys():
            # Defaulting a total this version added would report zero for work
            # an older version already finished.
            raise ValueError(
                "this evidence was written by another version of the audit;"
                " start a new evidence directory"
            )
    else:
        write_progress(progress_path, progress)

    archive = None
    if arguments.archive:
        (evidence / "archive").mkdir(mode=0o700, exist_ok=True)

        def archive(workspace, record_id, source_hash, payload):
            append_json(
                evidence
                / "archive"
                / f"{hashlib.sha256(workspace.encode()).hexdigest()}.jsonl",
                {
                    "source": arguments.source,
                    "workspace": workspace,
                    "record_id": record_id,
                    "payload_hash": source_hash,
                    "payload": payload,
                },
            )

    totals = dict(empty) if arguments.workspace else progress["totals"]
    selected = iter(arguments.workspace or [])
    audited = 0
    while arguments.limit is None or audited < arguments.limit:
        workspace = (
            next(selected, None)
            if arguments.workspace
            else provider.next_workspace(str(progress["cursor"]))
        )
        if workspace is None:
            break
        summary = audit_workspace(provider, replica, workspace, archive)
        for record in summary.pop("unresolved", []):
            append_json(
                evidence / "unresolved.jsonl", record | {"source": arguments.source}
            )
        append_json(
            evidence / "workspaces.jsonl",
            summary
            | {
                "source": arguments.source,
                "audited_at": datetime.datetime.now(datetime.UTC).isoformat(
                    timespec="seconds"
                ),
            },
        )
        audited += 1
        totals["workspaces"] += 1
        totals["unmapped"] += bool(summary.get("unmapped"))
        totals["uninitialized"] += (
            summary.get("replica_initialized") is False
            and summary.get("source_records", 0) > 0
        )
        for name, count in summary.get("classes", {}).items():
            totals[name] += count
        if not arguments.workspace:
            progress["cursor"] = workspace
            write_progress(progress_path, progress)
    return totals


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument(
        "--cloudsync", required=True, help="Infisical JSON export of /anarlog/cloudsync"
    )
    parser.add_argument(
        "--api",
        required=True,
        help="Infisical JSON export holding the Supabase URL and service role key",
    )
    parser.add_argument(
        "--database",
        required=True,
        help="SQLite Cloud database name of the encrypted replica",
    )
    parser.add_argument(
        "--evidence", required=True, help="existing private directory outside Git"
    )
    parser.add_argument(
        "--source",
        required=True,
        help="identity of the provider state compared, such as live-2026-11-20 or fenced-<time>; a live source is preliminary evidence",
    )
    parser.add_argument(
        "--workspace",
        action="append",
        help="audit only this workspace; repeatable, does not move the resume cursor",
    )
    parser.add_argument("--limit", type=int, help="stop after this many workspaces")
    parser.add_argument(
        "--archive",
        action="store_true",
        help="save the ciphertext of unresolved records for an authorized client",
    )
    parser.add_argument(
        "--pause-ms", type=int, default=200, help="delay before every request"
    )
    arguments = parser.parse_args()

    cloudsync, api = load_secrets(arguments.cloudsync), load_secrets(arguments.api)
    pause = arguments.pause_ms / 1000
    totals = run(
        arguments,
        Provider(
            cloudsync["SQLITECLOUD_PROJECT_URL"],
            cloudsync["SQLITECLOUD_TOKEN_ISSUER_API_KEY"],
            arguments.database,
            pause,
        ),
        Replica(api["SUPABASE_URL"], api["SUPABASE_SERVICE_ROLE_KEY"], pause),
        evidence_directory(arguments.evidence),
    )
    print(json.dumps(totals))
    return 2 if totals["missing"] or totals["diverged"] or totals["unmapped"] else 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (RuntimeError, ValueError, KeyError) as error:
        print(f"CloudSync replica audit stopped: {error}", file=sys.stderr)
        raise SystemExit(1) from error
