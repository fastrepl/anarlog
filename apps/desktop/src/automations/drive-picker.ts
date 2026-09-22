import {
  googleDrivePickerStart,
  googleDrivePickerComplete,
} from "@anlg/api-client";
import type { createClient } from "@anlg/api-client/client";
import { commands as openerCommands } from "@anlg/plugin-opener2";

import { buildWebAppUrl } from "~/shared/utils";

const pending = new Map<string, (folderId: string | null) => void>();

export function completeDrivePicker(
  returnTo: string | null | undefined,
): boolean {
  if (!returnTo?.startsWith("drive-picker:")) return false;
  const [, requestId, folderId] = returnTo.split(":");
  const complete = pending.get(requestId);
  if (complete) {
    pending.delete(requestId);
    complete(folderId || null);
  }
  return true;
}

export async function pickDriveFolder(
  client: ReturnType<typeof createClient>,
  connectionId: string,
) {
  const { data, error } = await googleDrivePickerStart({
    client,
    body: { connection_id: connectionId },
  });
  if (error || !data)
    throw new Error(
      apiMessage(
        error,
        "Could not open Google Drive. Reconnect and try again.",
      ),
    );
  const requestId = crypto.randomUUID();
  const url = new URL(await buildWebAppUrl("/app/google-drive-picker"));
  // The browser receives an authorization URL, never a provider access token.
  url.hash = new URLSearchParams({
    authorization_url: data.authorization_url,
    request_id: requestId,
  }).toString();
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    const selected = new Promise<string | null>((resolve, reject) => {
      pending.set(requestId, resolve);
      timer = setTimeout(
        () => reject(new Error("Folder selection timed out. Try again.")),
        5 * 60_000,
      );
    });
    void selected.catch(() => {});
    // Attach immediately so a failed browser open cannot leave an unhandled rejection.
    const opened = openerCommands
      .openUrl(url.toString(), null)
      .then((result) => {
        if (result.status === "error")
          throw new Error("Could not open the folder picker");
        return selected;
      });
    const encoded = await opened;
    if (!encoded) return null;
    let selection: {
      state: string;
      code: string;
      folder_id: string;
      error?: string;
    };
    try {
      selection = JSON.parse(decodeURIComponent(encoded));
    } catch {
      throw new Error("Invalid Google folder selection response.");
    }
    if (selection.state !== data.state)
      throw new Error("Google folder selection did not match this request.");
    if (selection.error === "access_denied") return null;
    if (selection.error || !selection.code || !selection.folder_id)
      throw new Error("Google folder selection was not completed. Try again.");
    const result = await googleDrivePickerComplete({
      client,
      body: {
        state: selection.state,
        code: selection.code,
        folder_id: selection.folder_id,
      },
    });
    if (result.error || !result.data)
      throw new Error(
        apiMessage(result.error, "Choose a folder where you can add files."),
      );
    return result.data;
  } finally {
    clearTimeout(timer);
    pending.delete(requestId);
  }
}

function apiMessage(error: unknown, fallback: string): string {
  const message = (error as { error?: { message?: unknown } } | undefined)
    ?.error?.message;
  return typeof message === "string" ? message : fallback;
}
