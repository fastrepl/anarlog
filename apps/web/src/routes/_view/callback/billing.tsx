import { createFileRoute, redirect } from "@tanstack/react-router";
import { CheckIcon, CopyIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { z } from "zod";

import { cn } from "@hypr/utils";

import { desktopSchemeSchema } from "@/functions/desktop-flow";

const validateSearch = z.object({
  scheme: desktopSchemeSchema.optional(),
});

export const Route = createFileRoute("/_view/callback/billing")({
  validateSearch,
  beforeLoad: async ({ search }) => {
    if (!search.scheme) {
      throw redirect({ to: "/app/account/" });
    }
  },
  component: Component,
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
});

function Component() {
  const { scheme = "hyprnote" } = Route.useSearch();
  const [copied, setCopied] = useState(false);

  const deeplink = `${scheme}://billing/refresh`;

  const handleDeeplink = () => {
    window.location.href = deeplink;
  };

  const handleCopy = async () => {
    await navigator.clipboard.writeText(deeplink);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  useEffect(() => {
    const timer = setTimeout(() => {
      window.location.href = deeplink;
    }, 250);
    return () => clearTimeout(timer);
  }, [deeplink]);

  return (
    <div className="flex min-h-screen items-center justify-center bg-linear-to-b from-white via-stone-50/20 to-white p-6">
      <div className="flex w-full max-w-md flex-col gap-8 text-center">
        <div className="flex flex-col gap-3">
          <h1 className="font-serif text-3xl tracking-tight text-stone-700">
            Returning to Char
          </h1>
          <p className="text-neutral-600">
            Click the button below if the app does not open automatically
          </p>
        </div>

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
      </div>
    </div>
  );
}
