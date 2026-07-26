import { createFileRoute, redirect } from "@tanstack/react-router";

import { resolveAppEntryPath } from "./-resolve-entry-path";

export const Route = createFileRoute("/app/")({
  beforeLoad: async () => {
    throw redirect({ to: await resolveAppEntryPath() });
  },
  component: () => null,
});
