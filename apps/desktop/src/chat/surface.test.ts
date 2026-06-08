import { describe, expect, it } from "vitest";

import {
  chatElevatedSurfaceClassNames,
  chatFloatingControlClassNames,
  chatFloatingPanelShellClassNames,
  chatInputEditorClassNames,
  chatPanelClassNames,
  chatSendButtonDisabledClassNames,
  chatToolbarSurface,
  isChatDarkAppearance,
} from "./surface";

describe("chat surface tokens", () => {
  it("always uses the dark stone chat chrome regardless of app theme", () => {
    expect(isChatDarkAppearance()).toBe(true);
    expect(chatToolbarSurface()).toBe("dark");
  });

  it("maps the original stone-800 shell to primary tokens", () => {
    expect(chatPanelClassNames()).toContain("bg-primary");
    expect(chatPanelClassNames()).toContain("text-primary-foreground");
    expect(chatPanelClassNames()).not.toContain("bg-card");
  });

  it("maps elevated chat surfaces to dark accent tokens", () => {
    expect(chatElevatedSurfaceClassNames()).toContain("bg-accent");
    expect(chatElevatedSurfaceClassNames()).toContain("text-accent-foreground");
    expect(chatElevatedSurfaceClassNames()).toContain("border-border");
    expect(chatInputEditorClassNames()).toContain("text-accent-foreground");
    expect(chatInputEditorClassNames()).toContain("chat-input-editor");
  });

  it("uses elevated controls on the dark chat panel", () => {
    expect(chatFloatingControlClassNames()).toContain("bg-accent");
    expect(chatFloatingControlClassNames()).toContain("text-accent-foreground");
  });

  it("uses a dark drop shadow on the floating shell", () => {
    expect(chatFloatingPanelShellClassNames()).toContain(
      "shadow-[0_16px_48px_rgba(0,0,0,0.55)]",
    );
    expect(chatFloatingPanelShellClassNames()).toContain("border-stone-600");
    expect(chatFloatingPanelShellClassNames()).toContain("bg-primary");
  });

  it("styles disabled send controls on the elevated input surface", () => {
    expect(chatSendButtonDisabledClassNames()).toContain(
      "text-muted-foreground/60",
    );
    expect(chatSendButtonDisabledClassNames()).toContain("border-border");
  });
});
