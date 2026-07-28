import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { createPortalSession } from "@/functions/billing";
import { desktopSchemeSchema } from "@/functions/desktop-flow";
import { captureOperationalError } from "@/lib/error-reporting";

const validateSearch = z.object({
  scheme: desktopSchemeSchema.optional(),
});

export const Route = createFileRoute("/_view/app/portal")({
  validateSearch,
  beforeLoad: async ({ search }) => {
    let url: string | null | undefined;
    try {
      ({ url } = await createPortalSession({
        data: { scheme: search.scheme },
      }));
    } catch (e) {
      captureOperationalError(e, {
        operation: "billing_portal_session_create",
      });
    }

    if (url) {
      throw redirect({ href: url } as any);
    }

    throw redirect({ to: "/app/account/" });
  },
});
