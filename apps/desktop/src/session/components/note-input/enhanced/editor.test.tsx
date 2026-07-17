import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
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
  noteEditorProps: [] as Record<string, unknown>[],
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

vi.mock("@hypr/editor/markdown", () => ({
  parseJsonContent: (value: string) => JSON.parse(value),
}));

vi.mock("@hypr/editor/note", () => ({
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

  it("does not rerender the editor when its props are unchanged", () => {
    const view = render(
      <EnhancedEditor
        sessionId="session-1"
        enhancedNoteId="note-1"
        content={hoisted.content}
      />,
    );

    view.rerender(
      <EnhancedEditor
        sessionId="session-1"
        enhancedNoteId="note-1"
        content={hoisted.content}
      />,
    );

    expect(hoisted.noteEditorProps).toHaveLength(1);
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

  it("keeps non-audio files available when audio is dropped with attachments", () => {
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
    const audioFile = { name: "clip.mp3", type: "audio/mpeg" } as File;
    const imageFile = { name: "photo.png", type: "image/png" } as File;

    expect(fileHandlerConfig.onDrop([audioFile, imageFile])).toEqual({
      remainingFiles: [imageFile],
    });
    expect(hoisted.processAudioFile).toHaveBeenCalledTimes(1);
    expect(hoisted.processAudioFile).toHaveBeenCalledWith(audioFile);
  });

  it("only imports the first audio file from a multi-audio drop", () => {
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
    const firstAudioFile = { name: "first.mp3", type: "audio/mpeg" } as File;
    const secondAudioFile = { name: "second.m4a", type: "" } as File;

    expect(fileHandlerConfig.onDrop([firstAudioFile, secondAudioFile])).toEqual(
      {
        remainingFiles: [secondAudioFile],
      },
    );
    expect(hoisted.processAudioFile).toHaveBeenCalledTimes(1);
    expect(hoisted.processAudioFile).toHaveBeenCalledWith(firstAudioFile);
  });

  it("shows an audio upload overlay and intercepts audio drops", async () => {
    render(
      <EnhancedEditor
        sessionId="session-1"
        enhancedNoteId="note-1"
        content={hoisted.content}
      />,
    );

    const file = new File(["audio"], "clip.m4a", { type: "" });
    const dataTransfer = audioDataTransfer(file);
    const dropTarget = screen.getByText("Note editor").parentElement;

    expect(dropTarget).not.toBeNull();
    fireEvent.dragEnter(dropTarget!, { dataTransfer });

    expect(
      screen.getByText("Drop to upload and transcribe audio"),
    ).not.toBeNull();
    expect(
      screen.getByText("WAV, MP3, OGG, MP4, M4A, FLAC, WEBM, or AAC audio"),
    ).not.toBeNull();
    await waitFor(() => expect(hoisted.focusWindow).toHaveBeenCalledTimes(1));
    expect(hoisted.showWindow).toHaveBeenCalledTimes(1);
    expect(hoisted.unminimizeWindow).toHaveBeenCalledTimes(1);

    fireEvent.drop(dropTarget!, { dataTransfer });

    expect(hoisted.processAudioFile).toHaveBeenCalledWith(file);
    expect(
      screen.queryByText("Drop to upload and transcribe audio"),
    ).toBeNull();
  });
});

function audioDataTransfer(file: File) {
  return {
    files: [file],
    items: [
      {
        kind: "file",
        type: file.type,
        getAsFile: () => file,
      },
    ],
    types: ["Files"],
    dropEffect: "none",
  } as unknown as DataTransfer;
}
