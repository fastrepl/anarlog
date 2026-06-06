import { describe, expect, it } from "vitest";

import {
  chatElevatedSurfaceClassNames,
  chatInputEditorClassNames,
  chatPanelClassNames,
  chatToolbarSurface,
  isChatDarkAppearance,
} from "./surface";

describe("chat surface tokens", () => {
  it("follows the saved app appearance", () => {
    expect(isChatDarkAppearance("dark", false)).toBe(true);
    expect(isChatDarkAppearance("light", true)).toBe(false);
    expect(isChatDarkAppearance("system", true)).toBe(true);
    expect(isChatDarkAppearance("system", false)).toBe(false);
  });

  it("uses a light card shell in light appearance", () => {
    expect(chatPanelClassNames(false)).toContain("bg-card");
    expect(chatPanelClassNames(false)).toContain("text-foreground");
    expect(chatToolbarSurface(false)).toBe("light");
  });

  it("uses elevated light surfaces with dark text in dark appearance", () => {
    expect(chatElevatedSurfaceClassNames(true)).toContain(
      "bg-primary-foreground/95",
    );
    expect(chatElevatedSurfaceClassNames(true)).toContain("text-primary");
    expect(chatInputEditorClassNames(true)).toBe("text-primary");
  });
});
