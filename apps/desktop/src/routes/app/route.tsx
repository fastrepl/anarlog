import { createFileRoute, Outlet } from "@tanstack/react-router";

import { TooltipProvider } from "@hypr/ui/components/ui/tooltip";

import { ListenerProvider } from "~/stt/contexts";

export const Route = createFileRoute("/app")({
  component: Component,
  loader: async ({ context: { listenerStore } }) => {
    return { listenerStore: listenerStore! };
  },
});

function Component() {
  const { listenerStore } = Route.useLoaderData();

  return (
    <TooltipProvider>
      <ListenerProvider store={listenerStore}>
        <Outlet />
      </ListenerProvider>
    </TooltipProvider>
  );
}
