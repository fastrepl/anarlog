import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { ChatEditorHandle, JSONContent } from "@anlg/editor/chat";

import { extractContextRefsFromTiptapJson, useSubmit } from "./hooks";

function docWithMention(type: string, id: string) {
  return {
    type: "doc",
    content: [
      {
        type: "paragraph",
        content: [
          { type: "text", text: "find action items in " },
          {
            type: "mention-@",
            attrs: { id, type, label: id },
          },
        ],
      },
    ],
  } as never;
}

describe("extractContextRefsFromTiptapJson", () => {
  it("turns a folder mention into a removable manual folder ref", () => {
    expect(
      extractContextRefsFromTiptapJson(docWithMention("folder", "CS 101")),
    ).toEqual([
      {
        kind: "folder",
        key: "folder:manual:CS 101",
        source: "manual",
        folderId: "CS 101",
      },
    ]);
  });

  it("deduplicates repeated folder mentions", () => {
    const doc = {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [
            {
              type: "mention-@",
              attrs: { id: "CS 101", type: "folder", label: "CS 101" },
            },
            {
              type: "mention-@",
              attrs: { id: "CS 101", type: "folder", label: "CS 101" },
            },
          ],
        },
      ],
    } as never;

    expect(extractContextRefsFromTiptapJson(doc)).toHaveLength(1);
  });
});

describe("useSubmit", () => {
  const savedPrompt = {
    id: "prompt-1",
    title: "What should I say?",
    prompt: "Suggest my next line.",
    availability: "recording" as const,
  };
  const chip: JSONContent = {
    type: "mention-@",
    attrs: { id: "prompt-1", type: "chat_prompt", label: "What should I say?" },
  };

  function submit(content: JSONContent[], prompts = [savedPrompt]) {
    const onSendMessage = vi.fn();
    const editor: ChatEditorHandle = {
      focus: () => true,
      getJSON: () => ({
        type: "doc",
        content: [{ type: "paragraph", content }],
      }),
      clearContent: vi.fn(),
      insertText: vi.fn(),
      replaceContent: vi.fn(),
    };
    const { result } = renderHook(() =>
      useSubmit({
        draftKey: "draft",
        editorRef: { current: editor },
        prompts,
        onSendMessage,
      }),
    );
    result.current();
    return onSendMessage;
  }

  it("sends a prompt chip as its name plus a snapshot part instead of the full prompt text", () => {
    expect(
      submit([chip, { type: "text", text: " focus on the API question" }]),
    ).toHaveBeenCalledWith(
      "What should I say? focus on the API question",
      [
        {
          type: "data-chat-prompt",
          data: {
            id: "prompt-1",
            title: "What should I say?",
            prompt: "Suggest my next line.",
          },
        },
        { type: "text", text: "focus on the API question" },
      ],
      [],
    );

    expect(submit([chip])).toHaveBeenCalledWith(
      "What should I say?",
      [
        {
          type: "data-chat-prompt",
          data: {
            id: "prompt-1",
            title: "What should I say?",
            prompt: "Suggest my next line.",
          },
        },
      ],
      [],
    );
  });

  it("keeps a chip whose saved prompt was deleted as visible text", () => {
    expect(
      submit([chip, { type: "text", text: " focus on the API question" }], []),
    ).toHaveBeenCalledWith(
      "/What should I say? focus on the API question",
      [
        {
          type: "text",
          text: "/What should I say? focus on the API question",
        },
      ],
      [],
    );
  });
});
