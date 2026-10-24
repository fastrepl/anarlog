import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  settings: vi.fn(),
  storedValues: {} as Record<string, string | boolean | undefined>,
  settingsValues: {} as Record<string, string | boolean | undefined>,
  settingsLoading: false,
  persistSettings: vi.fn(),
  downloadDir: vi.fn(),
  exportPdf: vi.fn(),
  writeTextFile: vi.fn(),
  revealItemInDir: vi.fn(),
  onOpenChange: vi.fn(),
  save: vi.fn(),
  isAppStoreBuild: vi.fn(),
  sharedNote: undefined as { body: unknown } | undefined,
}));

vi.mock("@tauri-apps/api/path", () => ({
  downloadDir: mocks.downloadDir,
  join: async (...parts: string[]) => parts.join("/"),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: mocks.save }));
vi.mock("@anlg/plugin-export", () => ({
  commands: { export: mocks.exportPdf },
}));
vi.mock("@anlg/plugin-fs2", () => ({
  commands: { writeTextFile: mocks.writeTextFile },
}));
vi.mock("@anlg/plugin-opener2", () => ({
  commands: { revealItemInDir: mocks.revealItemInDir },
}));
vi.mock("~/settings/queries", () => ({
  getStoredSettingValues: mocks.settings,
  useStoredSettingValues: () => ({
    values: mocks.settingsValues,
    hasValues: new Set(),
  }),
  useStoredSettingValuesQuery: () => ({
    data: { values: mocks.storedValues, hasValues: new Set() },
    isLoading: mocks.settingsLoading,
    error: null,
  }),
  useSetSettingValues: () => mocks.persistSettings,
}));
vi.mock("~/shared/app-store", () => ({
  isAppStoreBuild: mocks.isAppStoreBuild,
}));
vi.mock("~/session/queries", () => ({
  useSession: () => ({ title: "Project review" }),
  useEnhancedNote: () => undefined,
  useSessionParticipants: () => [],
}));
vi.mock("~/session/utils", () => ({ getSessionEvent: () => null }));
vi.mock("~/session/components/note-input/transcript/export-data", () => ({
  useTranscriptExportSegments: () => ({ data: [], isLoading: false }),
}));
vi.mock("~/stt/queries", () => ({ useSessionTranscriptMetadata: () => [] }));
vi.mock("~/auth", () => ({
  useAuth: () => ({ session: { user: { id: "viewer" } } }),
}));
vi.mock("~/shared-notes/cache", () => ({
  useDurableSharedNote: (_viewer: string | null, shareId: string) => ({
    data: shareId ? mocks.sharedNote : undefined,
  }),
}));

import { ExportModal } from "./export-modal";

function renderModal(
  currentView: React.ComponentProps<typeof ExportModal>["currentView"] = {
    type: "raw",
  },
) {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false } },
  });
  const renderElement = () => (
    <QueryClientProvider client={client}>
      <ExportModal
        sessionId="session-1"
        currentView={currentView}
        open
        onOpenChange={mocks.onOpenChange}
      />
    </QueryClientProvider>
  );
  const view = render(renderElement());
  return { ...view, reload: () => view.rerender(renderElement()) };
}

describe("ExportModal", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mocks.settings.mockResolvedValue({ values: {} });
    mocks.storedValues = {};
    mocks.settingsValues = {};
    mocks.settingsLoading = false;
    mocks.downloadDir.mockResolvedValue("/Users/test/Downloads");
    mocks.exportPdf.mockResolvedValue({ status: "ok", data: null });
    mocks.writeTextFile.mockResolvedValue({ status: "ok", data: null });
    mocks.isAppStoreBuild.mockReturnValue(false);
  });
  afterEach(cleanup);

  it.each([
    ["PDF", "pdf"],
    ["TXT", "txt"],
    ["Markdown", "md"],
    ["Org", "org"],
  ])("writes %s to the saved export folder", async (label, extension) => {
    mocks.settings.mockResolvedValue({
      values: { export_directory: "/Volumes/Work/Exports" },
    });
    renderModal();
    fireEvent.click(screen.getByRole("radio", { name: label }));
    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    await waitFor(() => expect(mocks.onOpenChange).toHaveBeenCalledWith(false));
    const writer = extension === "pdf" ? mocks.exportPdf : mocks.writeTextFile;
    expect(writer).toHaveBeenCalledWith(
      expect.stringMatching(
        new RegExp(`^/Volumes/Work/Exports/Project review_.*\\.${extension}$`),
      ),
      extension === "pdf" ? expect.any(Object) : expect.any(String),
    );
    expect(mocks.revealItemInDir).toHaveBeenCalledWith(writer.mock.calls[0][0]);
    expect(mocks.downloadDir).not.toHaveBeenCalled();
    expect(mocks.save).not.toHaveBeenCalled();
  });

  it.each([undefined, ""])(
    "uses Downloads when the preference is %s",
    async (directory) => {
      mocks.settings.mockResolvedValue({
        values: { export_directory: directory },
      });
      renderModal();
      fireEvent.click(screen.getByRole("button", { name: "Export" }));
      await waitFor(() =>
        expect(mocks.exportPdf).toHaveBeenCalledWith(
          expect.stringMatching(/^\/Users\/test\/Downloads\//),
          expect.any(Object),
        ),
      );
    },
  );

  it("exports the shared summary from a shared tab", async () => {
    mocks.sharedNote = {
      body: {
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: "Shared decisions" }],
          },
        ],
      },
    };
    renderModal({ type: "shared", id: "share-1" });
    fireEvent.click(screen.getByRole("radio", { name: "Markdown" }));
    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    await waitFor(() => expect(mocks.onOpenChange).toHaveBeenCalledWith(false));
    expect(mocks.writeTextFile.mock.calls[0][1]).toContain("Shared decisions");
  });

  it("does not export to Downloads if reading the saved preference fails", async () => {
    mocks.settings.mockRejectedValue(new Error("Database unavailable"));
    renderModal();
    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    await screen.findByRole("alert");
    expect(mocks.downloadDir).not.toHaveBeenCalled();
    expect(mocks.exportPdf).not.toHaveBeenCalled();
  });

  it("keeps the modal open and shows write failures", async () => {
    mocks.writeTextFile.mockResolvedValue({
      status: "error",
      error: "Permission denied",
    });
    renderModal();
    fireEvent.click(screen.getByRole("radio", { name: "Markdown" }));
    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    expect((await screen.findByRole("alert")).textContent).toContain(
      "Check the export location in Settings",
    );
    expect(mocks.onOpenChange).not.toHaveBeenCalled();
    expect(mocks.revealItemInDir).not.toHaveBeenCalled();
  });

  it("uses the saved folder as the App Store save dialog default", async () => {
    mocks.isAppStoreBuild.mockReturnValue(true);
    mocks.settings.mockResolvedValue({
      values: { export_directory: "/Users/test/Documents" },
    });
    mocks.save.mockResolvedValue("/Users/test/Documents/review.pdf");
    renderModal();
    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    await waitFor(() =>
      expect(mocks.exportPdf).toHaveBeenCalledWith(
        "/Users/test/Documents/review.pdf",
        expect.any(Object),
      ),
    );
    expect(mocks.save).toHaveBeenCalledWith({
      defaultPath: expect.stringMatching(/^\/Users\/test\/Documents\//),
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
  });

  it("leaves the export modal open when the native save dialog is canceled", async () => {
    mocks.isAppStoreBuild.mockReturnValue(true);
    mocks.save.mockResolvedValue(null);
    renderModal();
    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    await waitFor(() => expect(mocks.save).toHaveBeenCalled());
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Export" }).hasAttribute("disabled"),
      ).toBe(false),
    );
    expect(mocks.exportPdf).not.toHaveBeenCalled();
    expect(mocks.onOpenChange).not.toHaveBeenCalled();
    expect(mocks.revealItemInDir).not.toHaveBeenCalled();
  });

  it("opens with the last used format and sections", () => {
    // settingsValues stays empty: a fresh live-query handle reports empty on
    // its first render, so the form must initialize from the settled query
    // values, not from its own handle read.
    mocks.storedValues = {
      export_format: "md",
      export_include_memo: true,
      export_include_summary: false,
      export_include_transcript: true,
    };
    renderModal();
    expect(
      (screen.getByRole("radio", { name: "Markdown" }) as HTMLInputElement)
        .checked,
    ).toBe(true);
    expect(
      (screen.getByRole("checkbox", { name: "Memo" }) as HTMLInputElement)
        .checked,
    ).toBe(true);
    expect(
      (screen.getByRole("checkbox", { name: "Summary" }) as HTMLInputElement)
        .checked,
    ).toBe(false);
    expect(
      (screen.getByRole("checkbox", { name: "Transcript" }) as HTMLInputElement)
        .checked,
    ).toBe(true);
  });

  it("falls back to PDF summary defaults for unknown stored values", () => {
    mocks.storedValues = { export_format: "docx" };
    renderModal();
    expect(
      (screen.getByRole("radio", { name: "PDF" }) as HTMLInputElement).checked,
    ).toBe(true);
    expect(
      (screen.getByRole("checkbox", { name: "Memo" }) as HTMLInputElement)
        .checked,
    ).toBe(false);
    expect(
      (screen.getByRole("checkbox", { name: "Summary" }) as HTMLInputElement)
        .checked,
    ).toBe(true);
    expect(
      (screen.getByRole("checkbox", { name: "Transcript" }) as HTMLInputElement)
        .checked,
    ).toBe(false);
  });

  it("persists the used config on successful export", async () => {
    renderModal();
    fireEvent.click(screen.getByRole("radio", { name: "TXT" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Memo" }));
    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    await waitFor(() => expect(mocks.onOpenChange).toHaveBeenCalledWith(false));
    expect(mocks.persistSettings).toHaveBeenCalledWith({
      export_format: "txt",
      export_include_memo: true,
      export_include_summary: true,
      export_include_transcript: false,
    });
  });

  it("persists the submitted options when selections change during export", async () => {
    let resolveWrite: (value: unknown) => void = () => {};
    mocks.writeTextFile.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveWrite = resolve;
        }),
    );
    renderModal();
    fireEvent.click(screen.getByRole("radio", { name: "TXT" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Memo" }));
    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    await waitFor(() => expect(mocks.writeTextFile).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("radio", { name: "PDF" }));
    resolveWrite({ status: "ok", data: null });
    await waitFor(() => expect(mocks.onOpenChange).toHaveBeenCalledWith(false));
    expect(mocks.persistSettings).toHaveBeenCalledWith({
      export_format: "txt",
      export_include_memo: true,
      export_include_summary: true,
      export_include_transcript: false,
    });
  });

  it("builds the file from the submitted sections when selections change", async () => {
    mocks.sharedNote = {
      body: {
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: "Shared decisions" }],
          },
        ],
      },
    };
    renderModal({ type: "shared", id: "share-1" });
    fireEvent.click(screen.getByRole("radio", { name: "TXT" }));
    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Summary" }));
    await waitFor(() => expect(mocks.onOpenChange).toHaveBeenCalledWith(false));
    expect(mocks.writeTextFile.mock.calls[0][1]).toContain("Shared decisions");
    expect(mocks.persistSettings).toHaveBeenCalledWith({
      export_format: "txt",
      export_include_memo: false,
      export_include_summary: true,
      export_include_transcript: false,
    });
  });

  it("restores the last used config when reopened after export", async () => {
    mocks.persistSettings.mockImplementation((values: unknown) => {
      Object.assign(mocks.storedValues, values);
    });
    const first = renderModal();
    fireEvent.click(screen.getByRole("radio", { name: "TXT" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Memo" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Summary" }));
    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    await waitFor(() => expect(mocks.onOpenChange).toHaveBeenCalledWith(false));
    first.unmount();
    renderModal();
    expect(
      (screen.getByRole("radio", { name: "TXT" }) as HTMLInputElement).checked,
    ).toBe(true);
    expect(
      (screen.getByRole("checkbox", { name: "Memo" }) as HTMLInputElement)
        .checked,
    ).toBe(true);
    expect(
      (screen.getByRole("checkbox", { name: "Summary" }) as HTMLInputElement)
        .checked,
    ).toBe(false);
  });

  it("does not persist the config when the export fails", async () => {
    mocks.writeTextFile.mockResolvedValue({
      status: "error",
      error: "Permission denied",
    });
    renderModal();
    fireEvent.click(screen.getByRole("radio", { name: "Markdown" }));
    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    await screen.findByRole("alert");
    expect(mocks.persistSettings).not.toHaveBeenCalled();
  });

  it("waits for settings to load before initializing the form", () => {
    mocks.settingsLoading = true;
    mocks.storedValues = {
      export_format: "md",
      export_include_memo: true,
      export_include_summary: false,
      export_include_transcript: true,
    };
    const view = renderModal();
    expect(screen.queryByRole("button", { name: "Export" })).toBeNull();
    mocks.settingsLoading = false;
    view.reload();
    expect(
      (screen.getByRole("radio", { name: "Markdown" }) as HTMLInputElement)
        .checked,
    ).toBe(true);
    expect(
      (screen.getByRole("checkbox", { name: "Memo" }) as HTMLInputElement)
        .checked,
    ).toBe(true);
  });
});
