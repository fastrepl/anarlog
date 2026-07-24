import assert from "node:assert/strict";
import test from "node:test";

import {
  attemptDesktopAppOpen,
  buildDesktopAuthDeeplink,
} from "./desktop-auth-handoff.ts";

test("builds an encoded desktop auth callback", () => {
  assert.equal(
    buildDesktopAuthDeeplink("anarlog-staging", "fake access", "fake&refresh"),
    "anarlog-staging://auth/callback?access_token=fake+access&refresh_token=fake%26refresh",
  );
  assert.equal(
    buildDesktopAuthDeeplink("anarlog-staging", undefined, "fake-refresh"),
    null,
  );
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
