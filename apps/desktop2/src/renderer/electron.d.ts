/// <reference types="vite/client" />
import type { HyprElectronApi } from "../shared/api";

export type HyprPlatform = {
  os: NodeJS.Platform;
};

declare global {
  interface Window {
    hypr: HyprElectronApi;
    hyprPlatform: HyprPlatform;
  }
}

export {};
