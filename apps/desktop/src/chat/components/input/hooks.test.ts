import { describe, expect, test } from "vitest";

import type { JSONContent } from "@hypr/tiptap/chat";

import { serializeDraftMessage } from "./draft";

describe("serializeDraftMessage", () => {
  test("serializes mention labels into plain text and draft refs", () => {
    const json: JSONContent = {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            {
              type: "mention-@",
              attrs: {
                id: "human-1",
                type: "human",
                label: "john",
              },
            },
            {
              type: "text",
              text: " is this",
            },
          ],
        },
      ],
    };

    expect(serializeDraftMessage(json)).toEqual({
      text: "@john is this",
      refs: [
        {
          kind: "human",
          key: "human:manual:human-1",
          source: "draft",
          humanId: "human-1",
        },
      ],
    });
  });

  test("dedupes refs while preserving repeated mention text", () => {
    const json: JSONContent = {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            {
              type: "mention",
              attrs: {
                id: "org-1",
                type: "organization",
                label: "Acme",
              },
            },
            {
              type: "text",
              text: " and ",
            },
            {
              type: "mention-@",
              attrs: {
                id: "org-1",
                type: "organization",
                label: "Acme",
              },
            },
          ],
        },
      ],
    };

    expect(serializeDraftMessage(json)).toEqual({
      text: "@Acme and @Acme",
      refs: [
        {
          kind: "organization",
          key: "organization:manual:org-1",
          source: "draft",
          organizationId: "org-1",
        },
      ],
    });
  });
});
