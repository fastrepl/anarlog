import { commands as authCommands } from "@anlg/plugin-auth";

export const clearAuthStorage = async (): Promise<void> => {
  try {
    await authCommands.clear();
  } catch {
    // Ignore storage errors
  }
};
