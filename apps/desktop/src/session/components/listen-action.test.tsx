import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  openNew: vi.fn(),
  startListening: vi.fn(),
}));

vi.mock("./floating/options-menu", () => ({
  OptionsMenu: ({
    children,
    onConfigure,
  }: {
    children: ReactNode;
    onConfigure?: () => void;
  }) => (
    <div>
      {children}
      {onConfigure ? (
        <button type="button" onClick={onConfigure}>
          Configure audio
        </button>
      ) : null}
    </div>
  ),
}));

vi.mock("./floating/shared", () => ({
  ActionableTooltipContent: () => null,
  FloatingButton: ({
    children,
    disabled,
    onClick,
  }: {
    children: ReactNode;
    disabled?: boolean;
    onClick?: () => void;
  }) => (
    <button type="button" disabled={disabled} onClick={onClick}>
      {children}
    </button>
  ),
}));

vi.mock("./shared", () => ({
  RecordingIcon: () => null,
  useCurrentNoteHasContent: () => false,
  useListenButtonState: () => ({
    shouldRender: true,
    isDisabled: false,
    warningMessage: "Session failed: microphone unavailable",
    recoverySettingsTab: "permissions",
  }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: { openNew: typeof mocks.openNew }) => unknown) =>
    selector({ openNew: mocks.openNew }),
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (
    selector: (state: {
      live: { loading: boolean; sessionId: string | null };
    }) => unknown,
  ) => selector({ live: { loading: false, sessionId: null } }),
}));

vi.mock("~/stt/useStartListening", () => ({
  useStartListening: () => mocks.startListening,
}));

vi.mock("~/stt/window-control", () => ({
  isMainWebviewWindow: () => true,
  requestMainListenerControl: vi.fn(),
}));

import { ListenActionButton } from "./listen-action";

describe("ListenActionButton", () => {
  afterEach(cleanup);

  beforeEach(() => {
    mocks.openNew.mockClear();
    mocks.startListening.mockClear();
  });

  it("opens Permissions without retrying after an audio start failure", () => {
    render(<ListenActionButton sessionId="session-1" />);

    fireEvent.click(screen.getByRole("button", { name: "Configure audio" }));

    expect(mocks.openNew).toHaveBeenCalledWith({
      type: "settings",
      state: { tab: "permissions" },
    });
    expect(mocks.startListening).not.toHaveBeenCalled();
  });
});
