import type { Session, SupabaseClient } from "@supabase/supabase-js";

import { readPersistedAuthSession } from "./client";

export async function loadInitialSession(
  client: SupabaseClient,
): Promise<Session | null> {
  const storedSession = await readPersistedAuthSession();

  try {
    const { data, error } = await client.auth.getSession();

    if (error) {
      return storedSession;
    }

    return data.session ?? storedSession;
  } catch {
    return storedSession;
  }
}
