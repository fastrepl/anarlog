import { describe, expect, it, vi } from "vitest";

import {
  type FolderDefaultAccessRule,
  folderShareCandidateReady,
} from "./folder-default-access";

vi.mock("~/db", () => ({
  executeTransaction: vi.fn(),
  liveQueryClient: { execute: vi.fn() },
  useLiveQuery: vi.fn(),
}));
vi.mock("~/db/write-queue", () => ({ enqueueDatabaseWrite: vi.fn() }));

const rule: FolderDefaultAccessRule = {
  folder_path: "Weekly sync",
  access: "workspace",
  workspace_id: "team-ws",
  since: "2026-10-08T10:00:00.000Z",
};

const summary = JSON.stringify({
  type: "doc",
  content: [
    { type: "paragraph", content: [{ type: "text", text: "Decided X" }] },
  ],
});

function candidate(createdAt: string, body = summary) {
  return {
    sessionId: "session-1",
    folderPath: "Weekly sync",
    createdAt,
    documentId: "doc-1",
    documentUpdatedAt: createdAt,
    title: "Weekly sync",
    body,
  };
}

describe("folderShareCandidateReady", () => {
  it("does not retroactively share notes created before the folder default was set", () => {
    expect(
      folderShareCandidateReady(
        candidate("2026-10-07T10:00:00.000Z"),
        rule,
        false,
      ),
    ).toBe(false);
    expect(
      folderShareCandidateReady(
        candidate("2026-10-08T11:00:00.000Z"),
        rule,
        false,
      ),
    ).toBe(true);
  });

  it("waits until the summary has finished generating", () => {
    expect(
      folderShareCandidateReady(
        candidate("2026-10-08T11:00:00.000Z"),
        rule,
        true,
      ),
    ).toBe(false);
  });
});
