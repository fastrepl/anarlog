import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

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

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
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

  it("skips oversized base64 data URL images when compression is unavailable", async () => {
    const oversized = "a".repeat(350 * 1024);

    const images = await collectEnhanceImageContext(
      "session-1",
      `![pasted](data:image/png;base64,${oversized})`,
    );

    expect(images).toEqual([]);
    expect(fsSyncMocks.attachmentList).not.toHaveBeenCalled();
  });

  it("compresses oversized base64 data URL images into budget", async () => {
    const originalCreateElement = document.createElement.bind(document);
    vi.spyOn(document, "createElement").mockImplementation(
      (tagName: string, options?: ElementCreationOptions) => {
        if (tagName !== "canvas") {
          return originalCreateElement(tagName, options);
        }

        return {
          width: 0,
          height: 0,
          getContext: () => ({
            clearRect: vi.fn(),
            drawImage: vi.fn(),
          }),
          toDataURL: () => "data:image/jpeg;base64,aGVsbG8=",
        } as unknown as HTMLCanvasElement;
      },
    );
    vi.stubGlobal(
      "createImageBitmap",
      vi.fn(async () => ({
        width: 2400,
        height: 1600,
        close: vi.fn(),
      })),
    );

    const images = await collectEnhanceImageContext(
      "session-1",
      `![pasted](data:image/png;base64,${"a".repeat(350 * 1024)})`,
    );

    expect(images).toEqual([
      { base64: "aGVsbG8=", mimeType: "image/jpeg", filename: undefined },
    ]);
  });

  it("keeps image context within the aggregate byte budget", async () => {
    const image = "a".repeat(170 * 1024);

    const images = await collectEnhanceImageContext(
      "session-1",
      [
        `![one](data:image/png;base64,${image})`,
        `![two](data:image/png;base64,${image})`,
        `![three](data:image/png;base64,${image})`,
        `![four](data:image/png;base64,${image})`,
        `![five](data:image/png;base64,${image})`,
        `![six](data:image/png;base64,${image})`,
        `![seven](data:image/png;base64,${image})`,
      ].join("\n"),
    );

    expect(images).toHaveLength(6);
  });

  it("samples image context across the full note when over the image count cap", async () => {
    const images = await collectEnhanceImageContext(
      "session-1",
      Array.from(
        { length: 20 },
        (_, index) =>
          `![image-${index}](data:image/png;base64,image${String(index).padStart(2, "0")})`,
      ).join("\n"),
    );

    expect(images).toEqual(
      [0, 2, 4, 6, 8, 11, 13, 15, 17, 19].map((index) => ({
        base64: `image${String(index).padStart(2, "0")}`,
        mimeType: "image/png",
      })),
    );
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
