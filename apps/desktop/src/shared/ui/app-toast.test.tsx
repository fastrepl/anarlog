/** @vitest-environment jsdom */

import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { hydrateRoot } from "react-dom/client";
import { renderToString } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  type AppToastHandle,
  AppToaster,
  dismissAppToasts,
  showAppToast,
  showErrorToast,
  showSuccessToast,
} from "@anlg/ui/components/ui/app-toast";

afterEach(() => {
  dismissAppToasts();
  cleanup();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("app toast", () => {
  it("hydrates the server markup before mounting the portal", async () => {
    const container = document.createElement("div");
    container.innerHTML = renderToString(<AppToaster />);
    expect(container.innerHTML).toBe("");
    document.body.append(container);
    const onRecoverableError = vi.fn();
    let root: ReturnType<typeof hydrateRoot>;
    await act(async () => {
      root = hydrateRoot(container, <AppToaster />, { onRecoverableError });
    });
    expect(onRecoverableError).not.toHaveBeenCalled();
    expect(document.querySelector("[data-app-toaster]")).not.toBeNull();
    act(() => root.unmount());
    container.remove();
  });

  it("renders independent custom toasts with the shared app visuals", async () => {
    vi.useFakeTimers();
    const onAction = vi.fn();
    render(<AppToaster theme="light" />);

    act(() => {
      showAppToast({
        message: "First notification",
        action: { label: "Open", onClick: onAction },
      });
      showErrorToast("Second notification");
    });
    await act(() => vi.advanceTimersByTimeAsync(0));

    const toasts = document.querySelectorAll("[data-app-toast]");
    expect(toasts).toHaveLength(2);
    expect(
      document.querySelector('[data-app-toast][data-tone="info"]')?.textContent,
    ).toContain("First notification");
    expect(screen.getByRole("alert").textContent).toContain(
      "Second notification",
    );
    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    expect(onAction).toHaveBeenCalledOnce();
  });

  it("morphs a live toast in place through its handle", () => {
    const onCancel = vi.fn();
    render(<AppToaster theme="light" />);

    let handle: AppToastHandle | undefined;
    act(() => {
      handle = showAppToast({
        message: "Downloading update",
        description: "1%",
        durationMs: Number.POSITIVE_INFINITY,
      });
    });
    expect(screen.getByText("1%")).toBeTruthy();

    act(() => {
      handle?.update({
        description: "42%",
        action: { label: "Cancel", onClick: onCancel },
      });
    });
    expect(document.querySelectorAll("[data-app-toast]")).toHaveLength(1);
    expect(screen.queryByText("1%")).toBeNull();
    expect(screen.getByText("42%")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalledOnce();

    // The action dismissed it; a late update must not resurrect the content.
    act(() => {
      handle?.update({ description: "99%" });
    });
    expect(screen.queryByText("99%")).toBeNull();
  });

  it("keeps the toast up when its action opts out of dismissing", () => {
    const onDownload = vi.fn();
    const onCancel = vi.fn();
    render(<AppToaster theme="light" />);

    let handle: AppToastHandle | undefined;
    act(() => {
      handle = showAppToast({
        message: "Update available",
        durationMs: Number.POSITIVE_INFINITY,
        action: {
          label: "Download",
          onClick: onDownload,
          dismissOnClick: false,
        },
      });
    });

    fireEvent.click(screen.getByRole("button", { name: "Download" }));
    expect(onDownload).toHaveBeenCalledOnce();
    expect(screen.getByRole("status").getAttribute("data-phase")).toBe(
      "visible",
    );

    // The caller morphs it into the next step; a default action still dismisses.
    act(() => {
      handle?.update({
        message: "Downloading update",
        action: { label: "Cancel", onClick: onCancel },
      });
    });
    expect(screen.getByText("Downloading update")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalledOnce();
    expect(screen.getByRole("status").getAttribute("data-phase")).toBe(
      "exiting",
    );
  });

  it("reports once when a toast leaves, however it was dismissed", async () => {
    vi.useFakeTimers();
    const onDismiss = vi.fn();
    const onTimedOut = vi.fn();
    const onCleared = vi.fn();
    render(<AppToaster theme="light" />);

    let handle: AppToastHandle | undefined;
    act(() => {
      handle = showAppToast({
        message: "Sticky",
        durationMs: Number.POSITIVE_INFINITY,
        onDismiss,
      });
      showAppToast({
        message: "Timed",
        durationMs: 1_000,
        onDismiss: onTimedOut,
      });
    });
    expect(onDismiss).not.toHaveBeenCalled();

    // The user closes it, then code dismisses the same toast again: reported once.
    fireEvent.click(screen.getAllByRole("button", { name: "Dismiss" })[0]);
    expect(onDismiss).toHaveBeenCalledOnce();
    act(() => handle?.dismiss());
    expect(onDismiss).toHaveBeenCalledOnce();

    await act(() => vi.advanceTimersByTimeAsync(1_000));
    expect(onTimedOut).toHaveBeenCalledOnce();

    act(() => {
      showAppToast({
        message: "Cleared",
        durationMs: Number.POSITIVE_INFINITY,
        onDismiss: onCleared,
      });
    });
    act(() => dismissAppToasts());
    expect(onCleared).toHaveBeenCalledOnce();
  });

  it("dismisses when the toast body is clicked", async () => {
    vi.useFakeTimers();
    render(<AppToaster theme="light" />);

    act(() => {
      showSuccessToast("Changes saved");
    });

    fireEvent.click(screen.getByText("Changes saved"));
    expect(screen.getByRole("status").getAttribute("data-phase")).toBe(
      "exiting",
    );
    await act(() => vi.advanceTimersByTimeAsync(250));
    expect(screen.queryByText("Changes saved")).toBeNull();
  });

  it("pauses and resumes auto-dismiss from the expanded footer", async () => {
    vi.useFakeTimers();
    render(<AppToaster theme="light" />);

    act(() => {
      showAppToast({ message: "Sync complete", durationMs: 1_000 });
    });

    fireEvent.click(screen.getByRole("button", { name: "Pause auto-dismiss" }));
    await act(() => vi.advanceTimersByTimeAsync(2_000));
    expect(screen.getByText("Sync complete")).toBeTruthy();

    fireEvent.click(
      screen.getByRole("button", { name: "Resume auto-dismiss" }),
    );
    await act(() => vi.advanceTimersByTimeAsync(1_300));
    expect(screen.queryByText("Sync complete")).toBeNull();
  });

  it.each([
    { name: "default", durationMs: undefined, before: 2_900, after: 500 },
    { name: "durationMs", durationMs: 1_000, before: 0, after: 1_500 },
  ])(
    "auto-dismisses after the $name duration",
    async ({ durationMs, before, after }) => {
      vi.useFakeTimers();
      render(<AppToaster theme="light" />);

      act(() => {
        showAppToast({ message: "Temporary notice", durationMs });
      });
      await act(() => vi.advanceTimersByTimeAsync(0));
      await act(() => vi.advanceTimersByTimeAsync(before));

      expect(document.querySelector("[data-app-toast]")?.textContent).toContain(
        "Temporary notice",
      );

      await act(() => vi.advanceTimersByTimeAsync(after));

      expect(document.querySelector("[data-app-toast]")).toBeNull();
    },
  );
});
