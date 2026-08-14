import { fetch } from "@tauri-apps/plugin-http";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  acknowledgeSessionDelivery,
  EnterpriseCaptureClientError,
  listSessionDeliveries,
} from "./client";

vi.mock("@tauri-apps/plugin-http", () => ({ fetch: vi.fn() }));

function delivery() {
  return {
    cursor: 7,
    jobId: "11111111-1111-4111-8111-111111111111",
    revision: 2,
    finalized: true,
    contentHash: "a".repeat(64),
    acknowledged: false,
    createdAt: "2026-08-14T08:00:00Z",
    envelope: {
      schema_version: 1,
      source_id: "11111111-1111-4111-8111-111111111111",
      revision: 2,
      finalized: true,
      workspace_id: "workspace-1",
      session: { id: "session-1", status: "completed" },
    },
  };
}

function jsonResponse(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

describe("enterprise capture client", () => {
  beforeEach(() => vi.mocked(fetch).mockReset());

  it("requests a bounded authenticated cursor page", async () => {
    vi.mocked(fetch).mockResolvedValue(
      jsonResponse({ items: [delivery()], nextCursor: 7, hasMore: false }),
    );

    const page = await listSessionDeliveries({
      serverUrl: "https://capture.example.test/control/",
      accessToken: "access-token",
      workspaceId: "workspace 1",
      consumerId: "device-1",
      after: 4,
    });

    expect(page.items[0]?.envelope.session.id).toBe("session-1");
    const [url, init] = vi.mocked(fetch).mock.calls[0]!;
    expect(url).toBe(
      "https://capture.example.test/control/v1/workspaces/workspace%201/session-envelopes?consumerId=device-1&after=4&limit=10",
    );
    expect(new Headers(init?.headers).get("Authorization")).toBe(
      "Bearer access-token",
    );
  });

  it("rejects an envelope whose revision disagrees with its delivery", async () => {
    const invalid = delivery();
    invalid.envelope.revision = 3;
    vi.mocked(fetch).mockResolvedValue(
      jsonResponse({ items: [invalid], nextCursor: 7, hasMore: false }),
    );

    await expect(
      listSessionDeliveries({
        serverUrl: "https://capture.example.test",
        accessToken: "access-token",
        workspaceId: "workspace-1",
        consumerId: "device-1",
        after: 0,
      }),
    ).rejects.toMatchObject({
      code: "invalid_response",
    } satisfies Partial<EnterpriseCaptureClientError>);
  });

  it("acknowledges the exact stored revision and content hash", async () => {
    vi.mocked(fetch).mockResolvedValue(jsonResponse({ acknowledged: true }));

    await acknowledgeSessionDelivery({
      serverUrl: "https://capture.example.test",
      accessToken: "access-token",
      workspaceId: "workspace-1",
      consumerId: "device-1",
      jobId: "job-1",
      revision: 2,
      contentHash: "a".repeat(64),
    });

    const [url, init] = vi.mocked(fetch).mock.calls[0]!;
    expect(url).toContain("/session-envelopes/job-1/ack");
    expect(init?.method).toBe("POST");
    expect(JSON.parse(String(init?.body))).toEqual({
      consumerId: "device-1",
      revision: 2,
      contentHash: "a".repeat(64),
    });
  });

  it("rejects an oversized response without a declared content length", async () => {
    const chunk = new Uint8Array(1024 * 1024);
    let chunksRead = 0;
    vi.mocked(fetch).mockResolvedValue(
      new Response(
        new ReadableStream({
          pull(controller) {
            chunksRead += 1;
            controller.enqueue(chunk);
          },
        }),
        { headers: { "Content-Type": "application/json" } },
      ),
    );

    await expect(
      listSessionDeliveries({
        serverUrl: "https://capture.example.test",
        accessToken: "access-token",
        workspaceId: "workspace-1",
        consumerId: "device-1",
        after: 0,
      }),
    ).rejects.toMatchObject({
      code: "response_too_large",
    } satisfies Partial<EnterpriseCaptureClientError>);
    expect(chunksRead).toBeGreaterThanOrEqual(25);
    expect(chunksRead).toBeLessThanOrEqual(26);
  });

  it("keeps the request timeout active while reading the response body", async () => {
    vi.useFakeTimers();
    try {
      vi.mocked(fetch).mockImplementation(async (_url, init) => {
        const signal = init?.signal;
        return new Response(
          new ReadableStream({
            start(controller) {
              signal?.addEventListener("abort", () => {
                controller.error(new DOMException("aborted", "AbortError"));
              });
            },
          }),
          { headers: { "Content-Type": "application/json" } },
        );
      });
      const result = expect(
        listSessionDeliveries({
          serverUrl: "https://capture.example.test",
          accessToken: "access-token",
          workspaceId: "workspace-1",
          consumerId: "device-1",
          after: 0,
        }),
      ).rejects.toMatchObject({
        code: "request_timeout",
      } satisfies Partial<EnterpriseCaptureClientError>);

      await vi.advanceTimersByTimeAsync(30_000);
      await result;
    } finally {
      vi.useRealTimers();
    }
  });
});
