import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  loadPromptOverride,
  usePromptOverride,
  usePromptOverrides,
  usePromptTemplateSource,
} from "./data";

const { executeMock, executeProxyMock, subscribeMock, getTemplateSourceMock } =
  vi.hoisted(() => ({
    executeMock: vi.fn(),
    executeProxyMock: vi.fn(),
    subscribeMock: vi.fn(),
    getTemplateSourceMock: vi.fn(),
  }));

vi.mock("@hypr/plugin-db", () => ({
  execute: executeMock,
  executeProxy: executeProxyMock,
  subscribe: subscribeMock,
}));

vi.mock("@hypr/plugin-template", () => ({
  commands: {
    getTemplateSource: getTemplateSourceMock,
  },
}));

type SubscribeOptions<T> = {
  onData: (rows: T[]) => void;
  onError?: (message: string) => void;
};

function createWrapper() {
  const client = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  });

  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
  };
}

describe("prompt data", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    executeProxyMock.mockResolvedValue({ rows: [] });
  });

  it("loads a stored prompt override from SQLite", async () => {
    executeProxyMock.mockResolvedValue({
      rows: [
        [
          "enhance",
          "# Context",
          "2026-04-13T00:00:00Z",
          "2026-04-13T00:00:00Z",
        ],
      ],
    });

    await expect(loadPromptOverride("enhance")).resolves.toBe("# Context");
    expect(executeProxyMock).toHaveBeenCalledWith(
      expect.stringContaining("prompt_overrides"),
      expect.arrayContaining(["enhance"]),
      "all",
    );
  });

  it("returns null when no override exists", async () => {
    executeProxyMock.mockResolvedValue({ rows: [] });

    await expect(loadPromptOverride("title")).resolves.toBeNull();
  });

  it("maps a live prompt override row", async () => {
    let subscription: SubscribeOptions<any> | undefined;

    subscribeMock.mockImplementation(async (_sql, _params, options) => {
      subscription = options;
      return vi.fn();
    });

    const { result } = renderHook(() => usePromptOverride("enhance"));

    await waitFor(() => {
      expect(subscribeMock).toHaveBeenCalledTimes(1);
    });

    act(() => {
      subscription?.onData([
        {
          task_type: "enhance",
          content: "# Context",
          created_at: "2026-04-13T00:00:00Z",
          updated_at: "2026-04-13T00:00:00Z",
        },
      ]);
    });

    await waitFor(() => {
      expect(result.current.data?.content).toBe("# Context");
      expect(result.current.isLoading).toBe(false);
    });
  });

  it("maps live prompt overrides by task type", async () => {
    let subscription: SubscribeOptions<any> | undefined;

    subscribeMock.mockImplementation(async (_sql, _params, options) => {
      subscription = options;
      return vi.fn();
    });

    const { result } = renderHook(() => usePromptOverrides());

    await waitFor(() => {
      expect(subscribeMock).toHaveBeenCalledTimes(1);
    });

    act(() => {
      subscription?.onData([
        {
          task_type: "enhance",
          content: "# Context",
          created_at: "2026-04-13T00:00:00Z",
          updated_at: "2026-04-13T00:00:00Z",
        },
        {
          task_type: "title",
          content: "<note></note>",
          created_at: "2026-04-13T00:00:00Z",
          updated_at: "2026-04-13T00:00:00Z",
        },
      ]);
    });

    await waitFor(() => {
      expect(result.current.data?.enhance?.content).toBe("# Context");
      expect(result.current.data?.title?.content).toBe("<note></note>");
    });
  });

  it("loads the raw built-in template source from the template plugin", async () => {
    getTemplateSourceMock.mockResolvedValue({
      status: "ok",
      data: "{% raw %}",
    });

    const { result } = renderHook(() => usePromptTemplateSource("enhance"), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.data).toBe("{% raw %}");
    });

    expect(getTemplateSourceMock).toHaveBeenCalledWith("enhanceUser");
  });
});
