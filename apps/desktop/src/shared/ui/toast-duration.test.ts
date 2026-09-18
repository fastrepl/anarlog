import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { createElement } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { toast, Toaster, TOAST_DURATIONS } from "@anlg/ui/components/ui/toast";

beforeEach(() => {
  vi.useFakeTimers();
  render(createElement(Toaster));
});
afterEach(() => {
  act(() => toast.dismiss());
  cleanup();
  vi.useRealTimers();
});

function advance(ms: number) {
  return act(() => vi.advanceTimersByTimeAsync(ms));
}

describe("toast migration", () => {
  it.each(["success", "info", "warning", "error"] as const)(
    "expires %s after its default duration",
    async (tone) => {
      act(() => {
        toast[tone]("Notice");
      });
      await advance(TOAST_DURATIONS[tone] - 1);
      expect(screen.getByText("Notice")).toBeTruthy();
      await advance(251);
      expect(screen.queryByText("Notice")).toBeNull();
    },
  );

  it("keeps explicit persistent errors, warnings and loading notices until dismissed", async () => {
    act(() => {
      toast.error("Persistent error", { id: "error", duration: Infinity });
      toast.warning("Warning", { id: "warning", duration: Infinity });
      toast.loading("Working", { id: "loading" });
    });
    await advance(60_000);
    expect(screen.getByText("Warning")).toBeTruthy();
    expect(screen.getByText("Working")).toBeTruthy();
    expect(screen.getByText("Persistent error")).toBeTruthy();
    act(() => toast.dismiss("loading"));
    await advance(250);
    expect(screen.queryByText("Working")).toBeNull();
    expect(screen.getByText("Warning")).toBeTruthy();
  });

  it.each([12_000, 2_000])(
    "caps errors while respecting shorter durations (%s)",
    async (duration) => {
      act(() => {
        toast.error("Error", { duration });
      });
      await advance(Math.min(duration, TOAST_DURATIONS.error) + 250);
      expect(screen.queryByText("Error")).toBeNull();
    },
  );

  it("replaces a toast by ID and cancels its old expiration", async () => {
    act(() => {
      toast.info("Starting", { id: "download", duration: 500 });
    });
    await advance(250);
    act(() => {
      toast.loading("Downloading", { id: "download" });
    });
    await advance(500);
    expect(screen.queryByText("Starting")).toBeNull();
    expect(document.querySelectorAll("[data-app-toast]")).toHaveLength(1);
    expect(screen.getByText("Downloading")).toBeTruthy();
    act(() => toast.dismiss("download"));
    act(() => {
      toast.success("Done", { id: "download" });
    });
    await advance(250);
    expect(screen.getByText("Done")).toBeTruthy();
  });

  it("distinguishes user dismissal from timeout and cleanup", async () => {
    const onDismiss = vi.fn();
    act(() => {
      toast.info("Timed", { duration: 100, onDismiss });
    });
    await advance(350);
    act(() => {
      toast.info("Cleanup", { id: "cleanup", onDismiss });
    });
    act(() => toast.dismiss("cleanup"));
    await advance(250);
    expect(onDismiss).not.toHaveBeenCalled();
    act(() => {
      toast.info("Dismiss me", { onDismiss });
    });
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it("keeps condition-bound notices open and respects action preventDefault", () => {
    const onClick = vi.fn((event) => event.preventDefault());
    act(() => {
      toast.info("Settings needed", {
        dismissible: false,
        closeButton: false,
        duration: Infinity,
        action: { label: "Configure", onClick },
      });
    });
    expect(screen.queryByRole("button", { name: "Dismiss" })).toBeNull();
    fireEvent.click(screen.getByText("Settings needed"));
    fireEvent.click(screen.getByRole("button", { name: "Configure" }));
    expect(onClick).toHaveBeenCalledOnce();
    expect(screen.getByRole("status").getAttribute("data-phase")).toBe(
      "visible",
    );
  });
});
