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
    expect(isListableRemoteSttProvider("openrouter")).toBe(false);
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

  it("fetches Soniox live models with Bearer auth and omits async-only IDs", async () => {
    providerFetchMock.mockResolvedValue(
      new Response(
        JSON.stringify({
          models: [{ id: "stt-rt-v5" }, { id: "stt-async-v5" }],
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
    expect(models).toEqual(["stt-rt-v5"]);
  });

  it("filters OpenAI results down to transcription models and keeps OpenRouter curated", async () => {
    providerFetchMock.mockResolvedValueOnce(
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
    ).resolves.toEqual([]);
  });

  it("excludes Mistral TTS models and keeps Deepgram-compatible Cloudflare STT models", async () => {
    providerFetchMock
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            data: [
              { id: "voxtral-mini-2602" },
              { id: "voxtral-mini-tts-3-0" },
              {
                id: "mistral-stt-cap",
                audio_transcription_capabilities: { realtime: true },
              },
              {
                id: "voxtral-disabled",
                capabilities: { audio_transcription: false },
              },
            ],
          }),
          { status: 200 },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            result: [
              { name: "@cf/deepgram/nova-3" },
              { name: "@cf/deepgram/flux" },
              { name: "@cf/openai/whisper" },
            ],
          }),
          { status: 200 },
        ),
      );

    await expect(
      listSttProviderModels("mistral", "https://api.mistral.ai/v1", "m-key"),
    ).resolves.toEqual(["voxtral-mini-2602", "mistral-stt-cap"]);
    await expect(
      listSttProviderModels(
        "cloudflare_workers_ai",
        "https://api.cloudflare.com/client/v4/accounts/acct/ai/v1",
        "cf-key",
      ),
    ).resolves.toEqual(["@cf/deepgram/nova-3", "@cf/deepgram/flux"]);
  });

  it("keeps Together transcription models and excludes audio TTS models", async () => {
    providerFetchMock.mockResolvedValue(
      new Response(
        JSON.stringify([
          { id: "openai/whisper-large-v3", type: "transcribe" },
          { id: "distil-whisper/distil-large-v3", type: "audio" },
          { id: "hexgrad/Kokoro-82M", type: "audio" },
          { id: "vendor/text-to-speech-v1", type: "audio" },
          { id: "meta-llama/Llama-3.3-70B", type: "chat" },
        ]),
        { status: 200 },
      ),
    );

    await expect(
      listSttProviderModels(
        "together",
        "https://api.together.xyz/v1",
        "tg-key",
      ),
    ).resolves.toEqual([
      "openai/whisper-large-v3",
      "distil-whisper/distil-large-v3",
    ]);
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
