import type { Session, SupabaseClient } from "@supabase/supabase-js";

import { isFatalSessionError } from "./errors";

export async function loadInitialSession(
  client: SupabaseClient,
): Promise<{ clearStorage: boolean; session: Session | null }> {
  try {
    const { data, error } = await client.auth.getSession();

    if (error) {
      return {
        clearStorage: isFatalSessionError(error),
        session: null,
      };
    }

    return {
      clearStorage: false,
      session: data.session ?? null,
    };
  } catch (error) {
    return {
      clearStorage: isFatalSessionError(error),
      session: null,
    };
  }
}
