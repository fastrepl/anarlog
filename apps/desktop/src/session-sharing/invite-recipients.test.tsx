import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ShareInviteForm } from "./invite-recipients";

vi.mock("~/contacts/queries", () => ({
  useHumans: () => [],
}));

afterEach(cleanup);

describe("ShareInviteForm", () => {
  it("rounds the email field and invite button to match other share actions", () => {
    render(
      <ShareInviteForm
        invite={createInvite()}
        disabled={false}
        pending={false}
        onSubmit={vi.fn()}
      />,
    );

    const emailField = screen.getByRole("textbox", { name: "Invitee email" });
    const inviteButton = screen.getByRole("button", { name: "Invite" });

    expect(emailField.parentElement?.className).toContain("rounded-full");
    expect(inviteButton.className).toContain("rounded-full");
    expect(emailField.parentElement?.className).not.toContain("rounded-md");
    expect(inviteButton.className).not.toContain("rounded-md");
  });

  it("shows how many recipients the action will invite", () => {
    render(
      <ShareInviteForm
        invite={createInvite({
          recipients: [
            { name: "Ada", email: "ada@example.com" },
            { name: "Grace", email: "grace@example.com" },
          ],
          emails: ["ada@example.com", "grace@example.com"],
          canSubmit: true,
        })}
        disabled={false}
        pending={false}
        onSubmit={vi.fn()}
      />,
    );

    const inviteButton = screen.getByRole("button", { name: "Invite" });

    expect(inviteButton.textContent).toContain("(2)");
  });
});

function createInvite(overrides: Record<string, unknown> = {}) {
  return {
    query: "",
    setQuery: vi.fn(),
    recipients: [],
    emails: [],
    canSubmit: false,
    isSelectable: () => false,
    add: vi.fn(),
    commitQuery: vi.fn(),
    remove: vi.fn(),
    restore: vi.fn(),
    ...overrides,
  };
}
