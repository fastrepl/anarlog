import { useNavigate } from "@tanstack/react-router";
import { useCallback } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as windowsCommands } from "@hypr/plugin-windows";

export function useOAuthFlow() {
  const navigate = useNavigate();

  const start = useCallback(
    async (opts: { url: string; title: string; description: string }) => {
      await windowsCommands.windowSaveFrame({ type: "main" });
      await windowsCommands.windowSetFrameAnimated(
        { type: "main" },
        "TopRight",
        340,
        500,
      );
      await navigate({
        to: "/app/browser-pending",
        search: { title: opts.title, description: opts.description },
      });
      await openerCommands.openUrl(opts.url, null);
    },
    [navigate],
  );

  return { start };
}
