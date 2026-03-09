import { pluginEvents } from "./events";
import { registerPluginView } from "./registry";
import type { PluginCleanup, PluginContext } from "./types";

import { useTabs } from "~/store/zustand/tabs";

export function createPluginContext(
  pluginId: string,
  addCleanup: (cleanup: PluginCleanup) => void,
): PluginContext {
  return {
    registerView: (viewId, factory) => {
      registerPluginView(pluginId, viewId, factory);
    },
    openTab: (targetPluginId = pluginId, state) => {
      useTabs.getState().openNew({
        type: "extension",
        extensionId: targetPluginId,
        state: state ?? {},
      });
    },
    events: pluginEvents,
    registerEvent: (eventRef) => {
      addCleanup(async () => {
        const unlisten = await eventRef;
        unlisten();
      });
    },
    registerCleanup: addCleanup,
  };
}
