import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";
import { StrictMode, act, createElement } from "react";
import { createRoot } from "react-dom/client";

import {
  getNangoSessionToken,
  isDesktopIntegrationHandoff,
  prepareNangoSessionHandoff,
  useNangoSessionHandoffToken,
} from "./integration-handoff.ts";

const require = createRequire(import.meta.url);
const { JSDOM } = require("jsdom") as {
  JSDOM: new (
    html?: string,
    options?: { url?: string },
  ) => {
    window: Window & typeof globalThis & { close: () => void };
  };
};

test("recognizes a desktop Nango integration handoff", () => {
  assert.equal(
    isDesktopIntegrationHandoff({
      pathname: "/app/integration/",
      search: {
        flow: "desktop",
        action: "connect",
        handoff: "nango",
      },
    }),
    true,
  );
});

test("does not bypass app authentication for web or disconnect flows", () => {
  for (const search of [
    { flow: "web", action: "connect", handoff: "nango" },
    { flow: "desktop", action: "disconnect", handoff: "nango" },
    { flow: "desktop", action: "unknown", handoff: "nango" },
    { flow: "desktop", action: "connect" },
  ]) {
    assert.equal(
      isDesktopIntegrationHandoff({
        pathname: "/app/integration/",
        search,
      }),
      false,
    );
  }
});

test("reads the scoped Nango token from the URL fragment", () => {
  assert.equal(
    getNangoSessionToken("#session_token=nango%2Etoken%2Bvalue"),
    "nango.token+value",
  );
  assert.equal(getNangoSessionToken("#session_token="), null);
  assert.equal(
    getNangoSessionToken("#session_token=first&session_token=second"),
    null,
  );
  assert.equal(getNangoSessionToken("session_token=token"), null);
  assert.equal(getNangoSessionToken(""), null);
});

test("scrubs a Nango token before telemetry startup", () => {
  const dom = new JSDOM("", {
    url: "https://anarlog.so/app/integration?flow=desktop&action=connect&handoff=nango#session_token=nango.token",
  });
  const previousWindow = globalThis.window;

  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: dom.window,
  });

  try {
    prepareNangoSessionHandoff();
    assert.equal(dom.window.location.hash, "");
    assert.equal(
      dom.window.location.href,
      "https://anarlog.so/app/integration?flow=desktop&action=connect&handoff=nango",
    );
  } finally {
    dom.window.close();
    restoreGlobal("window", previousWindow);
  }
});

test("preserves the handoff token across StrictMode effect replay", async () => {
  const dom = new JSDOM('<div id="root"></div>', {
    url: "https://anarlog.so/app/integration?flow=desktop&action=connect&handoff=nango#session_token=nango.token",
  });
  const previousWindow = globalThis.window;
  const previousDocument = globalThis.document;
  const previousNavigator = globalThis.navigator;
  const previousActEnvironment = Reflect.get(
    globalThis,
    "IS_REACT_ACT_ENVIRONMENT",
  );
  const renderedTokens: Array<string | null | undefined> = [];

  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: dom.window,
  });
  Object.defineProperty(globalThis, "document", {
    configurable: true,
    value: dom.window.document,
  });
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: dom.window.navigator,
  });
  Object.defineProperty(globalThis, "IS_REACT_ACT_ENVIRONMENT", {
    configurable: true,
    value: true,
  });

  function HandoffProbe() {
    renderedTokens.push(useNangoSessionHandoffToken());
    return null;
  }

  const rootElement = document.getElementById("root");
  assert.ok(rootElement);
  const root = createRoot(rootElement);

  try {
    await act(async () => {
      root.render(createElement(StrictMode, null, createElement(HandoffProbe)));
    });

    assert.equal(renderedTokens.at(-1), "nango.token");
    assert.equal(renderedTokens.includes(null), false);
    assert.equal(dom.window.location.hash, "");
    assert.equal(
      dom.window.location.href,
      "https://anarlog.so/app/integration?flow=desktop&action=connect&handoff=nango",
    );
  } finally {
    await act(async () => {
      root.unmount();
    });
    dom.window.close();
    restoreGlobal("window", previousWindow);
    restoreGlobal("document", previousDocument);
    restoreGlobal("navigator", previousNavigator);
    restoreGlobal("IS_REACT_ACT_ENVIRONMENT", previousActEnvironment);
  }
});

function restoreGlobal(key: string, value: unknown) {
  if (value === undefined) {
    Reflect.deleteProperty(globalThis, key);
    return;
  }

  Object.defineProperty(globalThis, key, {
    configurable: true,
    value,
  });
}
