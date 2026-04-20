// Ported from `apps/desktop/src-tauri/src/embedded_cli.rs`. Both the Electron
// main process (which computes this state machine) and the renderer's
// settings panel consume the same shape.
export type EmbeddedCliState =
  | "installed"
  | "missing"
  | "conflict"
  | "unsupported"
  | "resource_missing";

export interface EmbeddedCliStatus {
  supported: boolean;
  commandName: string;
  installPath: string;
  resourcePath: string | null;
  state: EmbeddedCliState;
  details: string | null;
}
