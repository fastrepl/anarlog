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
});

function createInvite() {
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
  };
}
