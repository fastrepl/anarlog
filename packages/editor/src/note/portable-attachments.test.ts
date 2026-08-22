import { describe, expect, it } from "vitest";

import { normalizePortableAttachmentUrls } from "./portable-attachments";

describe("normalizePortableAttachmentUrls", () => {
  it("removes device-local image and file locations while keeping identities", () => {
    expect(
      normalizePortableAttachmentUrls({
        type: "doc",
        content: [
          {
            type: "image",
            attrs: {
              attachmentId: "diagram.png",
              src: "asset://localhost/device-a/diagram.png",
              alt: "Diagram",
            },
          },
          {
            type: "fileAttachment",
            attrs: {
              attachmentId: "notes.pdf",
              src: "file:///device-a/notes.pdf",
              path: "/device-a/notes.pdf",
              name: "notes.pdf",
            },
          },
          {
            type: "image",
            attrs: {
              attachmentId: "windows-diagram.png",
              src: "http://asset.localhost/C%3A/Users/Example/windows-diagram.png",
              path: "C:\\Users\\Example\\windows-diagram.png",
              alt: "Windows diagram",
            },
          },
        ],
      }),
    ).toEqual({
      type: "doc",
      content: [
        {
          type: "image",
          attrs: { attachmentId: "diagram.png", alt: "Diagram" },
        },
        {
          type: "fileAttachment",
          attrs: { attachmentId: "notes.pdf", name: "notes.pdf" },
        },
        {
          type: "image",
          attrs: {
            attachmentId: "windows-diagram.png",
            alt: "Windows diagram",
          },
        },
      ],
    });
  });

  it("preserves remote URLs and nodes without a catalog identity", () => {
    const document = {
      type: "doc",
      content: [
        {
          type: "image",
          attrs: {
            attachmentId: "remote-image",
            src: "https://example.com/image.png",
          },
        },
        {
          type: "image",
          attrs: { src: "asset://localhost/uncatalogued.png" },
        },
      ],
    };

    const normalized = normalizePortableAttachmentUrls(document);
    expect(normalized).toBe(document);
    expect(normalized).toEqual(document);
  });

  it("clones only branches containing local attachment locations", () => {
    const unchangedParagraph = {
      type: "paragraph",
      content: [{ type: "text", text: "Notes" }],
    };
    const document = {
      type: "doc",
      content: [
        unchangedParagraph,
        {
          type: "image",
          attrs: {
            attachmentId: "diagram.png",
            src: "asset://localhost/diagram.png",
          },
        },
      ],
    };

    const normalized = normalizePortableAttachmentUrls(document);

    expect(normalized).not.toBe(document);
    expect(normalized.content?.[0]).toBe(unchangedParagraph);
    expect(normalized.content?.[1]).toEqual({
      type: "image",
      attrs: { attachmentId: "diagram.png" },
    });
  });
});
