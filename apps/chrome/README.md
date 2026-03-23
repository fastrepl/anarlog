# Char Chrome Extension (WXT)

Google Meet POC: DOM parsing + native messaging to `com.char.native_host`.

## Dev

- `pnpm -F @hypr/chrome dev`
- WXT launches a browser profile and hot-reloads extension changes.
- `pnpm -F @hypr/chrome dev:nobrowser` (builds + watches, but does not auto-launch Chrome)
- Popup UI uses React via `@wxt-dev/module-react`.
- Styling uses Tailwind CSS v4.
- Meet badge UI is injected via WXT Shadow Root UI to isolate styles from the page.

## No-browser dev workflow

1. Run `pnpm -F @hypr/chrome dev:nobrowser`.
2. Keep that process running for rebuilds/HMR.
3. In Chrome, open `chrome://extensions` and enable Developer mode.
4. Click Load unpacked and select `apps/chrome/.output/chrome-mv3-dev`.
5. Test popup via the extension icon or open `chrome-extension://<EXTENSION_ID>/popup.html`.

Notes:

- `http://localhost:3000` is WXT's internal dev server endpoint for extension tooling, not a normal app page.
- If you run `pnpm -F @hypr/chrome build`, load `apps/chrome/.output/chrome-mv3` instead.

## Testing

- `pnpm -F @hypr/chrome test:unit`
- `pnpm -F @hypr/chrome test:e2e`

## Build / Publish

- `pnpm -F @hypr/chrome build` (output: `apps/chrome/.output/chrome-mv3`)
- `pnpm -F @hypr/chrome zip` (output zip for Chrome Web Store upload)

## Native host

Requires a registered native messaging host `com.char.native_host` pointing to `char-chrome-native-host`.
