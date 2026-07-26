import type { ReactNode } from "react";

export type PluginStateValue = string | number | boolean | null;

export type PluginCleanup = () => void | Promise<void>;
export type PluginEventUnlisten = () => void | Promise<void>;
export type PluginEventRef = PluginEventUnlisten | Promise<PluginEventUnlisten>;

export type PluginContext<TEvents = unknown> = {
  registerView: (viewId: string, factory: () => ReactNode) => void;
  openTab: (
    extensionId?: string,
    state?: Partial<Record<string, PluginStateValue>>,
  ) => void;
  events: TEvents;
  registerEvent: (eventRef: PluginEventRef) => void;
  registerCleanup: (cleanup: PluginCleanup) => void;
};

export type PluginModule<TEvents = unknown> = {
  id: string;
  onload: (ctx: PluginContext<TEvents>) => void | Promise<void>;
  onunload?: () => void | Promise<void>;
};
