import type { SupabaseClient, User } from "@supabase/supabase-js";

const BUCKET = "profile-avatars";

export async function saveProfileAvatar(
  client: SupabaseClient,
  userId: string,
  dataUrl: string | null,
): Promise<User> {
  const { data, error } = await client.auth.getUser();
  if (error) throw error;
  if (data.user?.id !== userId)
    throw new Error("Account changed during photo update");

  const bucket = client.storage.from(BUCKET);
  let path: string | undefined;
  let url: string | null = null;
  if (dataUrl !== null) {
    const match =
      /^data:(image\/(?:jpeg|png));base64,([A-Za-z0-9+/]+={0,2})$/.exec(
        dataUrl,
      );
    if (!match) throw new Error("Invalid profile photo");
    const bytes = Uint8Array.from(atob(match[2]), (char) => char.charCodeAt(0));
    if (!bytes.length || bytes.length > 262144)
      throw new Error("Profile photo is too large");
    path = `${userId}/${crypto.randomUUID()}.${match[1] === "image/png" ? "png" : "jpg"}`;
    const upload = await bucket.upload(path, bytes, {
      contentType: match[1],
      upsert: false,
    });
    if (upload.error) throw upload.error;
    url = bucket.getPublicUrl(path).data.publicUrl;
  }
  try {
    const current = await client.auth.getUser();
    if (current.error) throw current.error;
    if (current.data.user?.id !== userId)
      throw new Error("Account changed during photo update");
    const updated = await client.auth.updateUser({
      data: { profile_avatar: { url } },
    });
    if (updated.error) throw updated.error;
    if (!updated.data.user || updated.data.user.id !== userId)
      throw new Error("Account changed during photo update");
    const oldUrl = data.user.user_metadata?.profile_avatar?.url;
    const prefix = bucket.getPublicUrl(`${userId}/`).data.publicUrl;
    if (typeof oldUrl === "string" && oldUrl.startsWith(prefix)) {
      const oldPath = `${userId}/${oldUrl.slice(prefix.length)}`;
      await removeAvatar(bucket, oldPath);
    }
    return updated.data.user;
  } catch (error) {
    if (path) {
      // A lost update response can still mean the URL was published.
      const current = await client.auth.getUser().catch(() => null);
      if (
        !current?.error &&
        current?.data.user?.id === userId &&
        current.data.user.user_metadata?.profile_avatar?.url !== url
      ) {
        await removeAvatar(bucket, path);
      }
    }
    throw error;
  }
}

async function removeAvatar(
  bucket: ReturnType<SupabaseClient["storage"]["from"]>,
  path: string,
) {
  let failure: unknown;
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      const { error } = await bucket.remove([path]);
      if (!error) return;
      failure = error;
    } catch (error) {
      failure = error;
    }
  }
  console.warn("Profile photo cleanup failed", failure);
}
