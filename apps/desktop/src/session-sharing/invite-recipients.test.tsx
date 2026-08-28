import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  ShareInviteForm,
  ShareInviteSuggestions,
  shareInviteStyles,
} from "./invite-recipients";

import { expectStyle } from "~/session/stylex-test";

vi.mock("~/contacts/queries", () => ({
  useHumans: () => [],
}));

vi.mock("~/contacts/shared", () => ({
  ContactFacehash: ({ name }: { name: string }) => <span>{name[0]}</span>,
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

    expectStyle(emailField.parentElement, shareInviteStyles.inputShell);
    expectStyle(inviteButton, shareInviteStyles.submitButton);
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

describe("ShareInviteSuggestions", () => {
  it("marks suggested people as not invited without extra explanation", () => {
    render(
      <ShareInviteSuggestions
        invite={createInvite({
          recipients: [
            { name: "Ada", email: "ada@example.com" },
            { name: "Grace", email: "grace@example.com" },
          ],
        })}
        disabled={false}
      />,
    );

    expect(screen.getByText("Suggested attendees")).not.toBeNull();
    expect(screen.getAllByText("Not invited")).toHaveLength(2);
    expect(
      screen.queryByText(
        "Not invited yet. Nothing is sent until you click Invite.",
      ),
    ).toBeNull();
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
