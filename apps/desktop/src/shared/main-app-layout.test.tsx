import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  windowLabel: "main",
}));

vi.mock("@tanstack/react-router", () => ({
  Outlet: () => <div data-testid="outlet" />,
  useNavigate: () => vi.fn(),
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({}),
}));

vi.mock("@anlg/plugin-windows", () => ({
  events: {},
  getCurrentWebviewWindowLabel: () => mocks.windowLabel,
}));

vi.mock("./useNewNote", () => ({
  openNewNoteAndListen: vi.fn(),
  openSessionAndListen: vi.fn(),
  useNewNote: () => vi.fn(),
}));

vi.mock("~/auth", () => ({
  AuthProvider: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="auth-provider">{children}</div>
  ),
}));

vi.mock("~/auth/billing", () => ({
  BillingProvider: ({ children }: { children: React.ReactNode }) => children,
}));

vi.mock("~/devtools-panel/host", () => ({
  DevtoolsFloatingPanelHost: () => null,
}));

vi.mock("~/session/queries", () => ({
  getOrCreateSessionForEventId: vi.fn(),
}));

vi.mock("~/services/meeting-import-sync", () => ({
  MeetingImportSync: () => <div data-testid="meeting-import-sync" />,
}));

vi.mock("~/settings/team/mirror", () => ({
  useMyWorkspacesWithMirror: vi.fn(),
}));

vi.mock("~/shared/hooks/useMountEffect", () => ({
  useMountEffect: vi.fn(),
}));

vi.mock("~/sidebar/toast/undo-delete-toast", () => ({
  UndoDeleteToast: () => null,
}));

vi.mock("~/store/zustand/tabs", () => ({
  isTabInputSupported: vi.fn(),
  useTabs: () => vi.fn(),
}));

import MainAppLayout from "./main-app-layout";

describe("MainAppLayout", () => {
  beforeEach(() => {
    mocks.windowLabel = "main";
  });

  afterEach(cleanup);

  it("mounts connected import sync inside the auth provider", () => {
    render(<MainAppLayout />);

    expect(
      screen
        .getByTestId("auth-provider")
        .contains(screen.getByTestId("meeting-import-sync")),
    ).toBe(true);
  });

  it("does not mount connected import sync in secondary windows", () => {
    mocks.windowLabel = "note";

    render(<MainAppLayout />);

    expect(screen.queryByTestId("meeting-import-sync")).toBeNull();
  });
});
