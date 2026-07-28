import { describe, expect, it } from "vitest";

import { cloudApiSnapshotChanges } from "./lifecycle";

describe("cloud API snapshot lifecycle", () => {
  it("deletes snapshots already marked deleted on initial load", () => {
    const changes = cloudApiSnapshotChanges(null, [
      {
        id: "active",
        revision: "2026-07-28T00:00:00Z",
        deletedAt: null,
      },
      {
        id: "deleted",
        revision: "2026-07-28T00:00:00Z",
        deletedAt: "2026-07-28T01:00:00Z",
      },
    ]);

    expect(changes.deletes).toEqual(["deleted"]);
    expect(changes.upserts).toEqual(["active"]);
  });

  it("syncs revisions and deletes sessions removed from the query", () => {
    const changes = cloudApiSnapshotChanges(
      new Map([
        ["changed", "old-revision"],
        ["removed", "old-revision"],
      ]),
      [
        {
          id: "changed",
          revision: "new-revision",
          deletedAt: null,
        },
      ],
    );

    expect(changes.deletes).toEqual(["removed"]);
    expect(changes.upserts).toEqual(["changed"]);
  });

  it("schedules a soft deletion only when its state changes", () => {
    const deletedState = "deleted:2026-07-28T01:00:00Z";
    const revision = {
      id: "deleted",
      revision: "2026-07-28T00:00:00Z",
      deletedAt: "2026-07-28T01:00:00Z",
    };

    expect(
      cloudApiSnapshotChanges(new Map([["deleted", "old-revision"]]), [
        revision,
      ]).deletes,
    ).toEqual(["deleted"]);
    expect(
      cloudApiSnapshotChanges(new Map([["deleted", deletedState]]), [revision])
        .deletes,
    ).toEqual([]);
  });
});
