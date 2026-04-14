import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { executeProxy, subscribe } from "@hypr/plugin-db";

import { getTemplateById, useUserTemplate, useUserTemplates } from "./queries";

type SubscribeOptions<T> = {
  onData: (rows: T[]) => void;
  onError?: (message: string) => void;
};

describe("template queries", () => {
  const executeProxyMock = vi.mocked(executeProxy);
  const subscribeMock = vi.mocked(subscribe);

  beforeEach(() => {
    vi.clearAllMocks();
    executeProxyMock.mockResolvedValue({ rows: [] });
    subscribeMock.mockResolvedValue(() => {});
  });

  it("maps live query rows that use raw SQLite field names", async () => {
    const subscriptions: Array<SubscribeOptions<Record<string, unknown>>> = [];

    subscribeMock.mockImplementation(async (_sql, _params, options) => {
      subscriptions.push(options);
      return () => {};
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
          pinned: 1,
          pin_order: 2,
          category: "meetings",
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
          targets: ["engineering"],
          sections: [{ title: "Notes", description: "Capture updates" }],
        },
      ]);
      expect(templateResult.current.data).toEqual({
        id: "template-1",
        title: "Standup",
        description: "Daily sync",
        pinned: true,
        pinOrder: 2,
        category: "meetings",
        targets: ["engineering"],
        sections: [{ title: "Notes", description: "Capture updates" }],
      });
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
      targets: ["engineering"],
      sections: [{ title: "Notes", description: "Capture updates" }],
    });
  });
});
