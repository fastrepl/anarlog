import { describe, expect, test } from "vitest";

import { getPreferredProviderModel } from "./selection";

describe("getPreferredProviderModel", () => {
  test("returns the remembered model when it is still available", () => {
    expect(
      getPreferredProviderModel("deepgram", { deepgram: "nova-2-meeting" }, [
        { id: "nova-3-general" },
        { id: "nova-2-meeting" },
      ]),
    ).toBe("nova-2-meeting");
  });

  test("falls back to the first available model when none is remembered", () => {
    expect(
      getPreferredProviderModel("soniox", {}, [
        { id: "stt-v4" },
        { id: "stt-v3" },
      ]),
    ).toBe("stt-v4");
  });

  test("falls back to the first available model when the remembered model is gone", () => {
    expect(
      getPreferredProviderModel("deepgram", { deepgram: "nova-2-meeting" }, [
        { id: "nova-3-general" },
        { id: "nova-2-general" },
      ]),
    ).toBe("nova-3-general");
  });

  test("keeps the remembered value when the provider does not expose a static list", () => {
    expect(
      getPreferredProviderModel("custom", { custom: "whisper-large-v3" }, []),
    ).toBe("whisper-large-v3");
  });
});
