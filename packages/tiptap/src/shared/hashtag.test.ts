import { describe, expect, it } from "vitest";

import { findHashtags } from "./hashtag";

describe("findHashtags", () => {
  it("extracts regular hashtags", () => {
    expect(findHashtags("#alpha #beta").map((match) => match.tag)).toEqual([
      "alpha",
      "beta",
    ]);
  });

  it("ignores url fragments", () => {
    expect(
      findHashtags("https://web4.ai/#free #valid").map((match) => match.tag),
    ).toEqual(["valid"]);
  });

  it("ignores www url fragments", () => {
    expect(
      findHashtags("www.web4.ai/#free #valid").map((match) => match.tag),
    ).toEqual(["valid"]);
  });
});
