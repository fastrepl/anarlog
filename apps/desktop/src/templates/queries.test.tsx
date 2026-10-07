import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { executeProxy, subscribe } from "@anlg/plugin-db";

import {
  getTemplateById,
  useCreateTemplate,
  useToggleTemplateFavorite,
  useUserTemplate,
  useUserTemplates,
} from "./queries";
import { DEFAULT_TEMPLATE_ICON } from "./template-icon";

type SubscribeOptions<T> = {
  onData: (rows: T[]) => void;
  onError?: (message: string) => void;
};

describe("template queries", () => {
  const executeProxyMock = vi.mocked(executeProxy);
  const subscribeMock = vi.mocked(subscribe);

  function createWrapper() {
    const queryClient = new QueryClient({
      defaultOptions: {
        mutations: { retry: false },
        queries: { retry: false },
      },
    });

    return function Wrapper({ children }: { children: ReactNode }) {
      return (
        <QueryClientProvider client={queryClient}>
          {children}
        </QueryClientProvider>
      );
    };
  }

  afterEach(cleanup);

  beforeEach(() => {
    vi.clearAllMocks();
    executeProxyMock.mockResolvedValue({ rows: [] });
    subscribeMock.mockResolvedValue(async () => {});
  });

  it("maps live query rows that use raw SQLite field names", async () => {
    const subscriptions: Array<SubscribeOptions<Record<string, unknown>>> = [];

    subscribeMock.mockImplementation(async (_sql, _params, options) => {
      subscriptions.push(options);
      return async () => {};
    });

    const { result: templatesResult } = renderHook(() => useUserTemplates());
    const { result: templateResult } = renderHook(() =>
      useUserTemplate("template-1"),
    );

    await waitFor(() => {
      expect(subscribeMock).toHaveBeenCalledTimes(2);
    });

    act(() => {
      const rows = [
        {
          id: "template-1",
          title: "Standup",
          description: "Daily sync",
          pinned: true,
          pin_order: 2,
          category: "meetings",
          icon_json: '{"type":"emoji","value":"☀️"}',
          targets_json: '["engineering"]',
          sections_json: '[{"title":"Notes","description":"Capture updates"}]',
          created_at: "2026-04-14T00:00:00Z",
          updated_at: "2026-04-14T00:00:00Z",
        },
      ];

      subscriptions[0]?.onData(rows);
      subscriptions[1]?.onData(rows);
    });

    await waitFor(() => {
      expect(templatesResult.current).toEqual([
        {
          id: "template-1",
          title: "Standup",
          description: "Daily sync",
          pinned: true,
          pinOrder: 2,
          category: "meetings",
          icon: { type: "emoji", value: "☀️" },
          targets: ["engineering"],
          sections: [{ title: "Notes", description: "Capture updates" }],
          updatedAt: "2026-04-14T00:00:00Z",
        },
      ]);
      expect(templateResult.current.data).toEqual({
        id: "template-1",
        title: "Standup",
        description: "Daily sync",
        pinned: true,
        pinOrder: 2,
        category: "meetings",
        icon: { type: "emoji", value: "☀️" },
        targets: ["engineering"],
        sections: [{ title: "Notes", description: "Capture updates" }],
        updatedAt: "2026-04-14T00:00:00Z",
      });
    });
  });

  it("keeps live template rows visible when stored template JSON is invalid", async () => {
    const subscriptions: Array<SubscribeOptions<Record<string, unknown>>> = [];

    subscribeMock.mockImplementation(async (_sql, _params, options) => {
      subscriptions.push(options);
      return async () => {};
    });

    const { result } = renderHook(() => useUserTemplates());

    await waitFor(() => {
      expect(subscribeMock).toHaveBeenCalledTimes(1);
    });

    act(() => {
      subscriptions[0]?.onData([
        {
          id: "template-1",
          title: "Draft Template",
          description: "",
          pinned: false,
          pin_order: null,
          category: null,
          icon_json: "{",
          targets_json: "{",
          sections_json: '[{"title":"","description":""}]',
          created_at: "2026-04-14T00:00:00Z",
          updated_at: "2026-04-14T00:00:00Z",
        },
      ]);
    });

    await waitFor(() => {
      expect(result.current).toEqual([
        {
          id: "template-1",
          title: "Draft Template",
          description: "",
          pinned: false,
          pinOrder: undefined,
          category: undefined,
          icon: DEFAULT_TEMPLATE_ICON,
          targets: undefined,
          sections: [{ title: "", description: "" }],
          updatedAt: "2026-04-14T00:00:00Z",
        },
      ]);
    });
  });

  it("keeps execute-path reads working with Drizzle-mapped rows", async () => {
    executeProxyMock.mockResolvedValue({
      rows: [
        [
          "template-1",
          "Standup",
          "Daily sync",
          0,
          null,
          null,
          '{"type":"icon","value":"target","color":"#5b67d8"}',
          '["engineering"]',
          '[{"title":"Notes","description":"Capture updates"}]',
          "2026-04-14T00:00:00Z",
          "2026-04-14T00:00:00Z",
        ],
      ],
    });

    await expect(getTemplateById("template-1")).resolves.toEqual({
      id: "template-1",
      title: "Standup",
      description: "Daily sync",
      pinned: false,
      pinOrder: undefined,
      category: undefined,
      icon: { type: "icon", value: "target", color: "#5b67d8" },
      targets: ["engineering"],
      sections: [{ title: "Notes", description: "Capture updates" }],
      updatedAt: "2026-04-14T00:00:00Z",
    });
  });

  it("keeps the template timestamp when toggling its favorite flag", async () => {
    executeProxyMock
      .mockResolvedValueOnce({
        rows: [
          [
            "template-1",
            "Standup",
            "Daily sync",
            0,
            null,
            null,
            '{"type":"icon","value":"target","color":"#5b67d8"}',
            '["engineering"]',
            '[{"title":"Notes","description":"Capture updates"}]',
            "2026-04-14T00:00:00Z",
            "2026-04-14T00:00:00Z",
          ],
        ],
      })
      .mockResolvedValueOnce({ rows: [[2]] })
      .mockResolvedValueOnce({ rows: [] });

    const { result } = renderHook(() => useToggleTemplateFavorite(), {
      wrapper: createWrapper(),
    });

    await act(async () => {
      await result.current("template-1");
    });

    const updateCall = executeProxyMock.mock.calls.find(([sql]) =>
      String(sql).toLowerCase().startsWith("update"),
    );
    expect(updateCall?.[0]).toContain('"pinned"');
    expect(updateCall?.[0]).not.toContain("updated_at");
  });

  it("creates a template row through the SQLite proxy", async () => {
    const { result } = renderHook(() => useCreateTemplate(), {
      wrapper: createWrapper(),
    });

    let createdId: string | undefined;
    await act(async () => {
      createdId = await result.current({
        title: "New Template",
        description: "",
        sections: [],
      });
    });

    expect(createdId).toEqual(expect.any(String));
    expect(executeProxyMock).toHaveBeenCalledWith(
      expect.stringContaining('insert into "templates"'),
      [
        createdId,
        "New Template",
        "",
        0,
        JSON.stringify(DEFAULT_TEMPLATE_ICON),
        null,
        "[]",
      ],
      "run",
    );
  });
});
