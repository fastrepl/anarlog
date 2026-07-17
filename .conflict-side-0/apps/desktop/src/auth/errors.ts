import { AuthApiError, AuthSessionMissingError } from "@supabase/supabase-js";

import { commands as authCommands } from "@hypr/plugin-auth";

export const isFatalSessionError = (error: unknown): boolean => {
  if (error instanceof AuthSessionMissingError) {
    return true;
  }
  if (error instanceof AuthApiError) {
    const fatalCodes = [
      "refresh_token_not_found",
      "refresh_token_already_used",
    ];
    return fatalCodes.includes(error.code ?? "");
  }
  return false;
};

export const clearAuthStorage = async (): Promise<void> => {
  try {
    await authCommands.clear();
  } catch {
    // Ignore storage errors
  }
};
