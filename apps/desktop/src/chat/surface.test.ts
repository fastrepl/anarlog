import { describe, expect, it } from "vitest";

import {
  chatElevatedSurfaceClassNames,
  chatFloatingPanelClassNames,
  chatFloatingControlClassNames,
  chatFloatingPanelShellClassNames,
  chatInputEditorClassNames,
  chatPanelBorderClassNames,
  chatPanelClassNames,
  chatSendButtonDisabledClassNames,
  chatToolbarSurface,
  isChatDarkAppearance,
} from "./surface";

describe("chat surface tokens", () => {
  it("uses the app chrome appearance instead of the forced dark chat chrome", () => {
    expect(isChatDarkAppearance()).toBe(false);
    expect(chatToolbarSurface()).toBe("light");
  });

  it("matches the main sidebar card surface", () => {
    expect(chatPanelClassNames()).toContain("bg-card");
    expect(chatPanelClassNames()).toContain("text-card-foreground");
    expect(chatPanelClassNames()).not.toContain("bg-primary");
    expect(chatPanelBorderClassNames()).toContain("border-border");
  });

  it("maps elevated chat surfaces to the card theme", () => {
    expect(chatElevatedSurfaceClassNames()).toContain("bg-card");
    expect(chatElevatedSurfaceClassNames()).toContain("text-card-foreground");
    expect(chatElevatedSurfaceClassNames()).toContain("border-border");
    expect(chatInputEditorClassNames()).toContain("text-card-foreground");
    expect(chatInputEditorClassNames()).toContain("chat-input-editor");
  });

  it("uses elevated controls on the chat panel", () => {
    expect(chatFloatingControlClassNames()).toContain("bg-accent");
    expect(chatFloatingControlClassNames()).toContain("text-accent-foreground");
  });

  it("uses a neutral surface on the floating shell", () => {
    expect(chatFloatingPanelShellClassNames()).toContain(
      "shadow-[0_16px_48px_rgba(0,0,0,0.18)]",
    );
    expect(chatFloatingPanelShellClassNames()).toContain(
      "dark:shadow-[0_16px_48px_rgba(0,0,0,0.55)]",
    );
    expect(chatFloatingPanelShellClassNames()).toContain("bg-[#f4f4f5]");
    expect(chatFloatingPanelShellClassNames()).toContain("border-[#dedede]");
    expect(chatFloatingPanelShellClassNames()).toContain("dark:bg-[#202020]");
    expect(chatFloatingPanelShellClassNames()).toContain(
      "dark:border-[#3a3a3a]",
    );
    expect(chatFloatingPanelShellClassNames()).not.toContain("bg-card");
  });

  it("uses a neutral floating panel surface to separate it from white notes", () => {
    expect(chatFloatingPanelClassNames()).toContain("bg-[#f4f4f5]");
    expect(chatFloatingPanelClassNames()).toContain("text-card-foreground");
    expect(chatFloatingPanelClassNames()).toContain("dark:bg-[#202020]");
    expect(chatFloatingPanelClassNames()).not.toContain("bg-card");
  });

  it("styles disabled send controls on the elevated input surface", () => {
    expect(chatSendButtonDisabledClassNames()).toContain(
      "text-muted-foreground/60",
    );
    expect(chatSendButtonDisabledClassNames()).toContain("border-border");
  });
});
