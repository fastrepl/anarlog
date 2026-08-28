import { describe, expect, it } from "vitest";

import {
  PROVIDER_BRAND_ICONS,
  providerIconOpticalScale,
  providerIconSrc,
} from "./icons";

describe("providerIconSrc", () => {
  it("uses the official Meet camera instead of a letter fallback", () => {
    expect(providerIconSrc({ id: "google-meet" })).toBe(
      PROVIDER_BRAND_ICONS["google-meet"],
    );
  });

  it("uses the Zoom camera app icon instead of a native wordmark", () => {
    expect(
      providerIconSrc({
        id: "zoom",
        iconUrl: "data:image/png;base64,zoom-wordmark",
      }),
    ).toBe(PROVIDER_BRAND_ICONS.zoom);
  });

  it("keeps native icons for apps that already ship official marks", () => {
    expect(
      providerIconSrc({
        id: "granola",
        iconUrl: "data:image/png;base64,granola",
      }),
    ).toBe("data:image/png;base64,granola");
    expect(
      providerIconSrc({
        id: "chatgpt-record",
        iconUrl: "data:image/png;base64,chatgpt",
      }),
    ).toBe("data:image/png;base64,chatgpt");
  });

  it("falls back to brand marks when a known app has no native icon", () => {
    expect(providerIconSrc({ id: "chatgpt-record" })).toBe(
      PROVIDER_BRAND_ICONS["chatgpt-record"],
    );
    expect(providerIconSrc({ id: "slack-huddles" })).toBe(
      PROVIDER_BRAND_ICONS["slack-huddles"],
    );
    expect(providerIconSrc({ id: "microsoft-teams" })).toBe(
      PROVIDER_BRAND_ICONS["microsoft-teams"],
    );
  });

  it("leaves unknown apps without an icon source", () => {
    expect(providerIconSrc({ id: "circleback" })).toBeUndefined();
  });
});

describe("providerIconOpticalScale", () => {
  it("enlarges line-art brand marks so they match filled app icons", () => {
    expect(providerIconOpticalScale({ id: "chatgpt-record" })).toBe(1.22);
    expect(providerIconOpticalScale({ id: "slack-huddles" })).toBe(1.12);
  });

  it("leaves filled brand marks and native app icons unscaled", () => {
    expect(providerIconOpticalScale({ id: "google-meet" })).toBeUndefined();
    expect(
      providerIconOpticalScale({
        id: "zoom",
        iconUrl: "data:image/png;base64,zoom-wordmark",
      }),
    ).toBeUndefined();
    expect(
      providerIconOpticalScale({
        id: "granola",
        iconUrl: "data:image/png;base64,granola",
      }),
    ).toBeUndefined();
    expect(
      providerIconOpticalScale({
        id: "chatgpt-record",
        iconUrl: "data:image/png;base64,chatgpt",
      }),
    ).toBeUndefined();
  });
});
