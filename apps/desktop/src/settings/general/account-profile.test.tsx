import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  authId: "account-1" as string | null,
  save: vi.fn(),
  contact: {
    data: null as Record<string, unknown> | null | undefined,
    isLoading: false,
    error: null as Error | null,
  },
  query: vi.fn(),
}));
vi.mock("~/auth", () => ({
  useAuth: () => ({
    session: mocks.authId
      ? {
          user: {
            id: mocks.authId,
            email: "login@example.com",
            user_metadata: { full_name: "Ada" },
          },
        }
      : null,
  }),
}));
vi.mock("~/shared/owner-user", () => ({ useOwnerUserId: () => "local-owner" }));
vi.mock("~/contacts/queries", () => ({
  usePersonalContact: (id: string) => {
    mocks.query(id);
    return mocks.contact;
  },
  useOrganizations: () => [{ id: "company-1", name: "Acme" }],
  savePersonalContact: mocks.save,
}));
vi.mock("~/contacts/shared", () => ({ ContactFacehash: () => null }));
vi.mock("~/contacts/contact-avatar", () => ({
  ContactImage: () => <img alt="Profile" />,
  AvatarUploadButton: ({ onUpload }: { onUpload: (value: string) => void }) => (
    <button
      type="button"
      onClick={() => onUpload("data:image/jpeg;base64,photo")}
    >
      Change photo
    </button>
  ),
}));
vi.mock("~/contacts/details", () => ({
  ContactOrganizationSelector: ({
    onChange,
    disabled,
  }: {
    onChange: (value: string) => void;
    disabled?: boolean;
  }) => (
    <button
      type="button"
      disabled={disabled}
      onClick={() => onChange("company-1")}
    >
      Choose company
    </button>
  ),
}));
import { AccountProfile } from "./account-profile";

function view() {
  return (
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { mutations: { retry: false } } })
      }
    >
      <AccountProfile />
    </QueryClientProvider>
  );
}
beforeEach(() => {
  vi.clearAllMocks();
  mocks.authId = "account-1";
  mocks.contact = { data: null, isLoading: false, error: null };
  mocks.save.mockResolvedValue(undefined);
});
afterEach(cleanup);

it("saves the contact fields and photo to the signed-in personal card", async () => {
  render(view());
  expect(mocks.query).toHaveBeenCalledWith("account-1");
  expect((screen.getByLabelText("Name") as HTMLInputElement).value).toBe("Ada");
  for (const [label, value] of [
    ["Name", "Ada Lovelace"],
    ["Job Title", "Engineer"],
    ["Email", "contact@example.com"],
    ["Phone", "+123"],
    ["LinkedIn", "ada"],
    ["Notes", "My notes"],
  ]) {
    fireEvent.change(screen.getByLabelText(label), { target: { value } });
  }
  fireEvent.click(screen.getByRole("button", { name: "Choose company" }));
  fireEvent.click(screen.getByRole("button", { name: "Change photo" }));
  await waitFor(() =>
    expect(mocks.save).toHaveBeenCalledWith("account-1", {
      name: "Ada Lovelace",
      jobTitle: "Engineer",
      email: "contact@example.com",
      phone: "+123",
      linkedinUsername: "ada",
      memo: "My notes",
      organizationId: "company-1",
      avatarDataUrl: "data:image/jpeg;base64,photo",
    }),
  );
  expect(await screen.findByText("Saved")).toBeTruthy();
});

it("preserves a failed draft for retry and saves offline info to the local owner", async () => {
  mocks.authId = null;
  mocks.save.mockRejectedValueOnce(new Error("Database unavailable"));
  render(view());
  fireEvent.change(screen.getByLabelText("Name"), {
    target: { value: "Local name" },
  });
  expect(await screen.findByRole("alert")).toBeTruthy();
  expect((screen.getByLabelText("Name") as HTMLInputElement).value).toBe(
    "Local name",
  );
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  await waitFor(() => expect(mocks.save).toHaveBeenCalledTimes(2));
  expect(mocks.save).toHaveBeenLastCalledWith(
    "local-owner",
    expect.objectContaining({ name: "Local name" }),
  );
  expect(await screen.findByText("Saved")).toBeTruthy();
});

it("waits for the saved profile and reports read failures without exposing an empty form", () => {
  mocks.contact = { data: undefined, isLoading: true, error: null };
  const { rerender } = render(view());
  expect(screen.getByRole("status")).toBeTruthy();
  expect(screen.queryByRole("textbox")).toBeNull();
  mocks.contact = {
    data: undefined,
    isLoading: false,
    error: new Error("Read failed"),
  };
  rerender(view());
  expect(screen.getByRole("alert")).toBeTruthy();
  expect(screen.queryByRole("textbox")).toBeNull();
});

it("keeps autosaves bound to the account being edited", async () => {
  const { rerender } = render(view());
  fireEvent.change(screen.getByLabelText("Name"), {
    target: { value: "First account draft" },
  });
  mocks.authId = "account-2";
  rerender(view());
  expect((screen.getByLabelText("Name") as HTMLInputElement).value).toBe("Ada");
  expect(mocks.query).toHaveBeenLastCalledWith("account-2");
  await waitFor(() =>
    expect(mocks.save).toHaveBeenCalledWith(
      "account-1",
      expect.objectContaining({ name: "First account draft" }),
    ),
  );
});

it("keeps fields editable while a save is pending", async () => {
  mocks.save.mockReturnValue(new Promise(() => {}));
  render(view());
  fireEvent.change(screen.getByLabelText("Name"), {
    target: { value: "Ada Lovelace" },
  });
  await waitFor(() => expect(mocks.save).toHaveBeenCalledOnce());
  expect(
    (
      screen.getByRole("button", {
        name: "Choose company",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(false);
});

it("loads saved contact fields and removes a photo without clearing other details", async () => {
  mocks.contact.data = {
    name: "Saved name",
    email: "saved@example.com",
    phone: "555",
    jobTitle: "Designer",
    linkedinUsername: "saved",
    memo: "Keep these notes",
    organizationId: "company-1",
    avatarDataUrl: "old-photo",
  };
  render(view());
  expect((screen.getByLabelText("Name") as HTMLInputElement).value).toBe(
    "Saved name",
  );
  expect((screen.getByLabelText("Email") as HTMLInputElement).value).toBe(
    "saved@example.com",
  );
  fireEvent.click(screen.getByRole("button", { name: "Remove photo" }));
  await waitFor(() =>
    expect(mocks.save).toHaveBeenCalledWith("account-1", {
      ...mocks.contact.data,
      avatarDataUrl: null,
    }),
  );
});

it("formats phone numbers on blur and saves the country-specific format", async () => {
  render(view());
  const phone = screen.getByLabelText("Phone") as HTMLInputElement;
  fireEvent.change(phone, { target: { value: "+821012345678" } });
  expect(phone.value).toBe("+821012345678");
  fireEvent.blur(phone);
  expect(phone.value).toBe("+82 10 1234 5678");
  await waitFor(() =>
    expect(mocks.save).toHaveBeenCalledWith(
      "account-1",
      expect.objectContaining({ phone: "+82 10 1234 5678" }),
    ),
  );
});

it("formats a saved phone without marking the profile dirty", () => {
  mocks.contact.data = { name: "Ada", phone: "+16693299320" };
  render(view());
  expect((screen.getByLabelText("Phone") as HTMLInputElement).value).toBe(
    "+1 669 329 9320",
  );
  expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
  expect(mocks.save).not.toHaveBeenCalled();
});

it("formats a phone on keyboard submit without requiring blur", async () => {
  render(view());
  const phone = screen.getByLabelText("Phone") as HTMLInputElement;
  fireEvent.change(phone, { target: { value: "+442079460018" } });
  fireEvent.submit(phone.closest("form")!);
  await waitFor(() =>
    expect(mocks.save).toHaveBeenCalledWith(
      "account-1",
      expect.objectContaining({ phone: "+44 20 7946 0018" }),
    ),
  );
});

it("does not replace newer edits when an earlier save finishes", async () => {
  let finishFirst!: () => void;
  mocks.save.mockImplementationOnce(
    () =>
      new Promise<void>((resolve) => {
        finishFirst = resolve;
      }),
  );
  const { rerender } = render(view());
  const name = screen.getByLabelText("Name") as HTMLInputElement;
  fireEvent.change(name, { target: { value: "First edit" } });
  await waitFor(() => expect(mocks.save).toHaveBeenCalledOnce());
  fireEvent.change(name, { target: { value: "Latest edit" } });
  await waitFor(() => expect(mocks.save).toHaveBeenCalledTimes(2));
  finishFirst();
  mocks.contact.data = { name: "First edit" };
  rerender(view());
  expect(name.value).toBe("Latest edit");
  expect(mocks.save).toHaveBeenLastCalledWith(
    "account-1",
    expect.objectContaining({ name: "Latest edit" }),
  );
});

it("submits edits immediately even when navigating away without blur", async () => {
  const { unmount } = render(view());
  fireEvent.change(screen.getByLabelText("Notes"), {
    target: { value: "Keep this note" },
  });
  unmount();
  await waitFor(() =>
    expect(mocks.save).toHaveBeenCalledWith(
      "account-1",
      expect.objectContaining({ memo: "Keep this note" }),
    ),
  );
});
