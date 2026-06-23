import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { RawEditor } from "./raw";

const hoisted = vi.hoisted(() => ({
  rawMd: JSON.stringify({ type: "doc", content: [] }),
  sessionTitle: "Weekly sync",
  persistChange: vi.fn(),
  noteEditorProps: [] as Record<string, unknown>[],
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
  useFileUpload: () => vi.fn(),
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
});
