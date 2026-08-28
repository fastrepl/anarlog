import * as stylex from "@stylexjs/stylex";
import { describe, expect, it } from "vitest";

import {
  chatElevatedSurfaceStyle,
  chatFloatingControlStyle,
  chatFloatingPanelShellStyle,
  chatFloatingPanelStyle,
  chatInputEditorStyle,
  chatPanelBorderStyle,
  chatPanelStyle,
  chatSendButtonDisabledStyle,
  chatSendButtonShortcutDisabledStyle,
  chatToolbarSurface,
  isChatDarkAppearance,
} from "./surface";

describe("chat surface tokens", () => {
  it("uses the app chrome appearance instead of the forced dark chat chrome", () => {
    expect(isChatDarkAppearance()).toBe(false);
    expect(chatToolbarSurface()).toBe("light");
  });

  it("exposes every chat surface through StyleX", () => {
    const surfaceStyles = [
      chatPanelStyle,
      chatFloatingPanelStyle,
      chatPanelBorderStyle,
      chatFloatingPanelShellStyle,
      chatElevatedSurfaceStyle,
      chatInputEditorStyle,
      chatSendButtonDisabledStyle,
      chatSendButtonShortcutDisabledStyle,
      chatFloatingControlStyle,
    ];

    for (const surfaceStyle of surfaceStyles) {
      expect(stylex.props(surfaceStyle).className).toBeTruthy();
    }
  });
});
