import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useCallback } from "react";

import { commands as windowsCommands } from "@hypr/plugin-windows";

export const Route = createFileRoute("/app/browser-pending")({
  validateSearch: (search): { title: string; description: string } => ({
    title: String((search as { title?: string }).title ?? ""),
    description: String((search as { description?: string }).description ?? ""),
  }),
  component: BrowserPendingRoute,
});

function BrowserPendingRoute() {
  const { title, description } = Route.useSearch();
  const navigate = useNavigate();

  const handleCancel = useCallback(async () => {
    await windowsCommands.windowRestoreFrameAnimated({ type: "main" });
    await navigate({ to: "/app/main" });
  }, [navigate]);

  return (
    <div
      data-tauri-drag-region
      className="flex h-full flex-col items-center justify-center gap-6 p-8 select-none"
    >
      <img
        src="/assets/char-logo-icon-black.svg"
        alt=""
        className="h-10 w-10"
      />

      <div className="flex flex-col items-center gap-2 text-center">
        <h2 className="font-serif text-lg font-semibold">{title}</h2>
        <p className="text-sm text-neutral-500">{description}</p>
      </div>

      <div className="flex items-center gap-2">
        <div className="h-1.5 w-1.5 animate-bounce rounded-full bg-neutral-400 [animation-delay:-0.3s]" />
        <div className="h-1.5 w-1.5 animate-bounce rounded-full bg-neutral-400 [animation-delay:-0.15s]" />
        <div className="h-1.5 w-1.5 animate-bounce rounded-full bg-neutral-400" />
      </div>

      <button
        type="button"
        onClick={() => void handleCancel()}
        className="text-xs text-neutral-400 underline-offset-2 transition-colors hover:text-neutral-600 hover:underline"
      >
        Cancel
      </button>
    </div>
  );
}
