import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { RawEditor } from "./raw";

const hoisted = vi.hoisted(() => ({
  rawMd: JSON.stringify({ type: "doc", content: [] }),
  sessionTitle: "Weekly sync",
  persistChange: vi.fn(),
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
  NoteEditor: (props: Record<string, unknown>) => {
    hoisted.noteEditorProps.push(props);

    return <div>Note editor</div>;
  },
}));

vi.mock("@hypr/plugin-analytics", () => ({
  commands: {
    event: vi.fn(),
  },
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

vi.mock("~/session/components/shared", () => ({
  hasStoredNoteContent: (value: unknown) => Boolean(value),
}));

vi.mock("~/session/raw-editor-sync", () => ({
  emitRawEditorSync: vi.fn(),
}));

vi.mock("~/shared/hooks/useFileUpload", () => ({
  useFileUpload: () => hoisted.fileUpload,
}));

vi.mock("~/stt/useUploadFile", () => ({
  AUDIO_EXTENSIONS: ["wav", "mp3", "ogg", "mp4", "m4a", "flac"],
  isAudioUploadFile: (file: Pick<File, "name" | "type">) =>
    file.type.startsWith("audio/") ||
    ["wav", "mp3", "ogg", "mp4", "m4a", "flac"].some((extension) =>
      file.name.endsWith(`.${extension}`),
    ),
  useUploadFile: () => ({ processAudioFile: hoisted.processAudioFile }),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  UI: {
    useCell: (table: string, _row: string, cell: string) => {
      if (table === "sessions" && cell === "raw_md") {
        return hoisted.rawMd;
      }

      if (table === "sessions" && cell === "title") {
        return hoisted.sessionTitle;
      }

      return undefined;
    },
    useSetPartialRowCallback: () => hoisted.persistChange,
  },
}));

describe("RawEditor", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    hoisted.noteEditorProps = [];
    hoisted.rawMd = JSON.stringify({ type: "doc", content: [] });
    hoisted.sessionTitle = "Weekly sync";
    hoisted.persistChange = vi.fn();
    hoisted.fileUpload = vi.fn();
    hoisted.processAudioFile = vi.fn();
    hoisted.showWindow.mockReset();
    hoisted.unminimizeWindow.mockReset();
    hoisted.focusWindow.mockReset();
    hoisted.showWindow.mockResolvedValue(undefined);
    hoisted.unminimizeWindow.mockResolvedValue(undefined);
    hoisted.focusWindow.mockResolvedValue(undefined);
  });

  it("uses the shared session note editor styling", () => {
    render(<RawEditor sessionId="session-1" className="custom-editor-class" />);

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];

    expect(props?.className).toContain("session-note-editor");
    expect(props?.className).toContain("custom-editor-class");
    expect(props?.initialContent).toMatchObject({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Weekly sync" }],
        },
      ],
    });
  });

  it("routes dropped audio files to transcription", () => {
    render(<RawEditor sessionId="session-1" />);

    const props = hoisted.noteEditorProps[hoisted.noteEditorProps.length - 1];
    const fileHandlerConfig = props?.fileHandlerConfig as {
      onDrop: (files: File[]) => boolean | void;
    };
    const file = { name: "clip.mp3", type: "audio/mpeg" } as File;

    expect(fileHandlerConfig.onDrop([file])).toBe(true);
    expect(hoisted.processAudioFile).toHaveBeenCalledWith(file);
  });

  it("shows an audio upload overlay and intercepts audio drops", async () => {
    render(<RawEditor sessionId="session-1" />);

    const file = new File(["audio"], "clip.flac", { type: "" });
    const dataTransfer = audioDataTransfer(file);
    const dropTarget = screen.getByText("Note editor").parentElement;

    expect(dropTarget).not.toBeNull();
    fireEvent.dragEnter(dropTarget!, { dataTransfer });

    expect(
      screen.getByText("Drop to upload and transcribe audio"),
    ).not.toBeNull();
    expect(
      screen.getByText("WAV, MP3, OGG, MP4, M4A, or FLAC audio"),
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
