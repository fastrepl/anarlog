import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("~/session/queries", () => ({
  useSessionSummaries: () => [
    {
      id: "session-1",
      title: "Planning",
      created_at: "2026-07-10T09:00:00.000Z",
    },
  ],
}));

vi.mock("~/contacts/queries", () => ({
  useHumans: () => [
    {
      id: "human-1",
      name: "Alice",
      email: "alice@example.com",
      organizationId: "organization-1",
    },
  ],
  useOrganizations: () => [{ id: "organization-1", name: "Acme" }],
}));

import { useChatContextPipeline } from "./use-chat-context-pipeline";

describe("chat context display pipeline", () => {
  it("resolves pending entity labels from live SQLite records", () => {
    const { result } = renderHook(() =>
      useChatContextPipeline({
        messages: [],
        currentSessionId: "session-1",
        pendingManualRefs: [
          {
            kind: "human",
            key: "human:manual:human-1",
            source: "manual",
            humanId: "human-1",
          },
          {
            kind: "organization",
            key: "organization:manual:organization-1",
            source: "manual",
            organizationId: "organization-1",
          },
        ],
      }),
    );

    expect(result.current.contextEntities).toEqual([
      expect.objectContaining({
        kind: "session",
        title: "Planning",
        date: "2026-07-10T09:00:00.000Z",
      }),
      expect.objectContaining({
        kind: "human",
        name: "Alice",
        email: "alice@example.com",
        organizationName: "Acme",
        removable: true,
      }),
      expect.objectContaining({
        kind: "organization",
        name: "Acme",
        removable: true,
      }),
    ]);
  });
});
