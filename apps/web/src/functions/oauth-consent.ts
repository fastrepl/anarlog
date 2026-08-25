import { createServerFn } from "@tanstack/react-start";
import { z } from "zod";

import { getSupabaseServerClient } from "@/functions/supabase";
import { oauthAuthorizationIdSchema } from "@/lib/oauth-consent";

const authorizationInput = z.object({
  authorizationId: oauthAuthorizationIdSchema,
});

export const getOAuthAuthorizationDetails = createServerFn({ method: "GET" })
  .inputValidator(authorizationInput)
  .handler(async ({ data: { authorizationId } }) => {
    const supabase = getSupabaseServerClient();
    const { data, error } =
      await supabase.auth.oauth.getAuthorizationDetails(authorizationId);
    if (error || !data) {
      throw new Error("Unable to load this OAuth authorization request.");
    }
    if ("redirect_url" in data) {
      return { status: "redirect" as const, redirectUrl: data.redirect_url };
    }
    return {
      status: "consent" as const,
      authorizationId: data.authorization_id,
      client: {
        name: data.client.name,
        uri: data.client.uri,
      },
      scope: data.scope,
    };
  });

export const decideOAuthAuthorization = createServerFn({ method: "POST" })
  .inputValidator(
    authorizationInput.extend({
      decision: z.enum(["approve", "deny"]),
    }),
  )
  .handler(async ({ data: { authorizationId, decision } }) => {
    const supabase = getSupabaseServerClient();
    const { data, error } = await (decision === "approve"
      ? supabase.auth.oauth.approveAuthorization(authorizationId, {
          skipBrowserRedirect: true,
        })
      : supabase.auth.oauth.denyAuthorization(authorizationId, {
          skipBrowserRedirect: true,
        }));
    if (error || !data) {
      throw new Error("Unable to complete this OAuth authorization request.");
    }
    return { redirectUrl: data.redirect_url };
  });
