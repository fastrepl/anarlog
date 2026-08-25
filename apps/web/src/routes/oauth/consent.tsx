import { Check, CircleNotch } from "@phosphor-icons/react";
import { useMutation } from "@tanstack/react-query";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import {
  AuthShell,
  authPrimaryButtonClassName,
  authSecondaryButtonClassName,
} from "@/components/auth-shell";
import { fetchUser } from "@/functions/auth";
import {
  decideOAuthAuthorization,
  getOAuthAuthorizationDetails,
} from "@/functions/oauth-consent";
import {
  describeOAuthScopes,
  oauthAuthorizationIdSchema,
} from "@/lib/oauth-consent";

const searchSchema = z.object({
  authorization_id: oauthAuthorizationIdSchema,
});

export const Route = createFileRoute("/oauth/consent")({
  validateSearch: searchSchema,
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
  loaderDeps: ({ search }) => ({
    authorizationId: search.authorization_id,
  }),
  loader: async ({ deps: { authorizationId } }) => {
    const user = await fetchUser();
    if (!user) {
      const search = new URLSearchParams({ authorization_id: authorizationId });
      throw redirect({
        to: "/auth/",
        search: {
          flow: "web",
          redirect: `/oauth/consent?${search.toString()}`,
        },
      });
    }

    const details = await getOAuthAuthorizationDetails({
      data: { authorizationId },
    });
    if (details.status === "redirect") {
      throw redirect({ href: details.redirectUrl } as any);
    }
    return details;
  },
  component: OAuthConsent,
});

function OAuthConsent() {
  const details = Route.useLoaderData();
  const permissions = [
    "Read the Anarlog meetings you explicitly uploaded for connectors",
    ...describeOAuthScopes(details.scope),
  ];
  const decision = useMutation({
    mutationFn: (value: "approve" | "deny") =>
      decideOAuthAuthorization({
        data: {
          authorizationId: details.authorizationId,
          decision: value,
        },
      }),
    onSuccess: ({ redirectUrl }) => window.location.assign(redirectUrl),
  });

  return (
    <AuthShell
      title="Connect Anarlog"
      description={`${details.client.name} is requesting read-only access to your opted-in Anarlog meeting data.`}
    >
      <div className="flex flex-col gap-5">
        <div className="surface-subtle border-color-subtle rounded-xl border p-4">
          <p className="text-color text-sm font-medium">
            {details.client.name}
          </p>
          <p className="text-color-muted mt-1 truncate text-xs">
            {details.client.uri}
          </p>
        </div>

        <div>
          <p className="text-color text-sm font-medium">This connection can:</p>
          <ul className="mt-3 flex flex-col gap-3">
            {permissions.map((permission) => (
              <li
                key={permission}
                className="text-color-muted flex items-start gap-2 text-sm leading-5"
              >
                <Check className="text-color mt-0.5 size-4 shrink-0" />
                <span>{permission}</span>
              </li>
            ))}
          </ul>
        </div>

        <p className="text-color-muted text-xs leading-5">
          The connector cannot edit or delete meetings. Disable Cloud API &amp;
          Connectors in Anarlog to remove its server-readable meeting copies.
        </p>

        {decision.isError && (
          <p className="text-sm text-red-600">
            The authorization request could not be completed. Try again.
          </p>
        )}

        <div className="flex flex-col gap-3">
          <button
            type="button"
            className={authPrimaryButtonClassName}
            disabled={decision.isPending}
            onClick={() => decision.mutate("approve")}
          >
            {decision.isPending && decision.variables === "approve" ? (
              <CircleNotch className="size-4 animate-spin" />
            ) : null}
            Allow access
          </button>
          <button
            type="button"
            className={authSecondaryButtonClassName}
            disabled={decision.isPending}
            onClick={() => decision.mutate("deny")}
          >
            Deny
          </button>
        </div>
      </div>
    </AuthShell>
  );
}
