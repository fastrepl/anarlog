import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getStoredSettingValues: vi.fn(),
  play: vi.fn(),
}));

vi.mock("cuelume", () => ({
  play: mocks.play,
}));

vi.mock("~/settings/queries", () => ({
  getStoredSettingValues: mocks.getStoredSettingValues,
}));

import {
  COMPLETION_SOUND_NAMES,
  normalizeCompletionSoundName,
  playCompletionSound,
  previewCompletionSound,
} from "./completion-sound";

function stored(values: Record<string, boolean | string> = {}) {
  return {
    values,
    hasValues: new Set(Object.keys(values)),
  };
}

describe("completion sounds", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getStoredSettingValues.mockResolvedValue(stored());
  });

  it("offers the five curated completion sounds", () => {
    expect(COMPLETION_SOUND_NAMES).toEqual([
      "ready",
      "success",
      "chime",
      "sparkle",
      "bloom",
    ]);
  });

  it("uses Ready by default", async () => {
    await playCompletionSound();

    expect(mocks.play).toHaveBeenCalledWith("ready", { volume: 0.7 });
  });

  it("plays the selected completion sound", async () => {
    mocks.getStoredSettingValues.mockResolvedValue(
      stored({ notification_completion_sound_name: "sparkle" }),
    );

    await playCompletionSound();

    expect(mocks.play).toHaveBeenCalledWith("sparkle", { volume: 0.7 });
  });

  it("falls back to Ready for an unknown stored sound", () => {
    expect(normalizeCompletionSoundName("unknown")).toBe("ready");
  });

  const disabledSettings: Record<string, boolean>[] = [
    { notification_disabled: true },
    { notification_completion_sound: false },
  ];

  it.each(disabledSettings)("does not play when disabled", async (settings) => {
    mocks.getStoredSettingValues.mockResolvedValue(stored(settings));

    await playCompletionSound();

    expect(mocks.play).not.toHaveBeenCalled();
  });

  it("previews a sound without reading preferences", () => {
    previewCompletionSound("bloom");

    expect(mocks.play).toHaveBeenCalledWith("bloom", { volume: 0.7 });
    expect(mocks.getStoredSettingValues).not.toHaveBeenCalled();
  });
});
