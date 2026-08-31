import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";
import { StrictMode, act, createElement } from "react";
import { createRoot } from "react-dom/client";

import {
  attemptDesktopAppOpen,
  buildDesktopAuthCallbackPath,
  buildDesktopAuthDeeplink,
  getDesktopAppOpenLinkProps,
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
});

test("builds a web callback that preserves the sign-in method", () => {
  assert.equal(
    buildDesktopAuthCallbackPath(
      "fake access",
      "fake&refresh",
      "anarlog-staging",
      "email",
    ),
    "/callback/auth?flow=desktop&access_token=fake+access&refresh_token=fake%26refresh&scheme=anarlog-staging&method=email",
  );
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

test("keeps the manual open link independently clickable", () => {
  const dom = new JSDOM();
  const deeplink = "anarlog-staging://auth/callback?access_token=a";
  const anchor = dom.window.document.createElement("a");
  const props = getDesktopAppOpenLinkProps(deeplink);
  let clicks = 0;

  anchor.href = props.href;
  anchor.rel = props.rel;
  anchor.addEventListener("click", (event) => {
    clicks += 1;
    event.preventDefault();
  });
  anchor.click();

  assert.equal(anchor.getAttribute("href"), deeplink);
  assert.equal(anchor.rel, "noreferrer");
  assert.equal(clicks, 1);
  dom.window.close();
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
