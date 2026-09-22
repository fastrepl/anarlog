import { z } from "zod";

import { desktopSchemeSchema } from "../functions/desktop-flow.ts";

const route = "/app/google-drive-picker";
const returnSchema = z.object({
  requestId: z.string().uuid(),
  scheme: desktopSchemeSchema,
  port: z.number().int().min(1024).max(65535).optional(),
  expiresAt: z.number(),
});
let handoff: {
  authorizationUrl?: string;
  callback?: string;
  port?: number;
} | null = null;

export function drivePickerCallback(
  scheme: string,
  requestId: string,
  result: string | null,
) {
  const validScheme = desktopSchemeSchema.parse(scheme);
  z.string().uuid().parse(requestId);
  const params = new URLSearchParams({
    integration_id: "google-drive",
    status: result ? "success" : "cancelled",
    return_to: `drive-picker:${requestId}:${result ?? ""}`,
  });
  return `${validScheme}://integration/callback?${params}`;
}

export function prepareDrivePickerHandoff() {
  if (
    typeof window === "undefined" ||
    window.location.pathname.replace(/\/$/, "") !== route
  )
    return;
  const query = new URLSearchParams(window.location.search);
  const fragment = new URLSearchParams(window.location.hash.slice(1));
  // OAuth codes must be cleared before analytics and error reporting initialize.
  window.history.replaceState(
    window.history.state,
    "",
    window.location.pathname,
  );
  handoff = null;
  try {
    const authorizationUrl = fragment.get("authorization_url");
    if (authorizationUrl) {
      const url = new URL(authorizationUrl);
      const state = url.searchParams.get("state");
      if (
        url.origin !== "https://accounts.google.com" ||
        url.pathname !== "/o/oauth2/v2/auth" ||
        !state ||
        state.length > 8192
      )
        return;
      const port = query.get("callback_port");
      const destination = returnSchema.parse({
        requestId: fragment.get("request_id"),
        scheme: query.get("scheme"),
        port: port ? Number(port) : undefined,
        expiresAt: Date.now() + 600_000,
      });
      window.sessionStorage.setItem(
        `drive-picker:${state}`,
        JSON.stringify(destination),
      );
      handoff = { authorizationUrl };
      return;
    }
    const state = query.get("state");
    if (!state || state.length > 8192) return;
    const key = `drive-picker:${state}`;
    const stored = window.sessionStorage.getItem(key);
    window.sessionStorage.removeItem(key);
    if (!stored) return;
    const destination = returnSchema.parse(JSON.parse(stored));
    if (destination.expiresAt <= Date.now()) return;
    const code = query.get("code") ?? "";
    const folderId = query.get("picked_file_ids") ?? "";
    const error = query.get("error");
    if (code.length > 8192 || (!error && !/^[\w-]{1,256}$/.test(folderId)))
      return;
    const result = encodeURIComponent(
      JSON.stringify({
        state,
        code,
        folder_id: folderId,
        ...(error
          ? {
              error:
                error === "access_denied"
                  ? "access_denied"
                  : "authorization_failed",
            }
          : {}),
      }),
    );
    handoff = {
      callback: drivePickerCallback(
        destination.scheme,
        destination.requestId,
        result,
      ),
      port: destination.port,
    };
  } catch {
    handoff = null;
  }
}

export function consumeDrivePickerHandoff() {
  const result = handoff;
  handoff = null;
  return result;
}
