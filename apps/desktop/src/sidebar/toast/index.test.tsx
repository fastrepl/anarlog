import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  signIn: vi.fn(),
  dismissToast: vi.fn(),
  openNew: vi.fn(),
  updateSettingsTabState: vi.fn(),
  clearDevtoolsPreview: vi.fn(),
  setToastActionTarget: vi.fn(),
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({
    session: null,
    signIn: mocks.signIn,
  }),
}));

vi.mock("~/contexts/notifications", () => ({
  useNotifications: () => ({
    hasActiveDownload: false,
    downloadProgress: null,
    downloadingModel: null,
    activeDownloads: [],
    localSttStatus: null,
    isLocalSttModel: false,
  }),
}));

vi.mock("~/shared/config", () => ({
  useConfigValues: () => ({
    current_llm_provider: "local",
    current_llm_model: "model",
    current_stt_provider: "local",
    current_stt_model: "model",
  }),
}));

vi.mock("~/store/zustand/devtools-toast-preview", () => ({
  useDevtoolsToastPreview: (
    selector: (state: { preview: null; clearPreview: () => void }) => unknown,
  ) =>
    selector({
      preview: null,
      clearPreview: mocks.clearDevtoolsPreview,
    }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (
    selector: (state: {
      currentTab: { type: string };
      openNew: () => void;
      updateSettingsTabState: () => void;
    }) => unknown,
  ) =>
    selector({
      currentTab: { type: "empty" },
      openNew: mocks.openNew,
      updateSettingsTabState: mocks.updateSettingsTabState,
    }),
}));

vi.mock("~/store/zustand/toast-action", () => ({
  useToastAction: (
    selector: (state: { setTarget: (target: "stt" | null) => void }) => unknown,
  ) => selector({ setTarget: mocks.setToastActionTarget }),
}));

vi.mock("./useDismissedToasts", () => ({
  useDismissedToasts: () => ({
    dismissToast: mocks.dismissToast,
    isDismissed: () => false,
  }),
}));

import { ToastArea } from "./index";

describe("ToastArea", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    mocks.signIn.mockClear();
    mocks.dismissToast.mockClear();
    mocks.openNew.mockClear();
    mocks.updateSettingsTabState.mockClear();
    mocks.clearDevtoolsPreview.mockClear();
    mocks.setToastActionTarget.mockClear();
  });

  afterEach(() => {
    cleanup();
    document.body.innerHTML = "";
    vi.useRealTimers();
  });

  it("keeps the default toast placement fixed to the existing top timeline position", () => {
    render(<ToastArea />);

    act(() => {
      vi.advanceTimersByTime(500);
    });

    const toastContainer = screen
      .getByText("Pro features available")
      .closest(".fixed") as HTMLElement | null;

    expect(toastContainer?.style.left).toBe("calc(50% + 0px)");
    expect(toastContainer?.style.top).toBe("56px");
  });

  it("positions the left sidebar toast relative to the main white surface", () => {
    const mainSurface = document.createElement("div");
    mainSurface.setAttribute("data-chat-floating-anchor", "");
    vi.spyOn(mainSurface, "getBoundingClientRect").mockReturnValue({
      bottom: 520,
      height: 500,
      left: 200,
      right: 800,
      top: 20,
      width: 600,
      x: 200,
      y: 20,
      toJSON: () => ({}),
    });
    document.body.appendChild(mainSurface);

    render(<ToastArea placement="left-sidebar" />);

    act(() => {
      vi.advanceTimersByTime(500);
    });

    const toastContainer = screen
      .getByText("Pro features available")
      .closest(".fixed") as HTMLElement | null;

    expect(toastContainer?.style.left).toBe("500px");
    expect(toastContainer?.style.top).toBe("56px");
  });

  it("repositions the left sidebar toast when the main surface scrolls", () => {
    const mainSurface = document.createElement("div");
    mainSurface.setAttribute("data-chat-floating-anchor", "");
    let top = 20;

    vi.spyOn(mainSurface, "getBoundingClientRect").mockImplementation(() => ({
      bottom: top + 500,
      height: 500,
      left: 200,
      right: 800,
      top,
      width: 600,
      x: 200,
      y: top,
      toJSON: () => ({}),
    }));
    document.body.appendChild(mainSurface);

    render(<ToastArea placement="left-sidebar" />);

    act(() => {
      vi.advanceTimersByTime(500);
    });

    const toastContainer = screen
      .getByText("Pro features available")
      .closest(".fixed") as HTMLElement | null;

    expect(toastContainer?.style.top).toBe("56px");

    act(() => {
      top = 52;
      window.dispatchEvent(new Event("scroll"));
    });

    expect(toastContainer?.style.top).toBe("88px");
  });

  it("uses fallback placement immediately when left sidebar anchoring is disabled", () => {
    const mainSurface = document.createElement("div");
    mainSurface.setAttribute("data-chat-floating-anchor", "");
    vi.spyOn(mainSurface, "getBoundingClientRect").mockReturnValue({
      bottom: 520,
      height: 500,
      left: 200,
      right: 800,
      top: 20,
      width: 600,
      x: 200,
      y: 20,
      toJSON: () => ({}),
    });
    document.body.appendChild(mainSurface);

    const { rerender } = render(<ToastArea placement="left-sidebar" />);

    act(() => {
      vi.advanceTimersByTime(500);
    });

    const toastContainer = screen
      .getByText("Pro features available")
      .closest(".fixed") as HTMLElement | null;

    expect(toastContainer?.style.left).toBe("500px");
    expect(toastContainer?.style.top).toBe("56px");

    rerender(<ToastArea />);

    expect(toastContainer?.style.left).toBe("calc(50% + 0px)");
    expect(toastContainer?.style.top).toBe("56px");
  });
});
