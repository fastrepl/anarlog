import { cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  session: { user: { id: "viewer-1" } } as any,
  query: {
    data: null as any,
    isLoading: false,
    error: null as Error | null,
  },
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: mocks.session }),
}));

vi.mock("~/shared-notes/cache", () => ({
  useDurableSharedNote: () => mocks.query,
}));

vi.mock("~/session/components/session-surface", () => ({
  SessionSurface: ({
    header,
    children,
  }: {
    header?: ReactNode;
    children: ReactNode;
  }) => (
    <div>
      {header}
      {children}
    </div>
  ),
}));

vi.mock("@hypr/editor/note", () => ({
  NoteEditor: ({
    readOnly,
    showFormatToolbar,
    initialContent,
  }: {
    readOnly?: boolean;
    showFormatToolbar?: boolean;
    initialContent?: unknown;
  }) => (
    <div
      data-content={JSON.stringify(initialContent)}
      data-read-only={String(readOnly)}
      data-show-format-toolbar={String(showFormatToolbar)}
      data-testid="shared-note-editor"
    />
  ),
}));

vi.mock("~/editor-bridge/open-editor-link", () => ({
  openEditorLink: vi.fn(),
}));

import { TabContentSharedNote } from ".";

const tab = {
  type: "shared_sessions" as const,
  id: "share-1",
  active: true,
  slotId: "slot-1",
  pinned: false,
};

describe("TabContentSharedNote", () => {
  beforeEach(() => {
    mocks.session = { user: { id: "viewer-1" } };
    mocks.query = { data: null, isLoading: false, error: null };
  });

  afterEach(cleanup);

  it("renders cached content through the read-only editor", () => {
    mocks.query.data = {
      shareId: "share-1",
      workspaceId: "workspace-1",
      sessionId: "session-1",
      schemaVersion: 1,
      contentRevision: 2,
      title: "Shared plan",
      body: { type: "doc", content: [{ type: "paragraph" }] },
      capability: "viewer",
      manageAccess: false,
      accessVersion: 3,
      publishedAt: "2026-07-16T17:30:00.000Z",
    };

    render(<TabContentSharedNote tab={tab} />);

    expect(screen.getByText("Shared with me · View only")).toBeTruthy();
    expect(
      screen.getByTestId("shared-note-editor").getAttribute("data-read-only"),
    ).toBe("true");
    expect(
      screen
        .getByTestId("shared-note-editor")
        .getAttribute("data-show-format-toolbar"),
    ).toBe("false");
    expect(screen.getByTestId("shared-note-editor").dataset.content).toContain(
      "Shared plan",
    );
  });

  it("removes content when access is no longer cached", () => {
    render(<TabContentSharedNote tab={tab} />);

    expect(screen.getByText("Access no longer available")).toBeTruthy();
    expect(screen.queryByTestId("shared-note-editor")).toBeNull();
  });

  it("requires the account the note was shared with", () => {
    mocks.session = null;

    render(<TabContentSharedNote tab={tab} />);

    expect(screen.getByText("Sign in to view this shared note")).toBeTruthy();
  });
});
