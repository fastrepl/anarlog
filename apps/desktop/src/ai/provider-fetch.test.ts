import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { verifyProviderCredentials } from "@anlg/provider-validation";

import { providerFetch } from "./provider-fetch";

vi.mock("@tauri-apps/plugin-http", () => ({ fetch: vi.fn() }));

beforeEach(() => {
  vi.mocked(tauriFetch).mockReset();
  vi.mocked(tauriFetch).mockResolvedValue(Response.json({ data: [] }));
});

describe("providerFetch", () => {
  it.each([
    "http://localhost:8000/v1/models",
    "http://127.0.0.1:8000/v1/models",
    "http://[::1]:8000/v1/models",
    "https://localhost:8000/v1/models",
  ])(
    "suppresses the injected origin for %s without changing credentials",
    async (url) => {
      const controller = new AbortController();
      const headers = { Authorization: "Bearer local-key" };
      const init = {
        headers,
        signal: controller.signal,
        redirect: "error" as const,
      };

      await providerFetch(url, init);

      const sent = vi.mocked(tauriFetch).mock.calls[0][1];
      expect(new Headers(sent?.headers).get("Origin")).toBe("");
      expect(new Headers(sent?.headers).get("Authorization")).toBe(
        "Bearer local-key",
      );
      expect(new Headers(sent?.headers).get("User-Agent")).toBe("Anarlog");
      expect(sent?.signal).toBe(controller.signal);
      expect(sent?.redirect).toBe("error");
      expect(init.headers).toEqual({ Authorization: "Bearer local-key" });
    },
  );

  it("preserves a Request's headers and body and honors init header overrides", async () => {
    const request = new Request("http://127.0.0.1:8000/v1/chat/completions", {
      method: "POST",
      headers: { Authorization: "Bearer original-key" },
      body: '{"model":"mtplx"}',
    });
    await providerFetch(request);
    const [input, init] = vi.mocked(tauriFetch).mock.calls[0];
    expect(input).toBe(request);
    expect(new Headers(init?.headers).get("Authorization")).toBe(
      "Bearer original-key",
    );
    expect(new Headers(init?.headers).get("User-Agent")).toBe("Anarlog");
    expect(new Headers(init?.headers).get("Origin")).toBe("");
    expect(await request.clone().text()).toBe('{"model":"mtplx"}');

    await providerFetch(request, {
      headers: { Authorization: "Bearer replacement-key" },
    });
    expect(
      new Headers(vi.mocked(tauriFetch).mock.calls[1][1]?.headers).get(
        "Authorization",
      ),
    ).toBe("Bearer replacement-key");
    expect(
      new Headers(vi.mocked(tauriFetch).mock.calls[1][1]?.headers).get(
        "User-Agent",
      ),
    ).toBe("Anarlog");
  });

  it("preserves provider headers while adding Anarlog attribution", async () => {
    const url = new URL("http://localhost:11434/v1/chat/completions");
    const init = { headers: { Origin: "http://localhost:11434" } };
    await providerFetch(url, init);
    const headers = new Headers(
      vi.mocked(tauriFetch).mock.calls[0][1]?.headers,
    );
    expect(headers.get("Origin")).toBe("http://localhost:11434");
    expect(headers.get("User-Agent")).toBe("Anarlog");
    expect(init.headers).toEqual({ Origin: "http://localhost:11434" });
  });

  it.each([
    "https://api.openai.com/v1/models",
    "http://192.168.1.10:8000/v1/models",
    "https://localhost.example.com/v1/models",
    "https://127.0.0.1.example.com/v1/models",
  ])(
    "uses Anarlog without adding Origin for remote provider requests: %s",
    async (url) => {
      const init = { headers: { Authorization: "Bearer remote-key" } };
      await providerFetch(url, init);
      const [input, sent] = vi.mocked(tauriFetch).mock.calls[0];
      expect(input).toBe(url);
      expect(new Headers(sent?.headers).get("Authorization")).toBe(
        "Bearer remote-key",
      );
      expect(new Headers(sent?.headers).get("User-Agent")).toBe("Anarlog");
      expect(new Headers(sent?.headers).get("Origin")).toBeNull();
      expect(init.headers).toEqual({ Authorization: "Bearer remote-key" });
    },
  );

  it("keeps an existing User-Agent and appends Anarlog once", async () => {
    await providerFetch("https://api.example.com/v1/chat/completions", {
      headers: { "User-Agent": "ProviderSDK/1.0" },
    });
    expect(
      new Headers(vi.mocked(tauriFetch).mock.calls[0][1]?.headers).get(
        "User-Agent",
      ),
    ).toBe("ProviderSDK/1.0 Anarlog");
  });

  it.each(["custom", "openai"])(
    "verifies %s against an origin-restricted local endpoint and still rejects invalid keys",
    async (provider) => {
      vi.mocked(tauriFetch).mockImplementation(async (_input, init) => {
        const headers = new Headers(init?.headers);
        if (headers.get("Origin") !== "") {
          return Response.json(
            { error: { type: "origin_error" } },
            { status: 403 },
          );
        }
        return headers.get("Authorization") === "Bearer local-key"
          ? Response.json({ data: [{ id: "mtplx" }] })
          : Response.json(
              { error: { type: "authentication_error" } },
              { status: 401 },
            );
      });
      const credential = {
        provider,
        baseUrl: "http://127.0.0.1:8000/v1",
        apiKey: "local-key",
      };
      await expect(
        verifyProviderCredentials(credential, providerFetch),
      ).resolves.toBeUndefined();
      expect(
        vi
          .mocked(tauriFetch)
          .mock.calls.map(([, init]) =>
            new Headers(init?.headers).get("Authorization"),
          ),
      ).toEqual([
        "Bearer local-key",
        "Bearer anarlog-invalid-key-verification",
      ]);
      await expect(
        verifyProviderCredentials(
          { ...credential, apiKey: "wrong-key" },
          providerFetch,
        ),
      ).rejects.toThrow("The provider rejected this key");
    },
  );
});
