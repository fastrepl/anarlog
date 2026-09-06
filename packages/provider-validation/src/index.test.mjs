import assert from "node:assert/strict";
import { test } from "node:test";

import {
  ProviderCredentialError,
  verifyProviderCredentials,
  providerCredentialIdentity,
} from "./index.ts";

const credential = {
  provider: "openai",
  baseUrl: "https://api.openai.com/v1",
  apiKey: "synthetic-key",
};

test("accepts an authenticated model response, without sending keys in URLs", async () => {
  const requests = [];
  await verifyProviderCredentials(credential, async (url, init) => {
    requests.push({ url, init });
    return Response.json({ data: [] });
  });
  assert.equal(requests[0].url, "https://api.openai.com/v1/models");
  assert.equal(requests[0].init.headers.Authorization, "Bearer synthetic-key");
  assert.equal(requests[0].init.redirect, "error");
  assert.equal(requests.length, 1);
  assert.ok(
    !providerCredentialIdentity(credential).includes(credential.apiKey),
  );
  assert.notEqual(
    providerCredentialIdentity(credential),
    providerCredentialIdentity({ ...credential, apiKey: "other" }),
  );
});

for (const [status, message] of [
  [401, /rejected/],
  [403, /permissions/],
  [429, /rate limiting/],
  [500, /Couldn’t verify/],
  [404, /Couldn’t verify/],
]) {
  test(`does not save on HTTP ${status}, or expose response secrets`, async () => {
    await assert.rejects(
      verifyProviderCredentials(
        credential,
        async () => new Response("secret-key-echo", { status }),
      ),
      (error) => {
        assert.match(error.message, message);
        assert.ok(!error.message.includes("secret-key-echo"));
        assert.ok(error instanceof ProviderCredentialError);
        assert.equal(error.retryable, status !== 401 && status !== 403);
        return true;
      },
    );
  });
}

for (const body of [
  { error: "invalid key", data: [] },
  { valid: false },
  "<html>success</html>",
  null,
]) {
  test(`does not treat an arbitrary 200 response as verification: ${JSON.stringify(body)}`, async () => {
    await assert.rejects(
      verifyProviderCredentials(credential, async () => Response.json(body)),
    );
  });
}

test("a public model catalog cannot validate a key", async () => {
  await assert.rejects(
    verifyProviderCredentials({ ...credential, provider: "custom" }, async () =>
      Response.json({ data: [] }),
    ),
    /doesn’t support API key verification/,
  );
});

test("a gateway must accept the candidate key and reject the control key", async () => {
  const keys = [];
  await verifyProviderCredentials(
    { ...credential, provider: "custom" },
    async (_url, init) => {
      keys.push(init.headers.Authorization);
      return keys.length === 1
        ? Response.json({ data: [] })
        : new Response(null, { status: 401 });
    },
  );
  assert.deepEqual(keys, [
    "Bearer synthetic-key",
    "Bearer anarlog-invalid-key-verification",
  ]);
});

for (const status of [429, 500, 404]) {
  test(`an inconclusive control response (${status}) is retryable and never cached as proof`, async () => {
    let calls = 0;
    let controlStatus = status;
    const fetcher = async (_url, init) => {
      calls++;
      return init.headers.Authorization === "Bearer synthetic-key"
        ? Response.json({ data: [] })
        : new Response(null, { status: controlStatus });
    };
    const custom = { ...credential, provider: "custom" };
    await assert.rejects(
      verifyProviderCredentials(custom, fetcher),
      (error) => {
        assert.ok(error instanceof ProviderCredentialError);
        assert.equal(error.retryable, true);
        assert.doesNotMatch(error.message, /doesn’t support/);
        return true;
      },
    );
    controlStatus = 401;
    await verifyProviderCredentials(custom, fetcher);
    assert.equal(calls, 4);
  });
}

test("Vertex verifies tokens through the beta publisher catalog, separate from the project inference path", async () => {
  const requests = [];
  await verifyProviderCredentials(
    {
      ...credential,
      provider: "google_vertex_ai",
      baseUrl:
        "https://us-central1-aiplatform.googleapis.com/v1/projects/test-project/locations/us-central1/endpoints/openapi",
    },
    async (url, init) => {
      requests.push({ url, authorization: init.headers.Authorization });
      return requests.length === 1
        ? Response.json({
            publisherModels: [{ name: "publishers/google/models/test" }],
          })
        : new Response(null, { status: 401 });
    },
  );
  assert.deepEqual(requests, [
    {
      url: "https://us-central1-aiplatform.googleapis.com/v1beta1/publishers/google/models?pageSize=1",
      authorization: "Bearer synthetic-key",
    },
    {
      url: "https://us-central1-aiplatform.googleapis.com/v1beta1/publishers/google/models?pageSize=1",
      authorization: "Bearer anarlog-invalid-key-verification",
    },
  ]);
});

for (const [provider, path, header, value, payload] of [
  [
    "openrouter",
    "/v1/key",
    "Authorization",
    "Bearer synthetic-key",
    { data: { label: "test key" } },
  ],
  [
    "deepgram",
    "/v1/projects",
    "Authorization",
    "Token synthetic-key",
    { projects: [] },
  ],
  [
    "assemblyai",
    "/v1/v2/transcript?limit=1",
    "Authorization",
    "synthetic-key",
    { transcripts: [] },
  ],
  ["anthropic", "/v1/models", "x-api-key", "synthetic-key", { data: [] }],
  [
    "google_generative_ai",
    "/v1/models",
    "x-goog-api-key",
    "synthetic-key",
    { models: [] },
  ],
  [
    "siliconflow",
    "/v1/user/info",
    "Authorization",
    "Bearer synthetic-key",
    { status: true },
  ],
  [
    "cohere",
    "/v1/check-api-key",
    "Authorization",
    "Bearer synthetic-key",
    { valid: true },
  ],
  [
    "gladia",
    "/v1/v2/live?limit=1",
    "x-gladia-key",
    "synthetic-key",
    { items: [] },
  ],
  [
    "elevenlabs",
    "/v1/v1/user",
    "xi-api-key",
    "synthetic-key",
    { user_id: "test" },
  ],
  [
    "azure_openai",
    "/v1/openai/models?api-version=2024-10-21",
    "api-key",
    "synthetic-key",
    { data: [] },
  ],
]) {
  test(`uses ${provider}'s authentication protocol`, async () => {
    const host =
      provider === "anthropic"
        ? "api.anthropic.com"
        : provider === "google_generative_ai"
          ? "generativelanguage.googleapis.com"
          : "api.openai.com";
    await verifyProviderCredentials(
      { ...credential, provider, baseUrl: `https://${host}/v1` },
      async (url, init) => {
        assert.equal(url, `https://${host}${path}`);
        assert.equal(init.headers[header], value);
        return Response.json(payload);
      },
    );
  });
}

test("a negative Cohere validity result is rejected despite HTTP 200", async () => {
  await assert.rejects(
    verifyProviderCredentials({ ...credential, provider: "cohere" }, async () =>
      Response.json({ valid: false }),
    ),
    /did not confirm/,
  );
});

test("cancellation does not validate a key", async () => {
  const controller = new AbortController();
  await assert.rejects(
    verifyProviderCredentials(
      credential,
      async () => {
        controller.abort();
        return Response.json({ data: [] });
      },
      controller.signal,
    ),
    /Couldn’t verify/,
  );
});

test("network failures are not mislabeled as invalid credentials", async () => {
  await assert.rejects(
    verifyProviderCredentials(credential, async () => {
      throw Error("secret-key");
    }),
    /Couldn’t verify this key. Check your connection/,
  );
});

test("rejects malformed credentials and unsafe endpoints before a request", async () => {
  for (const update of [
    { apiKey: "" },
    { apiKey: "a\nb" },
    { baseUrl: "not-a-url" },
    { baseUrl: "http://remote.test" },
    { baseUrl: "https://u:p@remote.test" },
    { baseUrl: "https://remote.test/?token=test" },
  ]) {
    await assert.rejects(
      verifyProviderCredentials({ ...credential, ...update }, () =>
        assert.fail("Unexpected network request"),
      ),
      (error) => {
        assert.ok(error instanceof ProviderCredentialError);
        assert.equal(error.retryable, false);
        return true;
      },
    );
  }
});

test("public catalogs at overridden OpenAI endpoints are not trusted", async () => {
  await assert.rejects(
    verifyProviderCredentials(
      { ...credential, baseUrl: "https://public.example/v1" },
      async () => Response.json({ data: [] }),
    ),
    /doesn’t support API key verification/,
  );
});

test("reuses recent proof only for the same credential and endpoint", async () => {
  let requests = 0;
  const fetcher = async () => {
    requests++;
    return Response.json({ data: [] });
  };
  await verifyProviderCredentials(credential, fetcher);
  await verifyProviderCredentials(credential, fetcher);
  assert.equal(requests, 1);
  await verifyProviderCredentials(
    { ...credential, apiKey: "replacement" },
    fetcher,
  );
  assert.equal(requests, 2);
  await verifyProviderCredentials(
    { ...credential, baseUrl: credential.baseUrl + "/new" },
    fetcher,
  );
  assert.equal(requests, 3);
});

test("never caches a failed verification as accepted", async () => {
  let rejected = true;
  let calls = 0;
  const fetcher = async () => {
    calls++;
    return rejected
      ? new Response(null, { status: 401 })
      : Response.json({ data: [] });
  };
  await assert.rejects(
    verifyProviderCredentials(credential, fetcher),
    /rejected/,
  );
  rejected = false;
  await verifyProviderCredentials(credential, fetcher);
  assert.equal(calls, 2);
});
