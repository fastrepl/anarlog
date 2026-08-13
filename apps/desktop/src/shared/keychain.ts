import { commands as store2Commands } from "@anlg/plugin-store2";

const MACOS_KEYCHAIN_ACCESS_ERROR_PREFIX =
  "macOS couldn't access your login Keychain.";

export function isKeychainAccessError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return message.startsWith(MACOS_KEYCHAIN_ACCESS_ERROR_PREFIX);
}

export async function repairKeychainAccess(): Promise<void> {
  const result = await store2Commands.repairKeychainAccess();
  if (result.status === "error") {
    throw new Error(result.error);
  }
}
