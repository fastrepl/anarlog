import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { EnhancedEditor as SessionEnhancedEditor } from "./editor";

const hoisted = vi.hoisted(() => ({
  content: JSON.stringify({ type: "doc", content: [] }),
  sessionTitle: "Weekly sync",
  persistContent: vi.fn(() => Promise.resolve()),
  fileUpload: vi.fn(),
  processAudioFile: vi.fn(),
  showWindow: vi.fn(),
  unminimizeWindow: vi.fn(),
  focusWindow: vi.fn(),
  startCommentDraft: vi.fn(),
  commentDraft: null as Record<string, unknown> | null,
  noteEditorProps: [] as Record<string, unknown>[],
}));

vi.mock("@anlg/plugin-auth", () => ({
  commands: {
    getItem: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  },
}));

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => true,
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    show: hoisted.showWindow,
    unminimize: hoisted.unminimizeWindow,
    setFocus: hoisted.focusWindow,
  }),
}));

vi.mock("@anlg/editor/markdown", () => ({
  parseJsonContent: (value: string) =>
    value
      ? JSON.parse(value)
      : { type: "doc", content: [{ type: "paragraph" }] },
}));

vi.mock("@anlg/editor/note", () => ({
  normalizePortableAttachmentUrls: (value: unknown) => value,
  NoteEditor: (props: Record<string, unknown>) => {
    hoisted.noteEditorProps.push(props);

    return <div>Note editor</div>;
  },
}));

vi.mock("~/session/hooks/useAttachmentResolver", () => ({
  useAttachmentResolver: () => () => null,
}));

vi.mock("~/editor-bridge/app-link-view", () => ({
  AppLinkView: () => null,
}));

vi.mock("~/editor-bridge/mention-config", () => ({
  useMentionConfig: () => ({ users: [] }),
}));

vi.mock("~/editor-bridge/open-editor-link", () => ({
  openEditorLink: vi.fn(),
}));

vi.mock("~/editor-bridge/session-mention-drop", () => ({
  sessionMentionDropConfig: { read: () => null },
}));

vi.mock("~/editor-bridge/session-view", () => ({
  SessionNodeView: () => null,
}));

vi.mock("~/shared/hooks/useFileUpload", () => ({
  useFileUpload: () => hoisted.fileUpload,
}));

vi.mock("~/stt/useUploadFile", () => ({
  AUDIO_EXTENSIONS: ["wav", "mp3", "ogg", "mp4", "m4a", "flac", "webm", "aac"],
  isAudioUploadFile: (file: Pick<File, "name" | "type">) =>
    file.type.startsWith("audio/") ||
    ["wav", "mp3", "ogg", "mp4", "m4a", "flac", "webm", "aac"].some(
      (extension) => file.name.endsWith(`.${extension}`),
    ),
  useUploadFile: () => ({ processAudioFile: hoisted.processAudioFile }),
}));

vi.mock("~/session-sharing/comments", () => ({
  SessionCommentsLayer: () => <div data-testid="summary-comments-layer" />,
  useOwnedSessionComments: () => ({
    containerRef: { current: null },
    onCommentAnchorsEvent: vi.fn(),
    onViewReady: vi.fn(),
    onViewDisposed: vi.fn(),
    draft: hoisted.commentDraft,
    selection: {},
    startDraft: hoisted.startCommentDraft,
  }),
}));

vi.mock("~/session/queries", () => ({
  useEnhancedNote: () => ({ content: hoisted.content }),
  useUpdateEnhancedNoteContent: () => hoisted.persistContent,
}));

function EnhancedEditor(
  props: Omit<
    React.ComponentProps<typeof SessionEnhancedEditor>,
    "sessionTitle"
  >,
) {
  return (
    <SessionEnhancedEditor {...props} sessionTitle={hoisted.sessionTitle} />
  );
}

describe("EnhancedEditor", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    hoisted.noteEditorProps = [];
    hoisted.content = JSON.stringify({ type: "doc", content: [] });
    hoisted.sessionTitle = "Weekly sync";
    hoisted.persistContent = vi.fn(() => Promise.resolve());
    hoisted.fileUpload = vi.fn();
    hoisted.processAudioFile = vi.fn();
    hoisted.commentDraft = null;
    hoisted.showWindow.mockReset();
    hoisted.unminimizeWindow.mockReset();
    hoisted.focusWindow.mockReset();
    hoisted.showWindow.mockResolvedValue(undefined);
    hoisted.unminimizeWindow.mockResolvedValue(undefined);
    hoisted.focusWindow.mockResolvedValue(undefined);
  });

  it("shows the session title as the first line for persisted notes", () => {
    hoisted.content = JSON.stringify({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Summary Section" }],
        },
      ],
    });

    render(
      <EnhancedEditor
        sessionId="session-1"
        enhancedNoteId="note-1"
        content={hoisted.content}
      />,
    );

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];

    expect(props?.className).toContain("session-note-editor");
    expect(props?.className).toContain("enhanced-summary-editor");
    expect(props?.onCommentAnchorsEvent).toEqual(expect.any(Function));
    expect(props?.onCommentSelection).toBe(hoisted.startCommentDraft);
    expect(screen.getByTestId("summary-comments-layer")).toBeTruthy();
    expect(props?.placeholderComponent).toEqual(expect.any(Function));
    expect(props?.syncContentWhenFocused).toBe(false);
    expect(props?.handleChange).not.toBe(hoisted.persistContent);
    expect(props?.taskSource).toEqual({ type: "enhanced_note", id: "note-1" });
    expect(props?.initialContent).toMatchObject({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Weekly sync" }],
        },
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Summary Section" }],
        },
      ],
    });
  });

  it("hides the selection comment action while a draft is open", () => {
    hoisted.commentDraft = {};

    render(
      <EnhancedEditor
        sessionId="session-1"
        enhancedNoteId="note-1"
        content={hoisted.content}
      />,
    );

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];
    expect(props?.onCommentSelection).toBeUndefined();
  });

  it("persists content and updates the session title from the first line", () => {
    render(
      <EnhancedEditor
        sessionId="session-1"
        enhancedNoteId="note-1"
        content={hoisted.content}
      />,
    );

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];
    const input = {
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Edited title" }],
        },
      ],
    };

    (props?.handleChange as (input: unknown) => void)(input);

    expect(hoisted.persistContent).toHaveBeenCalledWith(
      JSON.stringify(input),
      "Edited title",
    );
  });

  it("does not persist the empty title layout for a new summary", () => {
    hoisted.content = "";
    hoisted.sessionTitle = "";

    render(
      <EnhancedEditor
        sessionId="session-1"
        enhancedNoteId="note-1"
        content={hoisted.content}
      />,
    );

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];
    const input = {
      type: "doc",
      content: [
        { type: "heading", attrs: { level: 1 } },
        { type: "paragraph" },
      ],
    };

    (props?.handleChange as (input: unknown) => void)(input);

    expect(hoisted.persistContent).not.toHaveBeenCalled();
  });

  it("does not persist a synthesized session title for a new summary", () => {
    hoisted.content = "";
    hoisted.sessionTitle = "Weekly sync";

    render(
      <EnhancedEditor
        sessionId="session-1"
        enhancedNoteId="note-1"
        content={hoisted.content}
      />,
    );

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];
    const input = {
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Weekly sync" }],
        },
        { type: "paragraph" },
      ],
    };

    (props?.handleChange as (input: unknown) => void)(input);

    expect(hoisted.persistContent).not.toHaveBeenCalled();
  });

  it("persists formatting applied to a synthesized session title", () => {
    hoisted.content = "";
    hoisted.sessionTitle = "Weekly sync";

    render(
      <EnhancedEditor
        sessionId="session-1"
        enhancedNoteId="note-1"
        content={hoisted.content}
      />,
    );

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];
    const input = {
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [
            {
              type: "text",
              text: "Weekly sync",
              marks: [{ type: "bold" }],
            },
          ],
        },
        { type: "paragraph" },
      ],
    };

    (props?.handleChange as (input: unknown) => void)(input);

    expect(hoisted.persistContent).toHaveBeenCalledWith(
      JSON.stringify(input),
      "Weekly sync",
    );
  });

  it("persists clearing an existing summary", () => {
    hoisted.content = JSON.stringify({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Existing summary" }],
        },
      ],
    });
    hoisted.sessionTitle = "";

    render(
      <EnhancedEditor
        sessionId="session-1"
        enhancedNoteId="note-1"
        content={hoisted.content}
      />,
    );

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];
    const input = {
      type: "doc",
      content: [
        { type: "heading", attrs: { level: 1 } },
        { type: "paragraph" },
      ],
    };

    (props?.handleChange as (input: unknown) => void)(input);

    expect(hoisted.persistContent).toHaveBeenCalledWith(
      JSON.stringify(input),
      "",
    );
  });

  it("persists an attachment added to a new summary", () => {
    hoisted.content = "";
    hoisted.sessionTitle = "";

    render(
      <EnhancedEditor
        sessionId="session-1"
        enhancedNoteId="note-1"
        content={hoisted.content}
      />,
    );

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];
    const input = {
      type: "doc",
      content: [
        { type: "heading", attrs: { level: 1 } },
        {
          type: "fileAttachment",
          attrs: { attachmentId: "attachment-1", name: "notes.pdf" },
        },
      ],
    };

    (props?.handleChange as (input: unknown) => void)(input);

    expect(hoisted.persistContent).toHaveBeenCalledWith(
      JSON.stringify(input),
      undefined,
    );
  });

  it("keeps streamed previews syncing while focused", () => {
    const contentOverride = {
      type: "doc",
      content: [
        { type: "paragraph", content: [{ type: "text", text: "Generating" }] },
      ],
    };

    render(
      <EnhancedEditor
        sessionId="session-1"
        enhancedNoteId="note-1"
        content={hoisted.content}
        contentOverride={contentOverride}
      />,
    );

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];

    expect(props?.syncContentWhenFocused).toBe(true);
    expect(props?.handleChange).toBeUndefined();
    expect(props?.taskSource).toBeUndefined();
    expect(props?.initialContent).toMatchObject({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Weekly sync" }],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "Generating" }],
        },
      ],
    });
  });

  it("routes dropped audio files to transcription", () => {
    render(
      <EnhancedEditor
        sessionId="session-1"
        enhancedNoteId="note-1"
        content={hoisted.content}
      />,
    );

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];
    const fileHandlerConfig = props?.fileHandlerConfig as {
      onDrop: (files: File[]) => boolean | void | { remainingFiles: File[] };
    };
    const file = { name: "clip.mp3", type: "audio/mpeg" } as File;

    expect(fileHandlerConfig.onDrop([file])).toBe(true);
    expect(hoisted.processAudioFile).toHaveBeenCalledWith(file);
  });
});
