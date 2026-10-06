import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";
import { StrictMode, act, createElement } from "react";
import { createRoot } from "react-dom/client";

import { consumeDesktopAuthHandoff } from "./auth-route-privacy.ts";
import {
  attemptDesktopAppOpen,
  buildDesktopAuthCallbackPath,
  buildDesktopAuthDeeplink,
  resolveDesktopAuthCallbackMethod,
  useDesktopAppAutoOpen,
} from "./desktop-auth-handoff.ts";

const require = createRequire(import.meta.url);
const { JSDOM } = require("jsdom") as {
  JSDOM: new (
    html?: string,
    options?: { url?: string },
  ) => {
    window: Window & typeof globalThis & { close: () => void };
  };
};

test("builds an encoded desktop auth callback", () => {
  assert.equal(
    buildDesktopAuthDeeplink(
      "anarlog-staging",
      "fake access",
      "fake&refresh",
      "google",
    ),
    "anarlog-staging://auth/callback?access_token=fake+access&refresh_token=fake%26refresh&method=google",
  );
  assert.equal(
    buildDesktopAuthDeeplink("anarlog-staging", undefined, "fake-refresh"),
    null,
  );

  const nightly = buildDesktopAuthDeeplink(
    "anarlog-nightly",
    "access",
    "refresh",
  );
  assert.ok(nightly);
  assert.ok(nightly.startsWith("anarlog-nightly://auth/callback?"));
});

test("stores credentials before returning a token-free HTTP callback URL", () => {
  const dom = new JSDOM("", { url: "https://anarlog.so" });
  const previousWindow = globalThis.window;
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: dom.window,
  });
  try {
    const state = "8d58e770-3a95-41ba-bfa1-ec03633a3a55";
    const path = buildDesktopAuthCallbackPath(
      "fake access",
      "fake&refresh",
      "anarlog-staging",
      "email",
      state,
    );
    const url = new URL(path, dom.window.location.origin);
    assert.equal(url.searchParams.has("access_token"), false);
    assert.equal(url.searchParams.has("refresh_token"), false);
    assert.equal(url.searchParams.get("scheme"), "anarlog-staging");
    assert.equal(url.searchParams.get("desktop_state"), state);
    assert.equal(url.searchParams.get("method"), "email");
    assert.deepEqual(consumeDesktopAuthHandoff(Date.now(), state), {
      accessToken: "fake access",
      refreshToken: "fake&refresh",
    });
    assert.equal(consumeDesktopAuthHandoff(), null);
    Object.defineProperty(dom.window, "sessionStorage", {
      get() {
        throw new Error("storage unavailable");
      },
    });
    assert.throws(
      () => buildDesktopAuthCallbackPath("access", "refresh"),
      /storage unavailable/,
    );
  } finally {
    dom.window.close();
    restoreGlobal("window", previousWindow);
  }
});

test("prefers the requested desktop sign-in method over a remembered method", () => {
  assert.equal(resolveDesktopAuthCallbackMethod("github", "google"), "github");
  assert.equal(resolveDesktopAuthCallbackMethod(undefined, "google"), "google");
  assert.equal(resolveDesktopAuthCallbackMethod(undefined, null), undefined);
});

test("attempts the external protocol through an anchor navigation", () => {
  const events: string[] = [];
  const link = {
    href: "",
    hidden: false,
    rel: "",
    tabIndex: 0,
    click: () => events.push("click"),
    remove: () => events.push("remove"),
  };
  const documentRef = {
    createElement: (tagName: string) => {
      events.push(`create:${tagName}`);
      return link;
    },
    body: {
      append: (element: unknown) => {
        assert.equal(element, link);
        events.push("append");
      },
    },
  } as unknown as Document;

  attemptDesktopAppOpen("anarlog-staging://auth/callback", documentRef);

  assert.equal(link.href, "anarlog-staging://auth/callback");
  assert.equal(link.rel, "noreferrer");
  assert.equal(link.hidden, true);
  assert.equal(link.tabIndex, -1);
  assert.deepEqual(events, ["create:a", "append", "click", "remove"]);
});

test("attempts automatic opening once under StrictMode and remounts", async () => {
  const dom = new JSDOM('<div id="root"></div>', {
    url: "https://anarlog.so",
  });
  const previousWindow = globalThis.window;
  const previousDocument = globalThis.document;
  const previousNavigator = globalThis.navigator;
  const previousActEnvironment = Reflect.get(
    globalThis,
    "IS_REACT_ACT_ENVIRONMENT",
  );
  const deeplink = "anarlog-staging://auth/callback?access_token=a";
  const attempts: string[] = [];

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

  const handleClick = (event: Event) => {
    const target = event.target;
    if (
      target instanceof dom.window.HTMLAnchorElement &&
      target.href.startsWith("anarlog-staging:")
    ) {
      attempts.push(target.getAttribute("href") ?? "");
      event.preventDefault();
    }
  };
  document.addEventListener("click", handleClick);

  function AutoOpenProbe() {
    useDesktopAppAutoOpen(deeplink);
    return null;
  }

  const rootElement = document.getElementById("root");
  assert.ok(rootElement);
  let root = createRoot(rootElement);

  try {
    await act(async () => {
      root.render(
        createElement(StrictMode, null, createElement(AutoOpenProbe)),
      );
    });

    assert.deepEqual(attempts, [deeplink]);

    await act(async () => {
      root.unmount();
    });
    root = createRoot(rootElement);
    await act(async () => {
      root.render(
        createElement(StrictMode, null, createElement(AutoOpenProbe)),
      );
    });

    assert.deepEqual(attempts, [deeplink]);
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
