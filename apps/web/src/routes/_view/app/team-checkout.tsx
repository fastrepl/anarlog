import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { createTeamCheckoutSession } from "@/functions/billing";
import { desktopSchemeSchema } from "@/functions/desktop-flow";
import {
  addInternalReturnPathSearch,
  sanitizeInternalReturnPath,
} from "@/lib/auth-redirect";
import { captureOperationalError } from "@/lib/error-reporting";

const validateSearch = z.object({
  workspace_id: z.string().uuid(),
  period: z.enum(["monthly", "yearly"]).catch("monthly"),
  quantity: z.coerce.number().int().positive().max(999_999),
  scheme: desktopSchemeSchema.optional(),
  return_to: z.string().optional(),
});

export const Route = createFileRoute("/_view/app/team-checkout")({
  validateSearch,
  beforeLoad: async ({ search }) => {
    const returnTo = sanitizeInternalReturnPath(search.return_to);
    let url: string | null | undefined;
    try {
      ({ url } = await createTeamCheckoutSession({
        data: {
          workspaceId: search.workspace_id,
          period: search.period,
          quantity: search.quantity,
          scheme: search.scheme,
          returnTo,
        },
      }));
    } catch (error) {
      captureOperationalError(error, {
        operation: "team_checkout_session_create",
        context: {
          workspace_id: search.workspace_id,
          period: search.period,
        },
      });
    }

    if (url) {
      throw redirect({ href: url } as any);
    }

    if (search.scheme) {
      throw redirect({
        href: `/callback/billing?scheme=${search.scheme}&checkout=failed&checkout_type=paid`,
      } as any);
    }

    throw redirect({
      href: addInternalReturnPathSearch(returnTo, {
        checkout: "failed",
        checkout_type: "paid",
      }),
    } as any);
  },
});
