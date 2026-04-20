import { Menu, Tray, app, nativeImage } from "electron";

import { resourcePath } from "./paths.js";

type TrayHandles = {
  tray: Tray;
  destroy: () => void;
};

type TrayDeps = {
  onOpen: () => void;
};

// Mirrors the minimum useful surface of `plugins/tray` (Open + Quit) using the
// same template icon (`plugins/tray/icons/tray_default.png`). Ported as
// infrastructure only — this is enough to prove Electron can host the tray
// lifecycle Tauri currently owns, without re-porting the full menu graph.
export function createTray(deps: TrayDeps): TrayHandles | null {
  const image = nativeImage.createFromPath(
    resourcePath("plugins/tray/icons/tray_default.png"),
  );

  if (image.isEmpty()) {
    console.warn("[desktop2] tray icon missing, skipping tray setup");
    return null;
  }

  // Template images render correctly in both light/dark menu bars on macOS.
  if (process.platform === "darwin") {
    image.setTemplateImage(true);
  }

  const tray = new Tray(image);
  tray.setToolTip(app.getName());

  const menu = Menu.buildFromTemplate([
    {
      label: `Open ${app.getName()}`,
      click: () => deps.onOpen(),
    },
    { type: "separator" },
    {
      label: "Quit",
      accelerator: "CommandOrControl+Q",
      click: () => app.exit(0),
    },
  ]);
  tray.setContextMenu(menu);

  // Single click surfaces the window on every platform; on macOS we also make
  // the tray button behave like a regular menu instead of a toggle popover.
  tray.on("click", () => deps.onOpen());

  return {
    tray,
    destroy: () => tray.destroy(),
  };
}
