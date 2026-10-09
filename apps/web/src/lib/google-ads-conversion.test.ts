import assert from "node:assert/strict";
import { test } from "node:test";

import { reportDownloadConversion } from "./google-ads-conversion.ts";

test("download navigation completes once when a conversion callback repeats", () => {
  let destination = "pending";
  let callback: () => void = () => {};
  reportDownloadConversion(
    (...args) => {
      callback = (args[2] as { event_callback: () => void }).event_callback;
    },
    () => {
      assert.equal(destination, "pending");
      destination = "download";
    },
  );
  assert.equal(destination, "pending");
  callback();
  callback();
  assert.equal(destination, "download");
});

test("blocked or missing tags cannot strand download navigation", async () => {
  for (const gtag of [
    undefined,
    () => {
      throw new Error("blocked");
    },
    () => {},
  ]) {
    await new Promise<void>((resolve) =>
      reportDownloadConversion(gtag, resolve),
    );
  }
});
