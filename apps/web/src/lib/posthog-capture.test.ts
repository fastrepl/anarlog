import assert from "node:assert/strict";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import test from "node:test";
import { gunzipSync } from "node:zlib";
import type { CaptureResult } from "posthog-js";

import { sanitizePostHogEvent } from "./analytics-sanitization.ts";
import { isTelemetryPrivateLocation } from "./auth-route-privacy.ts";
import { getPostHogPersistenceName } from "./private-route-analytics-identity.ts";

const require = createRequire(import.meta.url);
const { JSDOM } = require("jsdom") as {
  JSDOM: new (
    html: string,
    options: { url: string; pretendToBeVisual: boolean },
  ) => { window: Window & typeof globalThis & { close: () => void } };
};

// Exercise the installed SDK's serialization and HTTP transport against a local
// ingest contract. No project credentials or production analytics writes in CI.
test(
  "PostHog capture authenticates after privacy filtering",
  { timeout: 15_000 },
  async (t) => {
    const projectToken = `phc_${"a".repeat(40)}`;
    const requests: {
      events: CaptureResult[];
      status: number;
      compression: string | null;
      pathname: string;
    }[] = [];
    const server = createServer(async (req, res) => {
      const chunks: Buffer[] = [];
      for await (const chunk of req) chunks.push(Buffer.from(chunk));
      const url = new URL(req.url!, "http://localhost");
      const compression = url.searchParams.get("compression");
      const bytes = Buffer.concat(chunks);
      const decoded = compression === "gzip-js" ? gunzipSync(bytes) : bytes;
      const payload = JSON.parse(decoded.toString());
      const events: CaptureResult[] = Array.isArray(payload)
        ? payload
        : [payload];
      const status = events.every(
        (event) => event.properties.token === projectToken,
      )
        ? 200
        : 401;
      requests.push({ events, status, compression, pathname: url.pathname });
      res.writeHead(status, { "Content-Type": "application/json" });
      res.end(
        JSON.stringify({ status: status === 200 ? "Ok" : "Invalid API key" }),
      );
    });
    await new Promise<void>((resolve) =>
      server.listen(0, "127.0.0.1", resolve),
    );
    t.after(() => {
      server.closeAllConnections();
      server.close();
    });
    const address = server.address();
    assert.ok(address && typeof address !== "string");
    const host = `http://127.0.0.1:${address.port}`;
    const dom = new JSDOM("", {
      url: `${host}/pricing?token=private-query`,
      pretendToBeVisual: true,
    });
    t.after(async () => {
      // Let pageview scroll measurements finish before disposing the document.
      await new Promise((resolve) => setTimeout(resolve, 0));
      dom.window.close();
    });
    for (const key of [
      "window",
      "document",
      "navigator",
      "location",
      "XMLHttpRequest",
    ] as const) {
      const previous = Object.getOwnPropertyDescriptor(globalThis, key);
      Object.defineProperty(globalThis, key, {
        configurable: true,
        value: key === "window" ? dom.window : dom.window[key],
      });
      t.after(() => {
        if (previous) Object.defineProperty(globalThis, key, previous);
        else Reflect.deleteProperty(globalThis, key);
      });
    }
    const { PostHog } = await import("posthog-js");
    for (const batching of [false, true]) {
      await t.test(`gzip capture with batching=${batching}`, async () => {
        const start = requests.length;
        const client = new PostHog();
        client.init(projectToken, {
          api_host: host,
          persistence_name: getPostHogPersistenceName(projectToken),
          autocapture: false,
          capture_pageview: true,
          capture_pageleave: false,
          disable_session_recording: true,
          disable_external_dependency_loading: true,
          advanced_disable_flags: true,
          request_batching: batching,
          request_queue_config: { flush_interval_ms: 250 },
          before_send: (event) =>
            isTelemetryPrivateLocation(
              window.location.pathname,
              window.location.search,
            )
              ? null
              : sanitizePostHogEvent(
                  event,
                  window.location.origin,
                  projectToken,
                ),
        });
        try {
          client.capture("download_clicked", {
            token: "private-access-token",
            email: "patient@example.com",
            user_id: "raw-account-id",
            nested: { token: "private-nested-token", provider: "openai" },
          });
          const deadline = Date.now() + 5000;
          while (
            requests.slice(start).flatMap((request) => request.events).length <
              2 &&
            Date.now() < deadline
          ) {
            await new Promise((resolve) => setTimeout(resolve, 25));
          }
          const captured = requests.slice(start);
          const events = captured.flatMap((request) => request.events);
          assert.deepEqual(events.map((event) => event.event).sort(), [
            "$pageview",
            "download_clicked",
          ]);
          for (const request of captured) {
            assert.equal(request.status, 200);
            assert.equal(request.pathname, "/e/");
            assert.equal(request.compression, "gzip-js");
          }
          for (const event of events) {
            assert.equal(event.properties.token, projectToken);
            assert.equal(
              event.properties.distinct_id,
              client.get_distinct_id(),
            );
            assert.equal(event.properties.$current_url, `${host}/pricing`);
          }
          const serialized = JSON.stringify(events);
          for (const secret of [
            "patient@example.com",
            "raw-account-id",
            "private-query",
            "private-access-token",
            "private-nested-token",
          ]) {
            assert.equal(serialized.includes(secret), false);
          }
          assert.equal(client.get_property("$user_state"), "anonymous");

          // This is the exact missing-field failure introduced by #7351.
          const missingToken = structuredClone(events[0]);
          delete missingToken.properties.token;
          const rejected = await fetch(`${host}/e/`, {
            method: "POST",
            body: JSON.stringify(missingToken),
          });
          assert.equal(rejected.status, 401);
          await rejected.text();

          for (const pathname of ["/app", "/oauth/authorize"]) {
            dom.window.history.replaceState({}, "", pathname);
            assert.equal(
              client.capture("$pageview", {}, { send_instantly: true }),
              undefined,
            );
          }
          dom.window.history.replaceState(
            {},
            "",
            "/pricing?token=private-query",
          );
        } finally {
          client.opt_out_capturing();
          client.sessionManager?.destroy();
        }
      });
      // opt_out_capturing persists across SDK instances.
      window.localStorage.clear();
      for (const cookie of document.cookie.split("; ")) {
        document.cookie = `${cookie.split("=")[0]}=; max-age=0; path=/`;
      }
    }
  },
);
