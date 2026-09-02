import { describe, expect, it } from "vitest";

import {
  APP_FLOATING_CHROME_PADDING_PX,
  APP_FLOATING_CHROME_RADIUS_PX,
  APP_FLOATING_ITEM_RADIUS_PX,
  APP_FLOATING_PANEL_PADDING_PX,
  APP_FLOATING_PANEL_RADIUS_PX,
  FLOATING_MENU_ITEM_RADIUS_PX,
  FLOATING_MENU_PADDING_PX,
  FLOATING_MENU_RADIUS_PX,
  appFloatingContentClassName,
  appFloatingItemClassName,
  appFloatingMenuPanelClassName,
} from "@anlg/ui/components/ui/floating-content";

describe("floating content radii", () => {
  it("keeps app chrome, panel, and item corners concentric", () => {
    expect(APP_FLOATING_PANEL_RADIUS_PX).toBe(
      APP_FLOATING_CHROME_RADIUS_PX - APP_FLOATING_CHROME_PADDING_PX,
    );
    expect(APP_FLOATING_ITEM_RADIUS_PX).toBe(
      APP_FLOATING_PANEL_RADIUS_PX - APP_FLOATING_PANEL_PADDING_PX,
    );
    expect(appFloatingContentClassName).toContain(
      `rounded-[${APP_FLOATING_CHROME_RADIUS_PX}px]`,
    );
    expect(appFloatingContentClassName).toContain("p-0.5");
    expect(appFloatingMenuPanelClassName).toContain("p-1.5");
    expect(appFloatingItemClassName).toContain(
      `rounded-[${APP_FLOATING_ITEM_RADIUS_PX}px]`,
    );
  });

  it("keeps default menu and item corners concentric", () => {
    expect(FLOATING_MENU_ITEM_RADIUS_PX).toBe(
      FLOATING_MENU_RADIUS_PX - FLOATING_MENU_PADDING_PX,
    );
    expect(FLOATING_MENU_ITEM_RADIUS_PX).toBe(APP_FLOATING_ITEM_RADIUS_PX);
  });
});
