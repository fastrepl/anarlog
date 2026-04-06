import { cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ExportPDF } from "./export-pdf";

const {
  useMutationMock,
  useTranscriptExportSegmentsMock,
  useStoreMock,
  useQueriesMock,
  useCellMock,
  useSliceRowIdsMock,
  useSessionEventMock,
} = vi.hoisted(() => ({
  useMutationMock: vi.fn(),
  useTranscriptExportSegmentsMock: vi.fn(),
  useStoreMock: vi.fn(),
  useQueriesMock: vi.fn(),
  useCellMock: vi.fn(),
  useSliceRowIdsMock: vi.fn(),
  useSessionEventMock: vi.fn(),
}));

vi.mock("@tanstack/react-query", () => ({
  useMutation: useMutationMock,
}));

vi.mock("@tauri-apps/api/path", () => ({
  downloadDir: vi.fn(),
  join: vi.fn(),
}));

vi.mock("lucide-react", () => ({
  FileTextIcon: () => <span data-testid="file-icon" />,
  Loader2Icon: () => <span data-testid="loader-icon" />,
}));

vi.mock("@hypr/plugin-analytics", () => ({
  commands: { event: vi.fn() },
}));

vi.mock("@hypr/plugin-export", () => ({
  commands: { export: vi.fn() },
}));

vi.mock("@hypr/plugin-opener2", () => ({
  commands: { revealItemInDir: vi.fn() },
}));

vi.mock("@hypr/tiptap/shared", () => ({
  json2md: vi.fn(() => ""),
}));

vi.mock("@hypr/ui/components/ui/dropdown-menu", () => ({
  DropdownMenuItem: ({
    disabled,
    onClick,
    children,
  }: {
    disabled?: boolean;
    onClick?: (event: { preventDefault: () => void }) => void;
    children: ReactNode;
  }) => (
    <button
      disabled={disabled}
      onClick={() => onClick?.({ preventDefault: () => undefined })}
      type="button"
    >
      {children}
    </button>
  ),
}));

vi.mock("./export-utils", () => ({
  formatDate: vi.fn(() => "Mar 23"),
  formatDuration: vi.fn(() => "1m"),
}));

vi.mock("~/session/components/note-input/transcript/export-data", () => ({
  useTranscriptExportSegments: useTranscriptExportSegmentsMock,
}));

vi.mock("~/store/tinybase/hooks", () => ({
  useSessionEvent: useSessionEventMock,
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  INDEXES: {
    transcriptBySession: "transcriptBySession",
  },
  QUERIES: {
    sessionParticipantsWithDetails: "sessionParticipantsWithDetails",
  },
  UI: {
    useStore: useStoreMock,
    useQueries: useQueriesMock,
    useCell: useCellMock,
    useSliceRowIds: useSliceRowIdsMock,
  },
}));

describe("ExportPDF", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    useMutationMock.mockReturnValue({
      mutate: vi.fn(),
      isPending: false,
    });
    useTranscriptExportSegmentsMock.mockReturnValue({
      data: [],
      isLoading: true,
    });
    useStoreMock.mockReturnValue(null);
    useQueriesMock.mockReturnValue(null);
    useCellMock.mockReturnValue(undefined);
    useSliceRowIdsMock.mockReturnValue([]);
    useSessionEventMock.mockReturnValue(null);
  });

  it("does not block non-transcript PDF export on transcript loading", () => {
    render(
      <ExportPDF
        sessionId="session-1"
        currentView={{ type: "raw" } as never}
      />,
    );

    const button = screen.getByRole("button");
    expect(button).toHaveProperty("disabled", false);
    expect(screen.getByText("Export Memo to PDF")).not.toBeNull();
    expect(screen.getByTestId("file-icon")).not.toBeNull();
  });
});
