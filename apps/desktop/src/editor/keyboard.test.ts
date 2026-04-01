import { afterEach, describe, expect, it, vi } from "vitest";

import { isApplePlatform, isPlainArrowKey } from "./keyboard";

describe("editor keyboard utilities", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("detects Apple platforms", () => {
    const userAgentSpy = vi.spyOn(window.navigator, "userAgent", "get");
    userAgentSpy.mockReturnValue(
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/605.1.15",
    );
    expect(isApplePlatform()).toBe(true);

    userAgentSpy.mockReturnValue(
      "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15",
    );
    expect(isApplePlatform()).toBe(true);
  });

  it("rejects non-Apple platforms", () => {
    const userAgentSpy = vi.spyOn(window.navigator, "userAgent", "get");
    userAgentSpy.mockReturnValue("Mozilla/5.0 (Windows NT 10.0; Win64; x64)");
    expect(isApplePlatform()).toBe(false);

    userAgentSpy.mockReturnValue("Mozilla/5.0 (X11; Linux x86_64)");
    expect(isApplePlatform()).toBe(false);
  });

  it("matches only unmodified arrow keys", () => {
    expect(
      isPlainArrowKey(
        {
          key: "ArrowDown",
          altKey: false,
          ctrlKey: false,
          metaKey: false,
          shiftKey: false,
        },
        "ArrowDown",
      ),
    ).toBe(true);

    expect(
      isPlainArrowKey(
        {
          key: "ArrowDown",
          altKey: true,
          ctrlKey: false,
          metaKey: false,
          shiftKey: false,
        },
        "ArrowDown",
      ),
    ).toBe(false);

    expect(
      isPlainArrowKey(
        {
          key: "ArrowDown",
          altKey: false,
          ctrlKey: true,
          metaKey: false,
          shiftKey: false,
        },
        "ArrowDown",
      ),
    ).toBe(false);

    expect(
      isPlainArrowKey(
        {
          key: "ArrowDown",
          altKey: false,
          ctrlKey: false,
          metaKey: true,
          shiftKey: false,
        },
        "ArrowDown",
      ),
    ).toBe(false);

    expect(
      isPlainArrowKey(
        {
          key: "ArrowDown",
          altKey: false,
          ctrlKey: false,
          metaKey: false,
          shiftKey: true,
        },
        "ArrowDown",
      ),
    ).toBe(false);
  });
});
