import { Check, CircleNotch } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

import { AuthShell, authStyles } from "@/components/auth-shell";
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
      <div {...stylex.props(styles.root)}>
        <div {...stylex.props(styles.client)}>
          <p {...stylex.props(styles.clientName)}>{details.client.name}</p>
          <p {...stylex.props(styles.clientUri)}>{details.client.uri}</p>
        </div>

        <div>
          <p {...stylex.props(styles.sectionTitle)}>This connection can:</p>
          <ul {...stylex.props(styles.permissions)}>
            {permissions.map((permission) => (
              <li key={permission} {...stylex.props(styles.permission)}>
                <Check aria-hidden {...stylex.props(styles.check)} />
                <span>{permission}</span>
              </li>
            ))}
          </ul>
        </div>

        <p {...stylex.props(styles.disclosure)}>
          The connector cannot edit or delete meetings. Disable Cloud API &amp;
          Connectors in Anarlog to remove its server-readable meeting copies.
        </p>

        {decision.isError && (
          <p role="alert" {...stylex.props(styles.error)}>
            The authorization request could not be completed. Try again.
          </p>
        )}

        <div {...stylex.props(styles.actions)}>
          <button
            type="button"
            {...stylex.props(authStyles.primaryButton)}
            disabled={decision.isPending}
            onClick={() => decision.mutate("approve")}
          >
            {decision.isPending && decision.variables === "approve" ? (
              <CircleNotch aria-hidden {...stylex.props(styles.spinner)} />
            ) : null}
            Allow access
          </button>
          <button
            type="button"
            {...stylex.props(authStyles.secondaryButton)}
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

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  actions: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
  },
  check: {
    color: colors.foreground,
    flexShrink: 0,
    height: "1rem",
    marginTop: "0.125rem",
    width: "1rem",
  },
  client: {
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    padding: "1rem",
  },
  clientName: {
    color: colors.foreground,
    fontSize: "0.875rem",
    fontWeight: 500,
  },
  clientUri: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    marginTop: "0.25rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  disclosure: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1.25rem",
  },
  error: {
    color: "#dc2626",
    fontSize: "0.875rem",
  },
  permission: {
    alignItems: "flex-start",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
    lineHeight: "1.25rem",
  },
  permissions: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
    marginTop: "0.75rem",
  },
  root: {
    display: "flex",
    flexDirection: "column",
    gap: "1.25rem",
  },
  sectionTitle: {
    color: colors.foreground,
    fontSize: "0.875rem",
    fontWeight: 500,
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "1rem",
    width: "1rem",
  },
});
