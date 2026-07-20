import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { CheckIcon, CopyIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { z } from "zod";

import {
  AuthShell,
  authNoticeClassName,
  authPrimaryButtonClassName,
  authSecondaryButtonClassName,
} from "@/components/auth-shell";
import { flowSearchSchema } from "@/functions/desktop-flow";

const commonSearch = {
  integration_id: z.string(),
  status: z.string(),
  return_to: z.string().optional(),
};

const validateSearch = flowSearchSchema(commonSearch, {
  defaultFlow: "desktop",
});

type IntegrationDeeplinkParams = {
  integration_id: string;
  status: string;
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
  if (search.return_to) {
    params.set("return_to", search.return_to);
  }
  return `${scheme}://integration/callback?${params.toString()}`;
}

function Component() {
  const search = Route.useSearch();
  const scheme = search.scheme ?? "hyprnote";
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [copied, setCopied] = useState(false);

  const getDeeplink = () => {
    return buildDeeplinkUrl(scheme, {
      integration_id: search.integration_id,
      status: search.status,
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
      void navigate({ to: "/app/account/" } as any);
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
          <div className="flex flex-col gap-3">
            <button
              onClick={handleDeeplink}
              className={authPrimaryButtonClassName}
            >
              Open Anarlog
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
                    <CheckIcon className="size-4" />
                    Copied!
                  </>
                ) : (
                  <>
                    <CopyIcon className="size-4" />
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
