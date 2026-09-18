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
  signedIn: true,
  getUser: vi.fn(),
  save: vi.fn(),
  mirror: vi.fn(),
}));
vi.mock("~/auth", () => ({
  useAuth: () => ({
    session: mocks.signedIn ? { user: { id: "account-1" } } : null,
    supabase: { auth: { getUser: mocks.getUser } },
  }),
}));
vi.mock("@anlg/supabase/profile-avatar", () => ({
  saveProfileAvatar: mocks.save,
}));
vi.mock("./queries", () => ({
  usePersonalContact: () => ({
    data: { avatarDataUrl: "data:image/jpeg;base64,bGVnYWN5" },
  }),
  updateContactAvatar: mocks.mirror,
}));
vi.mock("./shared", () => ({ ContactFacehash: () => <span>No photo</span> }));
vi.mock("./contact-avatar", () => ({
  ContactImage: ({ src }: { src: string }) => <img src={src} alt="Profile" />,
  AvatarUploadButton: ({
    children,
    onUpload,
  }: {
    children: React.ReactNode;
    onUpload: (value: string) => void;
  }) => (
    <button
      type="button"
      onClick={() => onUpload("data:image/jpeg;base64,bmV3")}
    >
      {children}Change photo
    </button>
  ),
}));
import { ProfilePhoto } from "./profile-photo";

function view(
  localPhoto: string | null = "data:image/jpeg;base64,bGVnYWN5",
  cachedPhoto?: string,
) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  if (cachedPhoto)
    queryClient.setQueryData(["profile-photo", "account-1"], {
      id: "account-1",
      user_metadata: { profile_avatar: { url: cachedPhoto } },
    });
  const onSave = vi.fn().mockResolvedValue(undefined);
  const result = render(
    <QueryClientProvider client={queryClient}>
      <ProfilePhoto
        userId="account-1"
        name="Ada"
        localPhoto={localPhoto}
        onSave={onSave}
      />
    </QueryClientProvider>,
  );
  return { ...result, queryClient, onSave };
}
beforeEach(() => {
  vi.clearAllMocks();
  mocks.signedIn = true;
  mocks.getUser.mockResolvedValue({
    data: { user: { id: "account-1", user_metadata: {} } },
    error: null,
  });
  mocks.save.mockResolvedValue({
    id: "account-1",
    user_metadata: {
      profile_avatar: { url: "https://storage.example/new.jpg" },
    },
  });
  mocks.mirror.mockResolvedValue(undefined);
});
afterEach(cleanup);

it("migrates a legacy local photo after checking the authoritative account", async () => {
  const { onSave } = view();
  await waitFor(() => expect(mocks.save).toHaveBeenCalledTimes(1));
  expect(mocks.save).toHaveBeenCalledWith(
    expect.anything(),
    "account-1",
    "data:image/jpeg;base64,bGVnYWN5",
  );
  await waitFor(() =>
    expect(onSave).toHaveBeenCalledWith("https://storage.example/new.jpg"),
  );
  expect(screen.getByRole("img").getAttribute("src")).toBe(
    "https://storage.example/new.jpg",
  );
});

it("uses a newer remote photo without uploading stale local data and mirrors remote removal", async () => {
  mocks.getUser.mockResolvedValue({
    data: {
      user: {
        id: "account-1",
        user_metadata: {
          profile_avatar: { url: "https://storage.example/remote.jpg" },
        },
      },
    },
    error: null,
  });
  const { queryClient } = view();
  await waitFor(() =>
    expect(screen.getByRole("img").getAttribute("src")).toBe(
      "https://storage.example/remote.jpg",
    ),
  );
  expect(mocks.save).not.toHaveBeenCalled();
  await waitFor(() =>
    expect(mocks.mirror).toHaveBeenCalledWith(
      "human",
      "account-1",
      "https://storage.example/remote.jpg",
    ),
  );
  queryClient.setQueryData(["profile-photo", "account-1"], {
    id: "account-1",
    user_metadata: {
      profile_avatar: { url: null },
      avatar_url: "https://provider.example/old.jpg",
    },
  });
  await waitFor(() => expect(screen.queryByRole("img")).toBeNull());
  await waitFor(() =>
    expect(mocks.mirror).toHaveBeenCalledWith("human", "account-1", null),
  );
});

it("keeps a failed upload available to retry", async () => {
  mocks.save.mockRejectedValueOnce(new Error("offline"));
  view(null);
  fireEvent.click(screen.getByRole("button", { name: /Change photo/ }));
  expect(await screen.findByRole("alert")).toBeTruthy();
  expect(screen.getByRole("img").getAttribute("src")).toBe(
    "data:image/jpeg;base64,bmV3",
  );
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  expect(mocks.save).toHaveBeenCalledTimes(2);
});

it("does not migrate a local photo when the cloud account cannot be read", async () => {
  mocks.getUser.mockResolvedValue({
    data: { user: null },
    error: new Error("offline"),
  });
  view();
  expect(await screen.findByRole("alert")).toBeTruthy();
  expect(mocks.save).not.toHaveBeenCalled();
});

it("keeps guest photo changes local even with a cached signed-in profile", async () => {
  mocks.signedIn = false;
  const { onSave } = view(null, "https://storage.example/cached.jpg");
  expect(screen.queryByRole("img")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: /Change photo/ }));
  await waitFor(() =>
    expect(onSave).toHaveBeenCalledWith("data:image/jpeg;base64,bmV3"),
  );
  expect(mocks.save).not.toHaveBeenCalled();
  expect(mocks.mirror).not.toHaveBeenCalled();
});
