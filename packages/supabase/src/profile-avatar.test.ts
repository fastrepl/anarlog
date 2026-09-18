import type { SupabaseClient } from "@supabase/supabase-js";
import { expect, it, vi } from "vitest";

import { saveProfileAvatar } from "./profile-avatar";

function fixture() {
  const user = {
    id: "user-1",
    user_metadata: {
      profile_avatar: { url: "https://storage.example/user-1/old.jpg" },
    },
  };
  const upload = vi.fn().mockResolvedValue({ error: null });
  const remove = vi.fn().mockResolvedValue({ error: null });
  const updateUser = vi.fn().mockImplementation(async ({ data }) => ({
    data: { user: { ...user, user_metadata: data } },
    error: null,
  }));
  const getUser = vi.fn().mockResolvedValue({ data: { user }, error: null });
  const client = {
    auth: { getUser, updateUser },
    storage: {
      from: () => ({
        upload,
        remove,
        getPublicUrl: (path: string) => ({
          data: { publicUrl: `https://storage.example/${path}` },
        }),
      }),
    },
  } as unknown as SupabaseClient;
  return { client, upload, remove, updateUser, getUser };
}

it("uploads bytes and publishes only a URL, then deletes the old object", async () => {
  const f = fixture();
  const user = await saveProfileAvatar(
    f.client,
    "user-1",
    "data:image/jpeg;base64,YWJj",
  );
  expect(f.upload).toHaveBeenCalledWith(
    expect.stringMatching(/^user-1\/.+\.jpg$/),
    new Uint8Array([97, 98, 99]),
    { contentType: "image/jpeg", upsert: false },
  );
  expect(user.user_metadata.profile_avatar.url).toMatch(
    /^https:\/\/storage.example\/user-1\/.+\.jpg$/,
  );
  expect(JSON.stringify(f.updateUser.mock.calls)).not.toContain("base64");
  expect(f.remove).toHaveBeenCalledWith(["user-1/old.jpg"]);
});

it("persists an explicit removal and does not upload", async () => {
  const f = fixture();
  await saveProfileAvatar(f.client, "user-1", null);
  expect(f.upload).not.toHaveBeenCalled();
  expect(f.updateUser).toHaveBeenCalledWith({
    data: { profile_avatar: { url: null } },
  });
});

it("keeps the published photo if upload fails", async () => {
  const f = fixture();
  f.upload.mockResolvedValue({ error: new Error("offline") });
  await expect(
    saveProfileAvatar(f.client, "user-1", "data:image/jpeg;base64,YWJj"),
  ).rejects.toThrow("offline");
  expect(f.updateUser).not.toHaveBeenCalled();
  expect(f.remove).not.toHaveBeenCalled();
});

it("removes an unreferenced upload when metadata publishing fails", async () => {
  const f = fixture();
  f.updateUser.mockResolvedValue({ error: new Error("offline") });
  await expect(
    saveProfileAvatar(f.client, "user-1", "data:image/jpeg;base64,YWJj"),
  ).rejects.toThrow("offline");
  expect(f.remove).toHaveBeenCalledWith([f.upload.mock.calls[0][0]]);
  expect(f.remove).not.toHaveBeenCalledWith(["user-1/old.jpg"]);
});

it("does not update the newly signed-in account after switching during upload", async () => {
  const f = fixture();
  f.getUser.mockResolvedValueOnce({
    data: { user: { id: "user-1", user_metadata: {} } },
    error: null,
  });
  f.getUser.mockResolvedValueOnce({
    data: { user: { id: "user-2" } },
    error: null,
  });
  await expect(
    saveProfileAvatar(f.client, "user-1", "data:image/jpeg;base64,YWJj"),
  ).rejects.toThrow("Account changed");
  expect(f.updateUser).not.toHaveBeenCalled();
});

it.each([
  "https://example.com/photo.jpg",
  "data:image/svg+xml;base64,YWJj",
  `data:image/jpeg;base64,${"Y".repeat(400000)}`,
])("rejects unsupported photo input", async (input) => {
  const f = fixture();
  await expect(saveProfileAvatar(f.client, "user-1", input)).rejects.toThrow();
  expect(f.upload).not.toHaveBeenCalled();
});

it("retains an upload when publication may have succeeded but cannot be verified", async () => {
  const f = fixture();
  f.updateUser.mockImplementation(async () => {
    f.getUser.mockRejectedValue(new Error("offline"));
    return { error: new Error("lost response") };
  });
  await expect(
    saveProfileAvatar(f.client, "user-1", "data:image/jpeg;base64,YWJj"),
  ).rejects.toThrow("lost response");
  expect(f.remove).not.toHaveBeenCalled();
});
