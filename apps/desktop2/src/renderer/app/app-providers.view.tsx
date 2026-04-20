import type { ReactNode } from "react";

/**
 * Single provider stack for the renderer, wrapped around the shell + any
 * route-level content.
 *
 * Analogue of `apps/desktop/src/main2/layout.tsx#Main2Layout`. Today it is a
 * passthrough because the desktop2 slice deliberately does not port the Tauri
 * app's context graph (no TinyBase, no search engine, no AI task store, no
 * notification channel). Providers that the Tauri shell composes through
 * `Main2Layout` land here when their underlying services port over, in this
 * order:
 *
 *   NotificationProvider       // port of ~/contexts/notifications
 *   SearchEngineProvider       // once search indexing is wired
 *   SearchUIProvider           // overlay / shortcut state
 *   ShellProvider              // split-pane + chat + sidebar state
 *   ToolRegistryProvider       // AI tool registry, once ported
 *   AITaskProvider             // AI task queue, once ported
 *
 * The boundary rules of `apps/desktop2/AGENTS.md` still apply: nothing here
 * may import from `src/main/**` or `src/preload/**`. Providers must talk to
 * the Electron main process exclusively through `~/bridge` (which wraps
 * `window.hypr`, typed by `src/shared/api.ts`).
 */
export function AppProvidersView({ children }: { children: ReactNode }) {
  return <>{children}</>;
}
