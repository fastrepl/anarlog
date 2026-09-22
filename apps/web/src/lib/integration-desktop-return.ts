import { z } from "zod";

export const callbackPortSchema = z.coerce
  .number()
  .int()
  .min(1024)
  .max(65535)
  .optional();

export function integrationReturnUrl(deeplink: string, port?: number) {
  const url = new URL(deeplink);
  if (port === undefined || url.protocol !== "anarlog-dev:") return deeplink;
  const validPort = callbackPortSchema.parse(port);
  if (url.hostname !== "integration" || url.pathname !== "/callback") {
    throw new Error("Invalid integration callback");
  }
  return `http://127.0.0.1:${validPort}/integration/callback${url.search}`;
}

export async function returnToDesktop(deeplink: string, port?: number) {
  const destination = integrationReturnUrl(deeplink, port);
  if (destination === deeplink) {
    window.location.assign(deeplink);
    return;
  }
  // The native listener emits the callback before closing. Do not render its
  // HTML, which attempts to open the unregistered development URL scheme.
  await fetch(destination, {
    mode: "no-cors",
    credentials: "omit",
    cache: "no-store",
  });
}
