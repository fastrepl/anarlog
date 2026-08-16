import { describe, expect, it } from "vitest";

import { addNangoSessionHandoff } from "./integration-handoff";

describe("addNangoSessionHandoff", () => {
  it("keeps the desktop flow parameters and puts the scoped token in the fragment", () => {
    const result = new URL(
      addNangoSessionHandoff(
        "https://anarlog.so/app/integration?flow=desktop&action=connect&integration_id=google-calendar",
        "nango.token+value",
      ),
    );

    expect(result.searchParams.get("flow")).toBe("desktop");
    expect(result.searchParams.get("action")).toBe("connect");
    expect(result.searchParams.get("handoff")).toBe("nango");
    expect(result.searchParams.has("session_token")).toBe(false);
    expect(new URLSearchParams(result.hash.slice(1)).get("session_token")).toBe(
      "nango.token+value",
    );
  });
});
