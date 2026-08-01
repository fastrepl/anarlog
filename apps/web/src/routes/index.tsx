import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { HomePage } from "@/components/home-page";
import {
  DEFAULT_DESKTOP_SCHEME,
  desktopSchemeSchema,
} from "@/functions/desktop-flow";
import { getGitHubStats } from "@/functions/github";
import {
  ANARLOG_SITE_URL,
  ROOT_DESCRIPTION,
  getOrganizationJsonLd,
  getSoftwareApplicationJsonLd,
  getStructuredDataGraph,
} from "@/lib/seo";

const featureList = [
  "Bot-free meeting capture",
  "Fully offline notes",
  "On-device or bring-your-own-key AI",
  "Local-first storage",
  "Open source foundations",
];

const authCallbackSearchSchema = z.object({
  code: z.string().optional(),
  token_hash: z.string().optional(),
  type: z
    .enum([
      "email",
      "recovery",
      "magiclink",
      "signup",
      "invite",
      "email_change",
    ])
    .optional()
    .catch(undefined),
  flow: z.enum(["desktop", "web"]).optional().catch(undefined),
  scheme: desktopSchemeSchema.optional().catch(DEFAULT_DESKTOP_SCHEME),
  redirect: z.string().optional(),
  redirect_to: z.string().max(2048).optional(),
  error: z.string().optional(),
  error_description: z.string().optional(),
});

export const Route = createFileRoute("/")({
  validateSearch: authCallbackSearchSchema,
  beforeLoad: ({ search }) => {
    if (search.token_hash && search.type) {
      throw redirect({
        to: "/confirm-auth/",
        search: {
          token_hash: search.token_hash,
          type: search.type,
          flow: search.flow,
          scheme: search.scheme,
          redirect: search.redirect,
          redirect_to: search.redirect_to,
        },
      });
    }

    const hasAuthCallback = !!search.code || !!search.error;

    if (!hasAuthCallback) {
      return;
    }

    const flow = search.flow ?? "web";
    const scheme = search.scheme ?? DEFAULT_DESKTOP_SCHEME;

    throw redirect({
      to: "/callback/auth/",
      search: {
        flow,
        scheme,
        code: search.code,
        redirect: search.redirect,
        error: search.error,
        error_description: search.error_description,
      } as any,
    });
  },
  component: Component,
  loader: async () => {
    const githubStats = await getGitHubStats();

    return {
      githubStars: githubStats.stars ?? 8466,
    };
  },
  head: () => ({
    links: [{ rel: "canonical", href: ANARLOG_SITE_URL }],
    scripts: [
      {
        type: "application/ld+json",
        children: JSON.stringify(
          getStructuredDataGraph([
            getOrganizationJsonLd(),
            getSoftwareApplicationJsonLd({
              description: ROOT_DESCRIPTION,
              featureList,
            }),
          ]),
        ),
      },
    ],
  }),
});

function Component() {
  const { githubStars } = Route.useLoaderData();

  return (
    <HomePage formattedGithubStars={githubStars.toLocaleString("en-US")} />
  );
}
