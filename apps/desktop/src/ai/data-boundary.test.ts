import { describe, expect, it, vi } from "vitest";

import {
  canSendCalendarDerivedSttHints,
  isOnDeviceLlmConnection,
  isOnDeviceLiveSttConnection,
  isOnDeviceSttTarget,
  resolveGoogleCalendarLlmBoundary,
} from "./data-boundary";

describe("Google Calendar AI data boundary", () => {
  it.each([
    ["ollama", "http://127.0.0.1:11434/v1"],
    ["ollama", "http://localhost:11434/v1"],
    ["lmstudio", "http://[::1]:1234/v1"],
  ])("allows %s on a loopback endpoint", (providerId, baseUrl) => {
    expect(isOnDeviceLlmConnection({ providerId, baseUrl })).toBe(true);
    expect(
      resolveGoogleCalendarLlmBoundary({
        googleCalendarDataState: "present",
        providerId,
        baseUrl,
      }),
    ).toBe("allowed");
  });

  it.each([
    ["hyprnote", "http://localhost:3011/llm"],
    ["openrouter", "https://openrouter.ai/api/v1"],
    ["openai", "https://api.openai.com/v1"],
    ["custom", "http://localhost:9000/v1"],
    ["ollama", "https://models.example.com/v1"],
    ["lmstudio", "http://localhost.evil.example/v1"],
    ["ollama", "not-a-url"],
  ])("treats %s at %s as off-device", (providerId, baseUrl) => {
    expect(isOnDeviceLlmConnection({ providerId, baseUrl })).toBe(false);
    expect(
      resolveGoogleCalendarLlmBoundary({
        googleCalendarDataState: "present",
        providerId,
        baseUrl,
      }),
    ).toBe("blocked");
  });

  it("fails closed while the local Google data check is unresolved", () => {
    expect(
      resolveGoogleCalendarLlmBoundary({
        googleCalendarDataState: "loading",
        providerId: "openrouter",
        baseUrl: "https://openrouter.ai/api/v1",
      }),
    ).toBe("checking");
    expect(
      resolveGoogleCalendarLlmBoundary({
        googleCalendarDataState: "error",
        providerId: "openrouter",
        baseUrl: "https://openrouter.ai/api/v1",
      }),
    ).toBe("check_failed");
  });

  it("allows off-device models only when Google data is conclusively absent", () => {
    expect(
      resolveGoogleCalendarLlmBoundary({
        googleCalendarDataState: "absent",
        providerId: "openrouter",
        baseUrl: "https://openrouter.ai/api/v1",
      }),
    ).toBe("allowed");
  });

  it("recognizes only known local transcription targets", () => {
    expect(
      isOnDeviceSttTarget({ provider: "soniqo", baseUrl: "soniqo://local" }),
    ).toBe(true);
    expect(
      isOnDeviceSttTarget({
        provider: "am",
        baseUrl: "http://127.0.0.1:8080",
      }),
    ).toBe(true);
    expect(
      isOnDeviceSttTarget({
        provider: "hyprnote",
        baseUrl: "http://localhost:3011/stt",
      }),
    ).toBe(false);
    expect(
      isOnDeviceSttTarget({
        provider: "deepgram",
        baseUrl: "https://api.deepgram.com",
      }),
    ).toBe(false);
  });

  it("validates the resolved endpoint for live local STT", () => {
    expect(
      isOnDeviceLiveSttConnection({
        isLocalModel: true,
        baseUrl: "http://127.0.0.1:8080",
      }),
    ).toBe(true);
    expect(
      isOnDeviceLiveSttConnection({
        isLocalModel: true,
        baseUrl: "https://remote.example.com",
      }),
    ).toBe(false);
    expect(
      isOnDeviceLiveSttConnection({
        isLocalModel: true,
        baseUrl: undefined,
      }),
    ).toBe(false);
  });

  it("rechecks Google data immediately before off-device STT", async () => {
    await expect(
      canSendCalendarDerivedSttHints({
        targetIsOnDevice: false,
        googleCalendarDataState: "absent",
        checkHasGoogleCalendarData: async () => true,
      }),
    ).resolves.toBe(false);
    await expect(
      canSendCalendarDerivedSttHints({
        targetIsOnDevice: false,
        googleCalendarDataState: "absent",
        checkHasGoogleCalendarData: async () => false,
      }),
    ).resolves.toBe(true);
  });

  it("fails closed when the request-time STT data check fails", async () => {
    await expect(
      canSendCalendarDerivedSttHints({
        targetIsOnDevice: false,
        googleCalendarDataState: "absent",
        checkHasGoogleCalendarData: async () => {
          throw new Error("database unavailable");
        },
      }),
    ).resolves.toBe(false);
  });

  it("does not query Google data for on-device STT", async () => {
    const checkHasGoogleCalendarData = vi.fn(async () => true);

    await expect(
      canSendCalendarDerivedSttHints({
        targetIsOnDevice: true,
        googleCalendarDataState: "present",
        checkHasGoogleCalendarData,
      }),
    ).resolves.toBe(true);
    expect(checkHasGoogleCalendarData).not.toHaveBeenCalled();
  });
});
