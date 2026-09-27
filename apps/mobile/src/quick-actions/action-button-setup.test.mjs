import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { registerHooks } from "node:module";
import { beforeEach, test } from "node:test";
import { fileURLToPath } from "node:url";

const sourceRoot = new URL("../", import.meta.url);
const fixture = (globalThis.actionButtonSetupFixture = {
  storage: new Map(),
  gate: null,
  failWrites: false,
});
const modules = {
  "@react-native-async-storage/async-storage": `const fixture = globalThis.actionButtonSetupFixture;
    export default {
      getItem: async key => {
        const value = fixture.storage.get(key) ?? null;
        if (fixture.gate) await fixture.gate.promise;
        return value;
      },
      setItem: async (key, value) => {
        if (fixture.failWrites) throw new Error('disk full');
        fixture.storage.set(key, value);
      },
    };`,
  "expo-device": `export const isDevice = false; export const modelId = null;`,
  "react-native": `export const Platform = {OS: 'ios'};`,
  "@/lib/error-reporting": `export function captureOperationalError() {}`,
};

registerHooks({
  resolve(specifier, context, next) {
    if (modules[specifier])
      return {
        url: `data:text/javascript,${encodeURIComponent(modules[specifier])}`,
        shortCircuit: true,
      };
    if (specifier.startsWith("@/"))
      return {
        url: new URL(`${specifier.slice(2)}.ts`, sourceRoot).href,
        shortCircuit: true,
      };
    if (
      specifier.startsWith(".") &&
      context.parentURL?.startsWith(sourceRoot.href)
    ) {
      const url = new URL(`${specifier}.ts`, context.parentURL);
      if (existsSync(fileURLToPath(url)))
        return { url: url.href, shortCircuit: true };
    }
    return next(specifier, context);
  },
});

const {
  actionButtonSetupQuery,
  dismissActionButtonCard,
  markStartListeningShortcutRan,
} = await import("./action-button-setup.ts");
const { queryClient } = await import("../lib/query-client.ts");

beforeEach(() => {
  queryClient.clear();
  fixture.storage.clear();
  fixture.gate = null;
  fixture.failWrites = false;
});

test("a shortcut run during the cold-start load is not overwritten", async () => {
  fixture.gate = Promise.withResolvers();
  const loading = queryClient
    .fetchQuery(actionButtonSetupQuery)
    .catch(() => {});
  await new Promise((resolve) => setImmediate(resolve));

  const marking = markStartListeningShortcutRan();
  await new Promise((resolve) => setImmediate(resolve));
  fixture.gate.resolve();
  await Promise.all([loading, marking]);

  assert.equal(fixture.storage.get("action-button-shortcut-ran"), "1");
  assert.deepEqual(queryClient.getQueryData(actionButtonSetupQuery.queryKey), {
    cardDismissed: false,
    shortcutRan: true,
  });
});

test("a failed save rolls the cached setup back to stored state", async () => {
  await queryClient.fetchQuery(actionButtonSetupQuery);
  fixture.failWrites = true;
  await dismissActionButtonCard();

  assert.deepEqual(queryClient.getQueryData(actionButtonSetupQuery.queryKey), {
    cardDismissed: false,
    shortcutRan: false,
  });
});

test("dismissing the card persists across reloads", async () => {
  await dismissActionButtonCard();
  queryClient.clear();

  assert.deepEqual(await queryClient.fetchQuery(actionButtonSetupQuery), {
    cardDismissed: true,
    shortcutRan: false,
  });
});
