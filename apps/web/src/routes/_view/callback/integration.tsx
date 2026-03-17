import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { CheckIcon, CopyIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { z } from "zod";

import { cn } from "@hypr/utils";

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
      void navigate({ to: "/app/account/" });
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
      <div className="flex min-h-screen items-center justify-center bg-linear-to-b from-white via-stone-50/20 to-white p-6">
        <div className="flex w-full max-w-md flex-col gap-8 text-center">
          <div className="flex flex-col gap-3">
            <h1 className="font-serif text-3xl tracking-tight text-stone-700">
              {isSuccess ? "Connection successful" : "Connection failed"}
            </h1>
            <p className="text-neutral-600">
              {isSuccess
                ? "Click the button below to return to the app"
                : "Something went wrong during the connection"}
            </p>
          </div>

          {isSuccess && (
            <div className="flex flex-col gap-4">
              <button
                onClick={handleDeeplink}
                className={cn([
                  "flex h-12 w-full cursor-pointer items-center justify-center text-base font-medium transition-all",
                  "rounded-full bg-linear-to-t from-stone-600 to-stone-500 text-white shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
                ])}
              >
                Open Char
              </button>

              <button
                onClick={handleCopy}
                className={cn([
                  "flex w-full cursor-pointer flex-col items-center gap-3 p-4 text-left transition-all",
                  "rounded-lg border border-stone-100 bg-stone-50 hover:bg-stone-100 active:scale-[99%]",
                ])}
              >
                <p className="text-sm text-stone-500">
                  Button not working? Copy the link instead
                </p>
                <span
                  className={cn([
                    "flex h-10 w-full items-center justify-center gap-2 text-sm font-medium",
                    "rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900 shadow-xs",
                  ])}
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
                </span>
              </button>
            </div>
          )}
        </div>
      </div>
    );
  }

  if (search.flow === "web") {
    return <div>Redirecting...</div>;
  }
}
