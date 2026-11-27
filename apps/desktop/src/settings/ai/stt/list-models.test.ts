import { beforeEach, describe, expect, it, vi } from "vitest";

const { providerFetchMock } = vi.hoisted(() => ({
  providerFetchMock: vi.fn(),
}));

vi.mock("~/ai/provider-fetch", () => ({
  providerFetch: providerFetchMock,
}));

import {
  isListableRemoteSttProvider,
  listSttProviderModels,
} from "./list-models";

describe("listSttProviderModels", () => {
  beforeEach(() => {
    providerFetchMock.mockReset();
  });

  it("identifies listable remote STT providers", () => {
    expect(isListableRemoteSttProvider("deepgram")).toBe(true);
    expect(isListableRemoteSttProvider("soniox")).toBe(true);
    expect(isListableRemoteSttProvider("elevenlabs")).toBe(true);
    expect(isListableRemoteSttProvider("soniqo")).toBe(false);
    expect(isListableRemoteSttProvider("custom")).toBe(false);
  });

  it("fetches Deepgram STT models with Token auth", async () => {
    providerFetchMock.mockResolvedValue(
      new Response(
        JSON.stringify({
          stt: [
            { name: "general", canonical_name: "nova-3-general" },
            { name: "nova-2" },
          ],
          tts: [{ canonical_name: "aura-2-thalia-en" }],
        }),
        { status: 200 },
      ),
    );

    const models = await listSttProviderModels(
      "deepgram",
      "https://api.deepgram.com/v1",
      "dg-key",
    );

    expect(providerFetchMock).toHaveBeenCalledWith(
      "https://api.deepgram.com/v1/models",
      expect.objectContaining({
        headers: { Authorization: "Token dg-key" },
      }),
    );
    expect(models).toEqual(["nova-3-general", "nova-2"]);
  });

  it("fetches ElevenLabs Scribe models and excludes pure TTS models", async () => {
    providerFetchMock.mockResolvedValue(
      new Response(
        JSON.stringify([
          { model_id: "eleven_multilingual_v2", can_do_text_to_speech: true },
          { model_id: "scribe_v2", can_do_text_to_speech: false },
          { model_id: "scribe_v2_realtime" },
        ]),
        { status: 200 },
      ),
    );

    const models = await listSttProviderModels(
      "elevenlabs",
      "https://api.elevenlabs.io",
      "xi-key",
    );

    expect(providerFetchMock).toHaveBeenCalledWith(
      "https://api.elevenlabs.io/v1/models",
      expect.objectContaining({
        headers: { "xi-api-key": "xi-key" },
      }),
    );
    expect(models).toEqual(["scribe_v2", "scribe_v2_realtime"]);
  });

  it("fetches Soniox models with Bearer auth", async () => {
    providerFetchMock.mockResolvedValue(
      new Response(
        JSON.stringify({
          models: [{ id: "stt-rt-v5" }, { id: "stt-async-v4" }],
        }),
        { status: 200 },
      ),
    );

    const models = await listSttProviderModels(
      "soniox",
      "https://api.soniox.com",
      "soniox-key",
    );

    expect(providerFetchMock).toHaveBeenCalledWith(
      "https://api.soniox.com/v1/models",
      expect.objectContaining({
        headers: { Authorization: "Bearer soniox-key" },
      }),
    );
    expect(models).toEqual(["stt-rt-v5", "stt-async-v4"]);
  });

  it("filters OpenAI and OpenRouter results down to transcription/audio models", async () => {
    providerFetchMock
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            data: [
              { id: "gpt-4o" },
              { id: "whisper-1" },
              { id: "gpt-4o-transcribe" },
            ],
          }),
          { status: 200 },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            data: [
              {
                id: "anthropic/claude-sonnet-4",
                architecture: { input_modalities: ["text", "image"] },
              },
              {
                id: "openai/gpt-4o-audio-preview",
                architecture: { input_modalities: ["text", "audio"] },
              },
            ],
          }),
          { status: 200 },
        ),
      );

    await expect(
      listSttProviderModels("openai", "https://api.openai.com/v1", "oa-key"),
    ).resolves.toEqual(["whisper-1", "gpt-4o-transcribe"]);
    await expect(
      listSttProviderModels(
        "openrouter",
        "https://openrouter.ai/api/v1",
        "or-key",
      ),
    ).resolves.toEqual(["openai/gpt-4o-audio-preview"]);
  });

  it("returns empty list on HTTP failure without throwing", async () => {
    providerFetchMock.mockResolvedValue(
      new Response("unauthorized", { status: 401 }),
    );

    await expect(
      listSttProviderModels("soniox", "https://api.soniox.com", "bad-key"),
    ).resolves.toEqual([]);
  });
});
