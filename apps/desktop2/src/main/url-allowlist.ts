// Renderer-reachable URL schemes for `shell.openExternal`. Anything else
// is rejected before it hits the OS — if the renderer is ever compromised
// (XSS, malicious rendered content) we don't want arbitrary protocol
// handlers to be a privilege-escalation vector (`file://`, `javascript:`,
// custom registered handlers like `vscode://`, `slack://`, etc).
//
// Additions to this list require a matching risk review:
// - `http:` / `https:` launch the default browser.
// - `mailto:` launches the default mail client with a pre-filled draft.
// Anything more invasive (deep links, custom schemes) needs a dedicated
// IPC handler, not this generic surface.
export const OPEN_EXTERNAL_ALLOWED_SCHEMES: ReadonlySet<string> = new Set([
  "http:",
  "https:",
  "mailto:",
]);

export function isAllowedExternalUrl(raw: string): boolean {
  try {
    return OPEN_EXTERNAL_ALLOWED_SCHEMES.has(new URL(raw).protocol);
  } catch {
    return false;
  }
}
