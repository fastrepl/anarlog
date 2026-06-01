import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  collectEnhanceImageContext,
  collectImageReferences,
  getBase64ByteLength,
} from "./enhance-images";

const fsSyncMocks = vi.hoisted(() => ({
  attachmentList: vi.fn(),
  attachmentRead: vi.fn(),
}));

vi.mock("@hypr/plugin-fs-sync", () => ({
  commands: fsSyncMocks,
}));

describe("enhance image context", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    fsSyncMocks.attachmentList.mockResolvedValue({
      status: "ok",
      data: [
        {
          attachmentId: "diagram.png",
          path: "/vault/sessions/session-1/attachments/diagram.png",
          extension: "png",
          modifiedAt: "",
        },
        {
          attachmentId: "stale.png",
          path: "/vault/sessions/session-1/attachments/stale.png",
          extension: "png",
          modifiedAt: "",
        },
      ],
    });
    fsSyncMocks.attachmentRead.mockResolvedValue({
      status: "ok",
      data: [104, 101, 108, 108, 111],
    });
  });

  it("reads only image attachments referenced by note JSON", async () => {
    const rawContent = JSON.stringify({
      type: "doc",
      content: [
        {
          type: "image",
          attrs: {
            src: "asset://localhost/%2Fvault%2Fsessions%2Fsession-1%2Fattachments%2Fdiagram.png",
            attachmentId: "diagram.png",
          },
        },
      ],
    });

    const images = await collectEnhanceImageContext("session-1", rawContent);

    expect(images).toEqual([
      {
        base64: "aGVsbG8=",
        mimeType: "image/png",
        filename: "diagram.png",
      },
    ]);
    expect(fsSyncMocks.attachmentRead).toHaveBeenCalledWith(
      "session-1",
      "diagram.png",
    );
  });

  it("extracts markdown image filenames from asset URLs", () => {
    expect(
      collectImageReferences(
        "![diagram](asset://localhost/%2Fvault%2Fsessions%2Fsession-1%2Fattachments%2Fdiagram.png)",
      ),
    ).toEqual([{ filename: "diagram.png" }]);
  });

  it("does not treat remote markdown images as local attachments", () => {
    expect(
      collectImageReferences("![diagram](https://example.com/diagram.png)"),
    ).toEqual([{ filename: undefined }]);
  });

  it("keeps base64 data URL images without reading attachments", async () => {
    const images = await collectEnhanceImageContext(
      "session-1",
      "![pasted](data:image/png;base64,abc123)",
    );

    expect(images).toEqual([{ base64: "abc123", mimeType: "image/png" }]);
    expect(fsSyncMocks.attachmentList).not.toHaveBeenCalled();
  });

  it("does not load an attachment again for a node that already has a data URL", async () => {
    const rawContent = JSON.stringify({
      type: "doc",
      content: [
        {
          type: "image",
          attrs: {
            src: "data:image/png;base64,abc123",
            attachmentId: "diagram.png",
          },
        },
      ],
    });

    const images = await collectEnhanceImageContext("session-1", rawContent);

    expect(images).toEqual([{ base64: "abc123", mimeType: "image/png" }]);
    expect(fsSyncMocks.attachmentList).not.toHaveBeenCalled();
  });

  it("computes decoded base64 byte length before applying the data URL cap", () => {
    expect(getBase64ByteLength("aGVsbG8=")).toBe(5);
    expect(getBase64ByteLength("YW55IGNhcm5hbCBwbGVhcw==")).toBe(16);
  });
});
