import { cleanup, render, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  invitations: {
    data: undefined as
      | Array<{
          invitationId: string;
          workspaceId: string;
          workspaceName: string;
          workspaceLogoDataUrl: string | null;
          invitedByEmail: string | null;
          expiresAt: string;
        }>
      | undefined,
  },
  openNew: vi.fn(),
  toast: vi.fn(),
  dismiss: vi.fn(),
  inactive: vi.fn(),
  showNotification: vi.fn(),
  notificationsDisabled: false,
}));

vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children?: ReactNode }) => <>{children}</>,
  useLingui: () => ({
    t: (strings: TemplateStringsArray, ...values: unknown[]) =>
      strings.reduce(
        (message, part, index) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      ),
  }),
}));

vi.mock("@anlg/plugin-notification", () => ({
  commands: { showNotification: mocks.showNotification },
}));

vi.mock("~/shared/window-activity", () => ({
  isAppWindowInactive: mocks.inactive,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => mocks.notificationsDisabled,
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: Object.assign(mocks.toast, { dismiss: mocks.dismiss }),
}));

vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (selector: (state: { openNew: typeof mocks.openNew }) => unknown) =>
    selector({ openNew: mocks.openNew }),
}));

vi.mock("./my-invitations", () => ({
  MY_INVITATIONS_QUERY_KEY: "team-my-invitations",
  useMyWorkspaceInvitations: () => mocks.invitations,
}));

import { WorkspaceInvitationToasts } from "./invitation-toast";

const invitation = {
  invitationId: "inv-1",
  workspaceId: "ws-1",
  workspaceName: "Fastrepl",
  workspaceLogoDataUrl: null,
  invitedByEmail: "owner@example.com",
  expiresAt: "2026-09-01T00:00:00Z",
};

describe("WorkspaceInvitationToasts", () => {
  beforeEach(() => {
    mocks.invitations.data = undefined;
    mocks.notificationsDisabled = false;
    mocks.inactive.mockReset().mockResolvedValue(false);
    mocks.showNotification
      .mockReset()
      .mockResolvedValue({ status: "ok", data: null });
    mocks.openNew.mockClear();
    mocks.toast.mockClear();
    mocks.dismiss.mockClear();
  });

  afterEach(cleanup);

  it("toasts once per invitation and does not re-toast on refetch", () => {
    mocks.invitations.data = [invitation];
    const view = render(<WorkspaceInvitationToasts />);

    expect(mocks.toast).toHaveBeenCalledTimes(1);
    expect(mocks.toast).toHaveBeenCalledWith(
      "You've been invited to join Fastrepl",
      expect.objectContaining({
        id: "team-invitation:inv-1",
        duration: Infinity,
        description: "Invited by owner@example.com",
      }),
    );

    view.rerender(<WorkspaceInvitationToasts />);
    expect(mocks.toast).toHaveBeenCalledTimes(1);
  });

  it("dismisses the toast when the invitation disappears", () => {
    mocks.invitations.data = [invitation];
    const view = render(<WorkspaceInvitationToasts />);
    expect(mocks.toast).toHaveBeenCalledTimes(1);

    mocks.invitations.data = [];
    view.rerender(<WorkspaceInvitationToasts />);

    expect(mocks.dismiss).toHaveBeenCalledWith("team-invitation:inv-1");
  });

  it("opens the team settings tab from the View action", () => {
    mocks.invitations.data = [invitation];
    render(<WorkspaceInvitationToasts />);

    const options = mocks.toast.mock.calls[0]![1] as {
      action: { label: string; onClick: () => void };
    };
    expect(options.action.label).toBe("View");
    options.action.onClick();

    expect(mocks.openNew).toHaveBeenCalledWith({
      type: "settings",
      state: { tab: "team" },
    });
    expect(mocks.dismiss).toHaveBeenCalledWith("team-invitation:inv-1");
  });
  it("notifies once when the app is inactive", async () => {
    mocks.inactive.mockResolvedValue(true);
    mocks.invitations.data = [invitation];
    const view = render(<WorkspaceInvitationToasts />);
    await waitFor(() =>
      expect(mocks.showNotification).toHaveBeenCalledTimes(1),
    );
    expect(mocks.showNotification).toHaveBeenCalledWith(
      expect.objectContaining({
        key: "team-invitation:inv-1",
        title: "You've been invited to join Fastrepl",
        message: "Invited by owner@example.com",
        action_label: "View",
        source: null,
      }),
    );
    mocks.invitations.data = [{ ...invitation }];
    view.rerender(<WorkspaceInvitationToasts />);
    expect(mocks.showNotification).toHaveBeenCalledTimes(1);
  });

  it("keeps foreground invitations in the app", async () => {
    mocks.invitations.data = [invitation];
    render(<WorkspaceInvitationToasts />);
    await waitFor(() => expect(mocks.inactive).toHaveBeenCalled());
    expect(mocks.showNotification).not.toHaveBeenCalled();
    expect(mocks.toast).toHaveBeenCalledTimes(1);
  });

  it("respects disabled notifications while retaining the toast", () => {
    mocks.notificationsDisabled = true;
    mocks.invitations.data = [invitation];
    render(<WorkspaceInvitationToasts />);
    expect(mocks.showNotification).not.toHaveBeenCalled();
    expect(mocks.inactive).not.toHaveBeenCalled();
    expect(mocks.toast).toHaveBeenCalledTimes(1);
  });

  it("cancels pending notification delivery when an invitation disappears", async () => {
    let resolveActivity!: (inactive: boolean) => void;
    mocks.inactive.mockReturnValue(
      new Promise<boolean>((resolve) => {
        resolveActivity = resolve;
      }),
    );
    mocks.invitations.data = [invitation];
    const view = render(<WorkspaceInvitationToasts />);
    mocks.invitations.data = [];
    view.rerender(<WorkspaceInvitationToasts />);
    resolveActivity(true);
    await waitFor(() =>
      expect(mocks.dismiss).toHaveBeenCalledWith("team-invitation:inv-1"),
    );
    expect(mocks.showNotification).not.toHaveBeenCalled();
  });
});
