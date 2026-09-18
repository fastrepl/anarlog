/** @vitest-environment jsdom */

import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
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

function setHoverCapability(enabled: boolean): void {
  vi.stubGlobal(
    "matchMedia",
    vi.fn((query: string) => ({
      matches: query === "(hover: hover) and (pointer: fine)" ? enabled : false,
      media: query,
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  );
}

describe("app toast", () => {
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
    expect(
      screen.getByRole("alert").querySelector('[data-slot="smooth-corners"]'),
    ).not.toBeNull();
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

  it("pins compact toasts to the bottom-right with header-sized icons", () => {
    render(<AppToaster theme="light" align="end" size="compact" />);

    act(() => {
      showAppToast({
        message: "Update ready",
        description: "Restart to finish installing.",
        leading: <svg data-testid="leading-icon" />,
        durationMs: Number.POSITIVE_INFINITY,
      });
    });

    const toaster = document.querySelector("[data-app-toaster]");
    expect(toaster?.getAttribute("data-x-position")).toBe("end");
    expect(toaster?.className).toContain("justify-end");

    const toast = screen.getByRole("status");
    expect(toast.getAttribute("data-size")).toBe("compact");
    // The plus button in the header is a size-8 button with a size-4 glyph.
    expect(
      toast.querySelector('[data-slot="app-toast-leading"]')?.className,
    ).toContain("[&>svg]:size-4");
    expect(screen.getByRole("button", { name: "Dismiss" }).className).toContain(
      "size-8",
    );
  });

  it("mounts a bottom-center viewport at the default size", () => {
    render(<AppToaster theme="light" />);

    const toaster = document.querySelector("[data-app-toaster]");
    expect(toaster?.getAttribute("data-x-position")).toBe("center");
    expect(toaster?.getAttribute("data-y-position")).toBe("bottom");
    expect(toaster?.classList.contains("fixed")).toBe(true);

    act(() => {
      showAppToast({
        message: "Default size",
        durationMs: Number.POSITIVE_INFINITY,
      });
    });
    const toast = screen.getByRole("status");
    expect(toast.getAttribute("data-size")).toBe("default");
    expect(
      toast.querySelector('[data-slot="app-toast-leading"]')?.className,
    ).toContain("[&>svg]:size-6");
    expect(screen.getByRole("button", { name: "Dismiss" }).className).toContain(
      "size-9",
    );
  });

  it("can mount a top-center viewport clear of fixed bottom actions", () => {
    render(<AppToaster theme="light" position="top" />);

    const toaster = document.querySelector("[data-app-toaster]");
    expect(toaster?.getAttribute("data-y-position")).toBe("top");
    expect(toaster?.className).toContain("safe-area-inset-top");
    expect(toaster?.className).not.toContain("safe-area-inset-bottom");
  });

  it("stacks multiple desktop toasts and expands them on hover", () => {
    setHoverCapability(true);
    render(<AppToaster theme="light" />);

    act(() => {
      showAppToast({
        message: "Upload pending",
        durationMs: Number.POSITIVE_INFINITY,
      });
      showErrorToast("Upload failed");
      showAppToast({
        message: "Upload successful",
        tone: "success",
        durationMs: Number.POSITIVE_INFINITY,
      });
    });

    const stack = document.querySelector<HTMLElement>("[data-app-toast-stack]");
    const toasts = document.querySelectorAll("[data-app-toast]");
    expect(stack?.getAttribute("data-hover-capable")).toBe("true");
    expect(stack?.getAttribute("data-stacked")).toBe("true");
    expect(toasts).toHaveLength(3);
    expect(toasts[0]?.getAttribute("data-stack-depth")).toBe("2");
    expect(toasts[0]?.getAttribute("data-stacked-behind")).toBe("true");
    expect(toasts[2]?.getAttribute("data-stacked-behind")).toBe("false");

    fireEvent.mouseEnter(stack as HTMLElement);
    expect(stack?.getAttribute("data-stacked")).toBe("false");
    expect(toasts[0]?.getAttribute("data-stacked-behind")).toBe("false");

    fireEvent.mouseLeave(stack as HTMLElement);
    expect(stack?.getAttribute("data-stacked")).toBe("true");
  });

  it("keeps multiple mobile toasts separated without hover behavior", () => {
    setHoverCapability(false);
    render(<AppToaster theme="light" />);

    act(() => {
      showAppToast({ message: "First", durationMs: Number.POSITIVE_INFINITY });
      showAppToast({ message: "Second", durationMs: Number.POSITIVE_INFINITY });
    });

    const stack = document.querySelector<HTMLElement>("[data-app-toast-stack]");
    expect(stack?.getAttribute("data-hover-capable")).toBe("false");
    expect(stack?.getAttribute("data-stacked")).toBe("false");

    fireEvent.mouseEnter(stack as HTMLElement);
    expect(stack?.getAttribute("data-stacked")).toBe("false");
  });

  it("morphs between expanded details and the compact title", async () => {
    vi.useFakeTimers();
    render(<AppToaster theme="light" />);

    act(() => {
      showAppToast({
        message: "Changes saved",
        description: "Your workspace has been updated.",
        durationMs: Number.POSITIVE_INFINITY,
      });
    });

    const toast = screen.getByRole("status");
    expect(toast.getAttribute("data-expanded")).toBe("true");
    expect(screen.getByText("Your workspace has been updated.")).toBeTruthy();

    fireEvent.click(
      screen.getByRole("button", { name: "Collapse notification" }),
    );
    expect(toast.getAttribute("data-expanded")).toBe("false");
    expect(
      screen.getByRole("button", { name: "Expand notification" }),
    ).toBeTruthy();
  });

  it("dismisses to the side when the toast body is clicked", async () => {
    vi.useFakeTimers();
    render(<AppToaster theme="light" />);

    act(() => {
      showSuccessToast("Changes saved");
    });

    fireEvent.click(screen.getByText("Changes saved"));
    expect(screen.getByRole("status").getAttribute("data-phase")).toBe(
      "exiting",
    );
    expect(screen.getByRole("status").getAttribute("data-exit-direction")).toBe(
      "1",
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

  it("auto-dismisses after three seconds by default", async () => {
    vi.useFakeTimers();
    render(<AppToaster theme="light" />);

    act(() => {
      showAppToast({ message: "Default duration notice" });
    });
    await act(() => vi.advanceTimersByTimeAsync(0));

    await act(() => vi.advanceTimersByTimeAsync(2_900));
    expect(document.querySelector("[data-app-toast]")?.textContent).toContain(
      "Default duration notice",
    );

    await act(() => vi.advanceTimersByTimeAsync(500));
    expect(document.querySelector("[data-app-toast]")).toBeNull();
  });

  it("auto-dismisses after durationMs", async () => {
    vi.useFakeTimers();
    render(<AppToaster theme="light" />);

    act(() => {
      showAppToast({ message: "Temporary notice", durationMs: 1_000 });
    });
    await act(() => vi.advanceTimersByTimeAsync(0));

    expect(document.querySelector("[data-app-toast]")?.textContent).toContain(
      "Temporary notice",
    );

    // The custom viewport keeps the node through its side-exit animation.
    await act(() => vi.advanceTimersByTimeAsync(1_500));

    expect(document.querySelector("[data-app-toast]")).toBeNull();
  });

  it("wraps long messages instead of truncating them", async () => {
    vi.useFakeTimers();
    render(<AppToaster theme="light" />);

    act(() => {
      showAppToast({
        message: "A notification long enough to wrap onto another line",
      });
    });
    await act(() => vi.advanceTimersByTimeAsync(0));

    const message = document.querySelector('[data-slot="app-toast-message"]');
    expect(message?.classList.contains("whitespace-normal")).toBe(true);
    expect(message?.classList.contains("break-words")).toBe(true);
    expect(message?.classList.contains("truncate")).toBe(false);
  });
});
