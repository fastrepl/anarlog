import { isTauri } from "@tauri-apps/api/core";

import { commands } from "~/types/tauri.gen";

export async function getOnboardingNeeded(): Promise<boolean> {
  if (!isTauri()) {
    return false;
  }

  const result = await commands.getOnboardingNeeded();
  return result.status === "ok" && result.data;
}

export async function resolveShellEntryPath(): Promise<"/app/main"> {
  return "/app/main";
}

export async function resolveAppEntryPath(): Promise<
  "/app/main" | "/app/onboarding"
> {
  if (await getOnboardingNeeded()) {
    return "/app/onboarding";
  }

  return resolveShellEntryPath();
}

export function normalizeAppPath(pathname: string): string {
  if (pathname.length > 1 && pathname.endsWith("/")) {
    return pathname.slice(0, -1);
  }

  return pathname;
}

export function isShellEntryPath(pathname: string): boolean {
  const normalizedPath = normalizeAppPath(pathname);
  return (
    normalizedPath === "/app" ||
    normalizedPath === "/app/main" ||
    normalizedPath === "/app/main2"
  );
}

export function isExperimentalShellPath(pathname: string): boolean {
  const normalizedPath = normalizeAppPath(pathname);
  return (
    normalizedPath === "/app/main2" || normalizedPath.startsWith("/app/main2/")
  );
}
