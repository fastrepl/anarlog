import { afterEach, expect, it, vi } from "vitest";

vi.mock("~/settings/ai/shared", () => ({
  AnarlogProviderIcon: () => null,
  ProviderBrandImage: () => null,
  ProviderLobeIcon: () => null,
}));
vi.mock("~/stt/useLocalSttModel", () => ({ localSttQueries: {} }));

afterEach(() => {
  vi.doUnmock("~/env");
  vi.resetModules();
});

it.each([undefined, "https://api.anarlog.so"])(
  "routes hosted providers with AI override %s while keeping the Core URL",
  async (aiUrl) => {
    vi.resetModules();
    vi.doMock("~/env", () => ({
      env: {
        VITE_API_URL: "http://localhost:3001",
        VITE_AI_API_URL: aiUrl,
      },
    }));
    const { PROVIDERS: stt } = await import("~/settings/ai/stt/shared");
    const { PROVIDERS: llm } = await import("~/settings/ai/llm/shared");
    const { env } = await import("~/env");
    expect(stt.find((provider) => provider.id === "anarlog")?.baseUrl).toBe(
      `${aiUrl ?? "http://localhost:3001"}/stt`,
    );
    expect(llm.find((provider) => provider.id === "anarlog")?.baseUrl).toBe(
      `${aiUrl ?? "http://localhost:3001"}/llm`,
    );
    expect(env.VITE_API_URL).toBe("http://localhost:3001");
  },
  30_000,
);
