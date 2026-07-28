import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  checkPermission: vi.fn(),
  openPermission: vi.fn(),
  requestPermission: vi.fn(),
  resetPermission: vi.fn(),
}));

vi.mock("@hypr/plugin-permissions", () => ({
  commands: mocks,
}));

import { usePermission } from "./usePermissions";

describe("usePermission", () => {
  let queryClient: QueryClient;

  beforeEach(() => {
    vi.clearAllMocks();
    queryClient = new QueryClient({
      defaultOptions: {
        queries: { retry: false },
        mutations: { retry: false },
      },
    });
    mocks.checkPermission.mockResolvedValue({
      status: "ok",
      data: "denied",
    });
  });

  afterEach(() => {
    cleanup();
    queryClient.clear();
  });

  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );

  it("keeps a failed runtime probe denied and exposes its diagnostic", async () => {
    mocks.requestPermission.mockResolvedValue({
      status: "error",
      error: "PipeWire source unavailable",
    });

    const { result } = renderHook(() => usePermission("systemAudio"), {
      wrapper,
    });

    await waitFor(() => expect(result.current.status).toBe("denied"));

    act(() => result.current.request());

    await waitFor(() =>
      expect(result.current.error).toBe("PipeWire source unavailable"),
    );
    expect(result.current.status).toBe("denied");
  });

  it("lets a later failing probe override an optimistic grant", async () => {
    mocks.requestPermission.mockResolvedValue({ status: "ok", data: null });

    const { result } = renderHook(() => usePermission("systemAudio"), {
      wrapper,
    });

    await waitFor(() => expect(result.current.status).toBe("denied"));

    act(() => result.current.request());

    await waitFor(() => expect(result.current.status).toBe("authorized"));
    expect(result.current.confirmedStatus).toBe("denied");

    await waitFor(() => expect(result.current.status).toBe("denied"), {
      timeout: 5000,
    });
  });

  it("drops a stale request error once the probe recovers", async () => {
    mocks.requestPermission.mockResolvedValue({
      status: "error",
      error: "PipeWire source unavailable",
    });

    const { result } = renderHook(() => usePermission("microphone"), {
      wrapper,
    });

    await waitFor(() => expect(result.current.status).toBe("denied"));

    act(() => result.current.request());

    await waitFor(() =>
      expect(result.current.error).toBe("PipeWire source unavailable"),
    );

    mocks.checkPermission.mockResolvedValue({
      status: "ok",
      data: "authorized",
    });

    await waitFor(() => expect(result.current.status).toBe("authorized"), {
      timeout: 5000,
    });
    expect(result.current.error).toBeNull();
  });
});
