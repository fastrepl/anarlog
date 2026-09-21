import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { z } from "zod";

import { Check, Copy } from "@anlg/ui/components/icons";

import {
  AuthShell,
  authNoticeClassName,
  authPrimaryButtonClassName,
  authSecondaryButtonClassName,
} from "@/components/auth-shell";
import {
  DEFAULT_DESKTOP_SCHEME,
  flowSearchSchema,
} from "@/functions/desktop-flow";
import { integrationCallbackCopy } from "@/lib/integration-callback-copy";
import {
  callbackPortSchema,
  returnToDesktop,
} from "@/lib/integration-desktop-return";

const commonSearch = {
  callback_port: callbackPortSchema,
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
    meta: [{ name: "robots", content: "noindex, nofollow" }],
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

  const handoff = useMutation({
    mutationFn: () => returnToDesktop(getDeeplink(), search.callback_port),
  });
  const handleDeeplink = () => handoff.mutate();

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
        search: { tab: "connections" },
      } as any);
    }
  }, [search.flow, navigate, queryClient]);

  useEffect(() => {
    if (search.flow === "desktop" && search.status === "success") {
      const timer = setTimeout(() => {
        handoff.mutate();
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
    search.callback_port,
    handoff.mutate,
  ]);

  const isSuccess = search.status === "success";
  const copy = integrationCallbackCopy({
    status: search.status,
    disconnectedConnectionId: search.disconnected_connection_id,
  });

  if (search.flow === "desktop") {
    return (
      <AuthShell title={copy.title} description={copy.description}>
        {isSuccess ? (
          <div className="flex flex-col gap-3">
            {handoff.error ? (
              <p role="alert">
                Could not reach Anarlog. Keep the desktop app open and start
                connecting again from the app.
              </p>
            ) : null}
            <button
              disabled={
                handoff.isPending ||
                (search.callback_port !== undefined && handoff.isSuccess)
              }
              onClick={handleDeeplink}
              className={authPrimaryButtonClassName}
            >
              {search.callback_port !== undefined && handoff.isSuccess
                ? "Connected — return to the desktop app"
                : "Open Anarlog"}
            </button>

            <div className="rounded-xl border border-[#e5ddcf] bg-[#fbfaf7] p-4 text-center">
              <p className="mb-3 text-sm leading-6 text-[#756b5d]">
                Button not working? Copy the link instead
              </p>
              <button
                onClick={handleCopy}
                className={authSecondaryButtonClassName}
              >
                {copied ? (
                  <>
                    <Check className="size-4" />
                    Copied!
                  </>
                ) : (
                  <>
                    <Copy className="size-4" />
                    Copy URL
                  </>
                )}
              </button>
            </div>
          </div>
        ) : (
          <div className={authNoticeClassName}>
            <p className="text-sm font-medium text-[#4f4940]">
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
