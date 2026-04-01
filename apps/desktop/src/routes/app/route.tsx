import { createFileRoute, Outlet } from "@tanstack/react-router";

import { TooltipProvider } from "@hypr/ui/components/ui/tooltip";

import { useDeeplinkHandler } from "~/shared/hooks/useDeeplinkHandler";
import { ListenerProvider } from "~/stt/contexts";

export const Route = createFileRoute("/app")({
  component: Component,
  loader: async ({ context: { listenerStore } }) => {
    return { listenerStore: listenerStore! };
  },
});

function Component() {
  const { listenerStore } = Route.useLoaderData();

  useDeeplinkHandler();

  return (
    <TooltipProvider>
      <ListenerProvider store={listenerStore}>
        <Outlet />
      </ListenerProvider>
    </TooltipProvider>
  );
}
