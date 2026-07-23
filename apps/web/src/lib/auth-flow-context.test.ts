import assert from "node:assert/strict";
import test from "node:test";

import {
  resolveAuthFlowContext,
  toAuthFlowSearch,
} from "./auth-flow-context.ts";

test("restores desktop recovery context from the encoded Supabase redirect", () => {
  const context = resolveAuthFlowContext({
    redirectTo:
      "https://anarlog.so/callback/auth?flow=desktop&scheme=anarlog-staging&redirect=%2Fshare%2Finvite%2Fabc%2F",
  });

  assert.deepEqual(context, {
    flow: "desktop",
    scheme: "anarlog-staging",
    redirect: "/share/invite/abc/",
  });
  assert.deepEqual(toAuthFlowSearch(context), context);
});

test("keeps legacy desktop schemes valid during migration", () => {
  assert.deepEqual(
    resolveAuthFlowContext({
      redirectTo:
        "https://anarlog.so/callback/auth?flow=desktop&scheme=hyprnote",
    }),
    { flow: "desktop", scheme: "hyprnote" },
  );
});

test("defaults new desktop flows to the canonical Anarlog scheme", () => {
  assert.deepEqual(resolveAuthFlowContext({ flow: "desktop" }), {
    flow: "desktop",
    scheme: "anarlog",
  });
});

test("ignores unrelated redirect URLs and unsafe return paths", () => {
  assert.deepEqual(
    resolveAuthFlowContext({
      redirectTo: "https://attacker.example/callback?flow=desktop&scheme=char",
    }),
    { flow: "web" },
  );

  assert.deepEqual(
    resolveAuthFlowContext({
      flow: "web",
      redirect: "//attacker.example",
    }),
    { flow: "web", redirect: "/app/account/" },
  );
});

test("explicit route context takes precedence over template redirect context", () => {
  assert.deepEqual(
    resolveAuthFlowContext({
      flow: "web",
      redirect: "/app/account/",
      redirectTo: "https://anarlog.so/callback/auth?flow=desktop&scheme=char",
    }),
    { flow: "web", scheme: "char", redirect: "/app/account/" },
  );
});
