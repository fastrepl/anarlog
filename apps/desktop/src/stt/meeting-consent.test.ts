import { describe, expect, it } from "vitest";

import { interpretChatAsConsentResponse } from "./meeting-consent";

const DISCLOSURE =
  "I'm using Anarlog to record and transcribe this meeting. https://anarlog.so";

describe("meeting consent model", () => {
  it("ignores the disclosure text itself when classifying chat replies", () => {
    expect(interpretChatAsConsentResponse(DISCLOSURE, DISCLOSURE)).toBeNull();
    expect(interpretChatAsConsentResponse("I do not consent", DISCLOSURE)).toBe(
      "declined",
    );
    expect(interpretChatAsConsentResponse("I consent", DISCLOSURE)).toBe(
      "consented",
    );
    expect(
      interpretChatAsConsentResponse("sounds good", DISCLOSURE),
    ).toBeNull();
  });
});
