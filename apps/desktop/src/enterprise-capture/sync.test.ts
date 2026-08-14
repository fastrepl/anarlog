import { beforeEach, describe, expect, it, vi } from "vitest";

import { applySessionIngest } from "@anlg/plugin-db";
import { commands as localApiCommands } from "@anlg/plugin-local-api";

import { acknowledgeSessionDelivery, listSessionDeliveries } from "./client";
import {
  listPendingAcknowledgements,
  listPendingCompletions,
  loadDeliveryCursor,
  markCompletionDispatched,
  markDeliveryAcknowledged,
  recordAppliedDelivery,
  recordRejectedDelivery,
} from "./store";
import {
  dispatchPendingEnterpriseCompletions,
  syncEnterpriseWorkspace,
} from "./sync";

import { runMeetingCompletedAutomations } from "~/automations/engine";

vi.mock("@anlg/plugin-db", () => ({ applySessionIngest: vi.fn() }));
vi.mock("@anlg/plugin-local-api", () => ({
  commands: { dispatchEvent: vi.fn() },
}));
vi.mock("./client", () => ({
  acknowledgeSessionDelivery: vi.fn(),
  listSessionDeliveries: vi.fn(),
}));
vi.mock("./store", () => ({
  listPendingAcknowledgements: vi.fn(),
  listPendingCompletions: vi.fn(),
  loadDeliveryCursor: vi.fn(),
  markCompletionDispatched: vi.fn(),
  markDeliveryAcknowledged: vi.fn(),
  recordAppliedDelivery: vi.fn(),
  recordRejectedDelivery: vi.fn(),
}));
vi.mock("~/automations/engine", () => ({
  runMeetingCompletedAutomations: vi.fn(),
}));

const input = {
  serverUrl: "https://capture.example.test",
  accessToken: "access-token",
  workspaceId: "workspace-1",
  consumerId: "device-1",
};

const item = {
  cursor: 5,
  jobId: "job-1",
  revision: 2,
  finalized: true,
  contentHash: "a".repeat(64),
  acknowledged: false,
  createdAt: "2026-08-14T08:00:00Z",
  envelope: {
    schema_version: 1,
    source_id: "job-1",
    revision: 2,
    finalized: true,
    workspace_id: "workspace-1",
    session: { id: "session-1", status: "completed" },
  },
};

describe("enterprise capture sync", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(listPendingAcknowledgements).mockResolvedValue([]);
    vi.mocked(loadDeliveryCursor).mockResolvedValue(0);
    vi.mocked(listSessionDeliveries).mockResolvedValue({
      items: [item],
      nextCursor: 5,
      hasMore: false,
    });
    vi.mocked(applySessionIngest).mockResolvedValue("applied");
    vi.mocked(acknowledgeSessionDelivery).mockResolvedValue(undefined);
    vi.mocked(recordAppliedDelivery).mockResolvedValue(undefined);
    vi.mocked(recordRejectedDelivery).mockResolvedValue(undefined);
    vi.mocked(markDeliveryAcknowledged).mockResolvedValue(undefined);
  });

  it("acknowledges a permanent rejection and continues with later deliveries", async () => {
    const laterItem = {
      ...item,
      cursor: 6,
      jobId: "job-2",
      contentHash: "b".repeat(64),
      envelope: {
        ...item.envelope,
        source_id: "job-2",
        session: { id: "session-2", status: "completed" },
      },
    };
    vi.mocked(listSessionDeliveries).mockResolvedValue({
      items: [item, laterItem],
      nextCursor: 6,
      hasMore: false,
    });
    vi.mocked(applySessionIngest)
      .mockResolvedValueOnce("rejected")
      .mockResolvedValueOnce("applied");

    await syncEnterpriseWorkspace(input);

    expect(recordRejectedDelivery).toHaveBeenCalledWith({ ...input, item });
    expect(recordAppliedDelivery).toHaveBeenCalledWith({
      ...input,
      item: laterItem,
    });
    expect(acknowledgeSessionDelivery).toHaveBeenCalledTimes(2);
  });

  it("applies and persists before acknowledging a revision", async () => {
    await syncEnterpriseWorkspace(input);

    expect(applySessionIngest).toHaveBeenCalledWith(
      "workspace-1",
      item.envelope,
    );
    expect(recordAppliedDelivery).toHaveBeenCalledWith({ ...input, item });
    expect(acknowledgeSessionDelivery).toHaveBeenCalledWith({
      ...input,
      jobId: "job-1",
      revision: 2,
      contentHash: "a".repeat(64),
    });
    expect(
      vi.mocked(recordAppliedDelivery).mock.invocationCallOrder[0],
    ).toBeLessThan(
      vi.mocked(acknowledgeSessionDelivery).mock.invocationCallOrder[0]!,
    );
  });

  it("retries a durable acknowledgement before requesting later pages", async () => {
    vi.mocked(listPendingAcknowledgements).mockResolvedValue([
      { jobId: "job-old", revision: 1, contentHash: "b".repeat(64) },
    ]);

    await syncEnterpriseWorkspace(input);

    expect(vi.mocked(acknowledgeSessionDelivery).mock.calls[0]?.[0]).toEqual({
      ...input,
      jobId: "job-old",
      revision: 1,
      contentHash: "b".repeat(64),
    });
    expect(
      vi.mocked(acknowledgeSessionDelivery).mock.invocationCallOrder[0],
    ).toBeLessThan(
      vi.mocked(listSessionDeliveries).mock.invocationCallOrder[0]!,
    );
  });

  it("dispatches each durable completion once after successful processing", async () => {
    vi.mocked(listPendingCompletions)
      .mockResolvedValueOnce([
        {
          sourceId: "job-1",
          workspaceId: "workspace-1",
          sessionId: "session-1",
          revision: 2,
        },
      ])
      .mockResolvedValueOnce([]);
    vi.mocked(localApiCommands.dispatchEvent).mockResolvedValue({
      status: "ok",
      data: 0,
    });
    vi.mocked(runMeetingCompletedAutomations).mockResolvedValue(undefined);
    vi.mocked(markCompletionDispatched).mockResolvedValue(undefined);

    await dispatchPendingEnterpriseCompletions();
    await dispatchPendingEnterpriseCompletions();

    expect(localApiCommands.dispatchEvent).toHaveBeenCalledOnce();
    expect(runMeetingCompletedAutomations).toHaveBeenCalledOnce();
    expect(markCompletionDispatched).toHaveBeenCalledWith("job-1");
  });
});
