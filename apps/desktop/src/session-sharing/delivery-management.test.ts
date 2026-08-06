import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  sendEmail: vi.fn(),
  sendSlack: vi.fn(),
}));

vi.mock("./delivery-client", async (importOriginal) => {
  const original = await importOriginal<typeof import("./delivery-client")>();
  return {
    ...original,
    sendSessionShareRecapEmail: mocks.sendEmail,
    sendSlackRecap: mocks.sendSlack,
  };
});

import {
  deliverSessionShareRecapEmail,
  deliverSessionShareRecapToSlack,
} from "./delivery-management";

const body = {
  type: "doc",
  content: [
    {
      type: "paragraph",
      content: [{ type: "text", text: "Ship it." }],
    },
  ],
};

function context() {
  return {
    session: {
      access_token: "access-token",
      user: {
        email: "owner@example.com",
        user_metadata: { full_name: "Ada Lovelace" },
      },
    },
  } as unknown as Parameters<
    typeof deliverSessionShareRecapEmail
  >[0]["context"];
}

describe("meeting recap delivery", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal("crypto", { randomUUID: () => "delivery-id" });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("attributes Resend email to the signed-in owner", async () => {
    await deliverSessionShareRecapEmail({
      context: context(),
      shareId: "share-id",
      recipients: ["guest@example.com"],
      noteTitle: "Planning",
      body,
      signal: new AbortController().signal,
    });

    expect(mocks.sendEmail).toHaveBeenCalledWith(
      expect.objectContaining({
        recipients: ["guest@example.com"],
        senderName: "Ada Lovelace",
        noteTitle: "Planning",
        noteBody: "Ship it.",
        deliveryId: "delivery-id",
      }),
    );
  });

  it("posts an owner-attributed recap to the selected Slack channel", async () => {
    await deliverSessionShareRecapToSlack({
      context: context(),
      channel: { id: "C123", name: "general" },
      noteTitle: "Planning",
      body,
      signal: new AbortController().signal,
    });

    expect(mocks.sendSlack).toHaveBeenCalledWith(
      expect.objectContaining({
        accessToken: "access-token",
        channel: "C123",
        text: expect.stringContaining("Sent by Ada Lovelace via Anarlog"),
      }),
    );
  });
});
