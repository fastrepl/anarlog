// Screenshots the parity frontend in headless Chromium at the native harness's window size
// (910x600 at 2x). Optional steps click elements by visible text or aria-label first.
//
// node apps/desktop-gpui/parity/shoot.mjs <url> <out.png> [--click <text> | --label <aria-label>]...
// CHROME=<path to chrome-headless-shell> overrides the Playwright cache lookup.
import { spawn } from "node:child_process";
import { readdirSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const [url, out, ...steps] = process.argv.slice(2);
if (!url || !out)
  throw new Error(
    "usage: shoot.mjs <url> <out.png> [--click <text> | --label <aria-label>]...",
  );
const settle = Number(process.env.PARITY_SETTLE_MS ?? 2500);
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function chrome() {
  if (process.env.CHROME) return process.env.CHROME;
  const cache = path.join(os.homedir(), "Library/Caches/ms-playwright");
  const shell = readdirSync(cache)
    .filter((name) => name.startsWith("chromium_headless_shell-"))
    .sort()
    .at(-1);
  if (!shell) throw new Error("No Playwright headless shell; set CHROME");
  return path.join(
    cache,
    shell,
    "chrome-headless-shell-mac-arm64/chrome-headless-shell",
  );
}

const port = 9300 + Math.floor(Math.random() * 600);
const browser = spawn(
  chrome(),
  [`--remote-debugging-port=${port}`, "--hide-scrollbars", "about:blank"],
  {
    stdio: "ignore",
  },
);
try {
  let targets;
  for (let attempt = 0; !targets && attempt < 50; attempt++) {
    await sleep(100);
    targets = await fetch(`http://127.0.0.1:${port}/json`).then(
      (r) => r.json(),
      () => undefined,
    );
  }
  const socket = new WebSocket(
    targets.find((target) => target.type === "page").webSocketDebuggerUrl,
  );
  await new Promise((resolve) => (socket.onopen = resolve));
  let nextId = 0;
  const pending = new Map();
  const problems = new Set();
  socket.onmessage = ({ data }) => {
    const message = JSON.parse(data);
    if (message.id) pending.get(message.id)?.(message);
    if (message.method === "Runtime.exceptionThrown") {
      problems.add(
        message.params.exceptionDetails.exception?.description?.split("\n")[0],
      );
    }
    if (
      message.method === "Runtime.consoleAPICalled" &&
      ["error", "warning"].includes(message.params.type)
    ) {
      problems.add(
        message.params.args
          .map((arg) => arg.value ?? arg.description)
          .join(" ")
          .slice(0, 200),
      );
    }
  };
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const id = ++nextId;
      pending.set(id, (message) =>
        message.error
          ? reject(new Error(message.error.message))
          : resolve(message.result),
      );
      socket.send(JSON.stringify({ id, method, params }));
    });

  await send("Runtime.enable");
  await send("Page.enable");
  await send("Emulation.setDeviceMetricsOverride", {
    width: 910,
    height: 600,
    deviceScaleFactor: 2,
    mobile: false,
  });
  await send("Page.navigate", { url });
  await sleep(settle);

  for (let index = 0; index < steps.length; index += 2) {
    const [kind, text] = [steps[index], steps[index + 1]];
    if (!["--click", "--label"].includes(kind))
      throw new Error(`unknown step ${kind}`);
    const matches =
      kind === "--label"
        ? `element.getAttribute("aria-label") === ${JSON.stringify(text)}`
        : `element.textContent.trim() === ${JSON.stringify(text)}`;
    // The innermost visible match, so the click lands on the control itself.
    const { result } = await send("Runtime.evaluate", {
      returnByValue: true,
      expression: `(() => {
        const matches = [...document.querySelectorAll("body *")].filter((element) =>
          (${matches}) && element.getClientRects().length);
        const target = matches.find((element) => ![...element.children].some((child) => matches.includes(child)));
        if (!target) return null;
        const box = target.getBoundingClientRect();
        return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
      })()`,
    });
    if (!result.value)
      throw new Error(`no visible element for ${kind} ${JSON.stringify(text)}`);
    for (const type of ["mouseMoved", "mousePressed", "mouseReleased"]) {
      await send("Input.dispatchMouseEvent", {
        type,
        ...result.value,
        button: "left",
        clickCount: 1,
      });
    }
    await sleep(settle);
  }

  const { data } = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(out, Buffer.from(data, "base64"));
  console.log([...problems].filter(Boolean).join("\n"));
  socket.close();
} finally {
  browser.kill();
}
