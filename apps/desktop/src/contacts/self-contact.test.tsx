import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import type { HumanRecord } from "./queries";

const mocks = vi.hoisted(() => ({
  humans: [] as HumanRecord[],
  togglePin: vi.fn(),
  selectContact: vi.fn(),
  contextMenu: vi.fn(),
  ownerId: "self",
  authId: null as string | null,
}));

vi.mock("~/auth", () => ({
  useOptionalAuth: () => ({
    session: mocks.authId ? { user: { id: mocks.authId } } : null,
  }),
}));
vi.mock("~/shared/owner-user", () => ({ useOwnerUserId: () => mocks.ownerId }));
vi.mock("~/shared/hooks/useNativeContextMenu", () => ({
  useNativeContextMenu: () => mocks.contextMenu,
}));
vi.mock("~/store/zustand/tabs", () => ({
  useTabs: (select: (state: unknown) => unknown) =>
    select({
      currentTab: { type: "contacts", state: { selected: null } },
      updateContactsTabState: mocks.selectContact,
      invalidateResource: vi.fn(),
    }),
}));
vi.mock("./queries", () => ({
  useHumans: () => mocks.humans,
  useOrganizations: () => [],
  useHumanSessions: () => [],
  toggleContactPin: mocks.togglePin,
  deleteHuman: vi.fn(),
  deleteOrganization: vi.fn(),
  reorderPinnedContacts: vi.fn(),
  createHuman: vi.fn(),
  createOrganization: vi.fn(),
  mergeHumans: vi.fn(),
  updateHuman: vi.fn(),
  updateContactAvatar: vi.fn(),
}));
vi.mock("./contact-summary", () => ({
  useContactSummary: () => ({ facts: [] }),
}));
vi.mock("./related-notes", () => ({ RelatedNotesSection: () => null }));
vi.mock("./new-person-form", () => ({ NewPersonForm: () => null }));
vi.mock("./shared", () => ({
  ContactFacehash: () => null,
  ColumnHeader: ({
    onSearchChange,
  }: {
    onSearchChange: (value: string) => void;
  }) => (
    <input
      aria-label="Search contacts"
      onChange={(event) => onSearchChange(event.target.value)}
    />
  ),
}));

import { DetailsColumn } from "./details";

import { ContactsNav } from "~/sidebar/contacts";

function human(id: string, name: string, pinned = false): HumanRecord {
  return {
    id,
    name,
    pinned,
    userId: "self",
    email: "same@example.com",
    phone: "123",
    jobTitle: "Engineer",
    organizationId: "",
    createdAt: "",
    linkedinUsername: "",
    memo: "",
    pinOrder: 0,
    avatarDataUrl: null,
    summary: null,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.ownerId = "self";
  mocks.authId = null;
  mocks.humans = [human("other", "Alice", true), human("self", "Zoe")];
  mocks.togglePin.mockResolvedValue(undefined);
});
afterEach(cleanup);

it("keeps your unpinned card before draggable pins and visible during search", () => {
  render(<ContactsNav />);
  const self = screen.getByRole("button", { name: /Zoe/ });
  const other = screen.getByRole("button", { name: /Alice/ });
  expect(
    self.compareDocumentPosition(other) & Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
  expect(self.closest("li")).toBeNull();
  expect(other.closest("li")).not.toBeNull();
  const pin = screen.getByRole("img", { name: "Pinned contact" });
  fireEvent.click(pin);
  expect(mocks.selectContact).toHaveBeenCalledWith(expect.anything(), {
    selected: { type: "person", id: "self" },
  });
  fireEvent.contextMenu(self);
  expect(mocks.togglePin).not.toHaveBeenCalled();
  expect(mocks.contextMenu).not.toHaveBeenCalled();
  fireEvent.change(screen.getByRole("textbox"), {
    target: { value: "nobody" },
  });
  expect(screen.getAllByRole("button", { name: /Zoe/ })).toHaveLength(1);
});

it("renders your details without edit, photo, merge, or delete controls", () => {
  const { rerender } = render(
    <DetailsColumn
      human={mocks.humans[1]}
      humans={mocks.humans}
      organizations={[]}
      handleSessionClick={vi.fn()}
      onDelete={vi.fn()}
    />,
  );
  expect(screen.queryByRole("textbox")).toBeNull();
  expect(screen.queryByRole("button", { name: "Change photo" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Contact options" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Merge" })).toBeNull();
  expect(screen.getByText("Engineer")).not.toBeNull();
  rerender(
    <DetailsColumn
      human={mocks.humans[0]}
      humans={mocks.humans}
      organizations={[]}
      handleSessionClick={vi.fn()}
      onDelete={vi.fn()}
    />,
  );
  expect(screen.getAllByRole("textbox").length).toBeGreaterThan(0);
  expect(
    screen.getByRole("button", { name: "Contact options" }),
  ).not.toBeNull();
  expect(screen.queryByRole("button", { name: "Merge" })).toBeNull();
});
