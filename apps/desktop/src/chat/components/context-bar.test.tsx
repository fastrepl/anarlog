import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { openNewMock, searchMock, shellState, storeState } = vi.hoisted(() => ({
  openNewMock: vi.fn(),
  searchMock: vi.fn(),
  shellState: {
    mode: "FloatingOpen" as
      | "FloatingClosed"
      | "FloatingOpen"
      | "RightPanelOpen",
  },
  storeState: {
    rows: {} as Record<string, Record<string, unknown>>,
    sessionIds: [] as string[],
  },
}));

vi.mock("@hypr/ui/components/ui/popover", () => ({
  AppFloatingPanel: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
  Popover: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  PopoverContent: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
  PopoverTrigger: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock("@hypr/ui/components/ui/tooltip", () => ({
  Tooltip: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  TooltipContent: ({ children }: { children: ReactNode }) => (
    <div>{children}</div>
  ),
  TooltipTrigger: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock("~/contexts/shell", () => ({
  useShell: () => ({
    chat: {
      mode: shellState.mode,
    },
  }),
}));

vi.mock("~/search/contexts/engine", () => ({
  useSearchEngine: () => ({
    search: searchMock,
  }),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  UI: {
    useRowIds: () => storeState.sessionIds,
    useStore: () => ({
      getRow: (_table: string, rowId: string) => storeState.rows[rowId] ?? {},
      hasRow: (_table: string, rowId: string) => rowId in storeState.rows,
    }),
  },
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: <T,>(selector: (state: { openNew: typeof openNewMock }) => T) =>
    selector({ openNew: openNewMock }),
}));

import { ContextBar } from "./context-bar";

function renderContextBar() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <ContextBar
        entities={[
          {
            kind: "session",
            key: "session:auto:current",
            source: "auto-current",
            sessionId: "current",
            title: "Current Note",
            date: "2026-04-01T00:00:00.000Z",
            pending: false,
          },
        ]}
        onAddEntity={vi.fn()}
      />
    </QueryClientProvider>,
  );
}

describe("ContextBar session picker", () => {
  beforeEach(() => {
    cleanup();
    openNewMock.mockClear();
    searchMock.mockReset();
    shellState.mode = "FloatingOpen";
    storeState.rows = {};
    storeState.sessionIds = [];
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as typeof ResizeObserver;
  });

  it("shows recent sessions from TinyBase and skips malformed blank rows", () => {
    storeState.sessionIds = ["blank", "old", "new"];
    storeState.rows = {
      blank: {
        title: "",
        created_at: "",
      },
      old: {
        title: "Old Note",
        created_at: "2024-01-01T00:00:00.000Z",
      },
      new: {
        title: "New Note",
        created_at: "2026-04-14T00:00:00.000Z",
      },
    };

    renderContextBar();

    const newNote = screen.getByText("New Note");
    const oldNote = screen.getByText("Old Note");

    expect(
      newNote.compareDocumentPosition(oldNote) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(screen.queryByText("Untitled")).toBeNull();
    expect(screen.queryByText(/1970/)).toBeNull();
    expect(searchMock).not.toHaveBeenCalled();
  });

  it("hydrates search hits from TinyBase before rendering", async () => {
    storeState.sessionIds = ["session-1"];
    storeState.rows = {
      "session-1": {
        title: "Hydrated Note",
        created_at: "2026-02-02T00:00:00.000Z",
      },
    };
    searchMock.mockResolvedValue([
      {
        score: 1,
        document: {
          id: "session-1",
          type: "session",
          title: "",
          content: "",
          created_at: 0,
        },
      },
    ]);

    renderContextBar();

    fireEvent.change(screen.getByPlaceholderText("Search sessions..."), {
      target: { value: "hydrated" },
    });

    await waitFor(() => {
      expect(searchMock).toHaveBeenCalledWith("hydrated", {
        created_at: undefined,
      });
    });

    expect(await screen.findByText("Hydrated Note")).toBeTruthy();
    expect(screen.queryByText(/1970/)).toBeNull();
  });

  it("uses balanced context bar horizontal margin in the right panel", () => {
    shellState.mode = "RightPanelOpen";

    renderContextBar();

    const outer = document.querySelector("[data-chat-context-bar]");

    expect(outer?.className).toContain("mx-3");
    expect(outer?.className).not.toContain("mx-5");
    expect(outer?.className).not.toContain("mx-2");
    expect(outer?.className).not.toContain("mr-0");
  });
});
