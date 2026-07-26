import type * as React from "react";

import type { PluginEventRef, PluginModule } from "@hypr/plugin-sdk";

type SessionLifecyclePayload = {
  type: string;
  session_id: string | null;
};

type PluginEvents = {
  tauri: {
    listener: {
      captureLifecycleEvent: {
        listen: (
          handler: (event: { payload: SessionLifecyclePayload }) => void,
        ) => PluginEventRef;
      };
    };
  };
};

declare global {
  interface Window {
    __char_react?: typeof React;
    __char_plugins?: {
      register: (plugin: PluginModule<PluginEvents>) => void;
    };
  }
}
