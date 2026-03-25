import { renderHook } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";

import type { TiptapEditor } from "@hypr/tiptap/chat";
import type { JSONContent } from "@hypr/tiptap/chat";

import { serializeDraftMessage } from "./draft";
import { useSyncDraftStateFromEditor } from "./hooks";

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
          label: "john",
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
          label: "Acme",
          source: "draft",
          organizationId: "org-1",
        },
      ],
    });
  });
});

describe("useSyncDraftStateFromEditor", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  test("syncs draft refs from the mounted editor state", async () => {
    const getJSON = vi.fn(() => ({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            {
              type: "mention",
              attrs: {
                id: "human-1",
                type: "human",
                label: "John",
              },
            },
          ],
        },
      ],
    }));
    const handleEditorUpdate = vi.fn();
    const editorRef = {
      current: {
        editor: {
          isDestroyed: false,
          isInitialized: true,
          getJSON,
        } as unknown as TiptapEditor,
      },
    } as React.RefObject<{ editor: TiptapEditor | null } | null>;

    renderHook(() =>
      useSyncDraftStateFromEditor({ editorRef, handleEditorUpdate }),
    );

    await vi.waitFor(() => expect(handleEditorUpdate).toHaveBeenCalledTimes(1));
    expect(getJSON).toHaveBeenCalledTimes(1);
    expect(handleEditorUpdate).toHaveBeenCalledWith({
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            {
              type: "mention",
              attrs: {
                id: "human-1",
                type: "human",
                label: "John",
              },
            },
          ],
        },
      ],
    });
  });
});
