import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { createPortalSession } from "@/functions/billing";
import { desktopSchemeSchema } from "@/functions/desktop-flow";

const validateSearch = z.object({
  scheme: desktopSchemeSchema.optional(),
});

export const Route = createFileRoute("/_view/app/portal")({
  validateSearch,
  beforeLoad: async ({ search }) => {
    if (!search.scheme) {
      throw redirect({ to: "/download/" });
    }

    const { url } = await createPortalSession({
      data: { scheme: search.scheme },
    });

    if (url) {
      throw redirect({ href: url } as any);
    }

    throw redirect({ to: "/" });
  },
});
