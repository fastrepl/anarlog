import { describe, expect, it, vi } from "vitest";

import {
  buildSlackRecap,
  listSlackChannels,
  sendSessionShareRecapEmail,
} from "./delivery-client";

function session() {
  return { access_token: "token" } as Parameters<
    typeof sendSessionShareRecapEmail
  >[0]["session"];
}

describe("meeting recap delivery client", () => {
  it("sends recap email through the authenticated shared-note endpoint", async () => {
    const fetcher = vi
      .fn()
      .mockResolvedValue(new Response(null, { status: 204 }));

    await sendSessionShareRecapEmail({
      apiBaseUrl: "https://api.anarlog.so",
      session: session(),
      shareId: "share-id",
      recipients: ["one@example.com"],
      senderName: "Owner",
      noteTitle: "Planning",
      noteBody: "Ship it.",
      deliveryId: "delivery-id",
      fetcher,
    });

    expect(fetcher).toHaveBeenCalledWith(
      new URL("https://api.anarlog.so/shared-notes/share-id/recap/email"),
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          recipients: ["one@example.com"],
          senderName: "Owner",
          noteTitle: "Planning",
          noteBody: "Ship it.",
          deliveryId: "delivery-id",
        }),
      }),
    );
  });

  it("parses Slack channels and preserves private-channel state", async () => {
    const fetcher = vi.fn().mockResolvedValue(
      Response.json({
        channels: [
          { id: "C1", name: "general", is_private: false },
          { id: "G1", name: "leadership", is_private: true },
        ],
      }),
    );

    await expect(
      listSlackChannels({
        apiBaseUrl: "https://api.anarlog.so",
        accessToken: "token",
        fetcher,
      }),
    ).resolves.toEqual([
      { id: "C1", name: "general", isPrivate: false },
      { id: "G1", name: "leadership", isPrivate: true },
    ]);
  });

  it("keeps Slack messages within the API limit", () => {
    const recap = buildSlackRecap({
      senderName: "Owner",
      noteTitle: "Planning",
      noteBody: "x".repeat(50_000),
    });

    expect(recap.length).toBeLessThanOrEqual(40_000);
    expect(recap).toContain("Sent by Owner via Anarlog");
  });
});
