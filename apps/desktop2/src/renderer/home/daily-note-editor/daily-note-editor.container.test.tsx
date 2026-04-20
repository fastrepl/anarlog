import { QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { queryClient } from "~/query-client";

// Stub the tiptap-backed editor with a plain textarea so the container's
// subscribe/debounce/mutation wiring can be driven from DOM events. The
// real editor is exercised by its own package tests.
vi.mock("@hypr/editor/daily", () => ({
  DailyNoteEditor: ({
    value,
    onChange,
    placeholder,
  }: {
    value: string;
    onChange: (next: string) => void;
    placeholder?: string;
  }) => (
    <textarea
      value={value}
      placeholder={placeholder}
      onChange={(event) => onChange(event.target.value)}
    />
  ),
}));

import { DailyNoteEditorContainer } from "~/home/daily-note-editor/daily-note-editor.container";

describe("DailyNoteEditorContainer", () => {
  beforeEach(() => {
    queryClient.clear();
    vi.mocked(window.hypr.db.subscribe).mockImplementation(
      async (_sql, _params, options) => {
        options.onData([
          {
            date: "2026-04-20",
            content: "loaded note",
            created_at: "2026-04-20T00:00:00Z",
            updated_at: "2026-04-20T00:00:00Z",
          },
        ]);
        return async () => {};
      },
    );
    vi.mocked(window.hypr.db.executeProxy).mockResolvedValue({ rows: [] });
  });

  it("loads the note and debounces autosave", async () => {
    render(
      <QueryClientProvider client={queryClient}>
        <DailyNoteEditorContainer date="2026-04-20" />
      </QueryClientProvider>,
    );

    const editor = screen.getByPlaceholderText(
      "Write today's notes...",
    ) as HTMLTextAreaElement;

    await waitFor(() => {
      expect(editor.value).toBe("loaded note");
    });

    fireEvent.change(editor, { target: { value: "updated note" } });
    await new Promise((resolve) => window.setTimeout(resolve, 350));

    await waitFor(() => {
      expect(window.hypr.db.executeProxy).toHaveBeenCalled();
    });

    const proxyCalls = vi.mocked(window.hypr.db.executeProxy).mock.calls;
    const upsertCall = proxyCalls.find(([sql]) =>
      sql.toLowerCase().includes("insert"),
    );
    expect(upsertCall).toBeDefined();
    expect(upsertCall?.[1]).toEqual(
      expect.arrayContaining(["2026-04-20", "updated note"]),
    );
  });
});
