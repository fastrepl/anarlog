import assert from "node:assert/strict";
import test from "node:test";

import { observeReferralView } from "./referral-visibility.ts";

test("account visits do not count as referral views until the section is visible", () => {
  let callback!: IntersectionObserverCallback;
  let views = 0;
  const observer = {
    observe() {},
    disconnect() {},
  } as unknown as IntersectionObserver;
  const stop = observeReferralView(
    {} as Element,
    () => views++,
    (notify) => {
      callback = notify;
      return observer;
    },
  );
  const update = (isIntersecting: boolean) =>
    callback([{ isIntersecting } as IntersectionObserverEntry], observer);
  update(false);
  assert.equal(views, 0);
  update(true);
  update(false);
  update(true);
  assert.equal(views, 1);
  stop();
});
