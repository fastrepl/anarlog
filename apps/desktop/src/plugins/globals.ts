import * as React from "react";

import { registerPluginModule } from "./registry";
import type { PluginModule } from "./types";

declare global {
  interface Window {
    __char_react?: typeof React;
    __char_plugins?: {
      register: (plugin: PluginModule) => void;
    };
  }
}

export function initPluginGlobals() {
  window.__char_react = React;
  window.__char_plugins = {
    register(plugin) {
      registerPluginModule(plugin);
    },
  };
}
