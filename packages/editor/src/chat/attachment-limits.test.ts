import { describe, expect, it } from "vitest";

import {
  canRetainChatImage,
  estimateImageDataUrlBytes,
  MAX_CHAT_DRAFT_BYTES,
  MAX_CHAT_IMAGE_BYTES,
} from "./attachment-limits";

describe("chat attachment limits", () => {
  it("rejects an image before reading it when the file exceeds the image limit", () => {
    expect(
      canRetainChatImage({
        fileSize: MAX_CHAT_IMAGE_BYTES + 1,
        mimeType: "image/png",
        currentDraftBytes: 0,
        pendingImageBytes: 0,
      }),
    ).toBe(false);
  });

  it("accounts for base64 expansion and concurrently pending image reads", () => {
    const imageBytes = estimateImageDataUrlBytes(
      MAX_CHAT_IMAGE_BYTES,
      "image/png",
    );

    expect(
      canRetainChatImage({
        fileSize: MAX_CHAT_IMAGE_BYTES,
        mimeType: "image/png",
        currentDraftBytes: MAX_CHAT_DRAFT_BYTES - imageBytes - 512,
        pendingImageBytes: 0,
      }),
    ).toBe(true);
    expect(
      canRetainChatImage({
        fileSize: MAX_CHAT_IMAGE_BYTES,
        mimeType: "image/png",
        currentDraftBytes: MAX_CHAT_DRAFT_BYTES - imageBytes - 512,
        pendingImageBytes: 1,
      }),
    ).toBe(false);
  });
});
