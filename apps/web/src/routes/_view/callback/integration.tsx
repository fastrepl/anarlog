import { Check, Copy } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { z } from "zod";

import { AuthShell, authStyles } from "@/components/auth-shell";
import {
  DEFAULT_DESKTOP_SCHEME,
  flowSearchSchema,
} from "@/functions/desktop-flow";
const styles = stylex.create({
  style1: {
    display: "flex",
    flexDirection: "column",
    gap: ".75rem",
  },
  style2: {
    borderRadius: ".75rem",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: "#e5ddcf",
    backgroundColor: "#fbfaf7",
    padding: "1rem",
    textAlign: "center",
  },
  style3: {
    marginBottom: ".75rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style4: {
    width: "1rem",
    height: "1rem",
  },
  style5: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: "#4f4940",
  },
});
const commonSearch = {
  integration_id: z.string(),
  status: z.string(),
  disconnected_connection_id: z.string().optional(),
  return_to: z.string().optional(),
};
const validateSearch = flowSearchSchema(commonSearch, {
  defaultFlow: "desktop",
});
type IntegrationDeeplinkParams = {
  integration_id: string;
  status: string;
  disconnected_connection_id?: string;
  return_to?: string;
};
export const Route = createFileRoute("/_view/callback/integration")({
  validateSearch,
  component: Component,
  head: () => ({
    meta: [
      {
        name: "robots",
        content: "noindex, nofollow",
      },
    ],
  }),
});
function buildDeeplinkUrl(
  scheme: string,
  search: IntegrationDeeplinkParams,
): string {
  const params = new URLSearchParams({
    integration_id: search.integration_id,
    status: search.status,
  });
  if (search.disconnected_connection_id) {
    params.set("disconnected_connection_id", search.disconnected_connection_id);
  }
  if (search.return_to) {
    params.set("return_to", search.return_to);
  }
  return `${scheme}://integration/callback?${params.toString()}`;
}
function Component() {
  const search = Route.useSearch();
  const scheme = search.scheme ?? DEFAULT_DESKTOP_SCHEME;
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [copied, setCopied] = useState(false);
  const getDeeplink = () => {
    return buildDeeplinkUrl(scheme, {
      integration_id: search.integration_id,
      status: search.status,
      disconnected_connection_id: search.disconnected_connection_id,
      return_to: search.return_to,
    });
  };
  const handleDeeplink = () => {
    const deeplink = getDeeplink();
    if (search.flow === "desktop" && deeplink) {
      window.location.href = deeplink;
    }
  };
  const handleCopy = async () => {
    const deeplink = getDeeplink();
    if (deeplink) {
      await navigator.clipboard.writeText(deeplink);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };
  useEffect(() => {
    if (search.flow === "web") {
      void queryClient.invalidateQueries({
        predicate: (query) => query.queryKey[0] === "integration-status",
      });
      void navigate({
        to: "/app/account/",
        search: {
          tab: "connections",
        },
      } as any);
    }
  }, [search.flow, navigate, queryClient]);
  useEffect(() => {
    if (search.flow === "desktop" && search.status === "success") {
      const deeplink = getDeeplink();
      const timer = setTimeout(() => {
        window.location.href = deeplink;
      }, 250);
      return () => clearTimeout(timer);
    }
  }, [
    search.flow,
    search.status,
    scheme,
    search.integration_id,
    search.disconnected_connection_id,
    search.return_to,
  ]);
  const isSuccess = search.status === "success";
  if (search.flow === "desktop") {
    return (
      <AuthShell
        title={isSuccess ? "You’re connected" : "Connection didn’t work"}
        description={
          isSuccess
            ? "Return to Anarlog to keep going."
            : "Something went wrong while connecting."
        }
      >
        {isSuccess ? (
          <div {...stylex.props(styles.style1)}>
            <button
              onClick={handleDeeplink}
              {...stylex.props(authStyles.primaryButton)}
            >
              Open Anarlog
            </button>

            <div {...stylex.props(styles.style2)}>
              <p {...stylex.props(styles.style3)}>
                Button not working? Copy the link instead
              </p>
              <button
                onClick={handleCopy}
                {...stylex.props(authStyles.secondaryButton)}
              >
                {copied ? (
                  <>
                    <Check {...stylex.props(styles.style4)} />
                    Copied!
                  </>
                ) : (
                  <>
                    <Copy {...stylex.props(styles.style4)} />
                    Copy URL
                  </>
                )}
              </button>
            </div>
          </div>
        ) : (
          <div {...stylex.props(authStyles.notice)}>
            <p {...stylex.props(styles.style5)}>
              Close this window and try again from Anarlog.
            </p>
          </div>
        )}
      </AuthShell>
    );
  }
  if (search.flow === "web") {
    return <div>Redirecting...</div>;
  }
}
