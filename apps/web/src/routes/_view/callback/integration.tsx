import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useEffect } from "react";
import { z } from "zod";

import { DeeplinkPrompt } from "@/components/deeplink-prompt";
import { flowSearchSchema } from "@/functions/desktop-flow";
import { useAutoDeeplink } from "@/hooks/use-auto-deeplink";

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

  const deeplink =
    search.flow === "desktop"
      ? buildDeeplinkUrl(scheme, {
          integration_id: search.integration_id,
          status: search.status,
          return_to: search.return_to,
        })
      : null;

  useAutoDeeplink(search.status === "success" ? deeplink : null);

  useEffect(() => {
    if (search.flow === "web") {
      void queryClient.invalidateQueries({
        predicate: (query) => query.queryKey[0] === "integration-status",
      });
      void navigate({ to: "/app/account/" });
    }
  }, [search.flow, navigate, queryClient]);

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

          {isSuccess && deeplink && <DeeplinkPrompt url={deeplink} />}
        </div>
      </div>
    );
  }

  if (search.flow === "web") {
    return <div>Redirecting...</div>;
  }
}
