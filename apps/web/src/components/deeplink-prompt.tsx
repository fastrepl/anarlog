import { CheckIcon, CopyIcon } from "lucide-react";
import { useState } from "react";

import { cn } from "@hypr/utils";

import { openDeeplink } from "@/hooks/use-auto-deeplink";

export function DeeplinkPrompt({ url }: { url: string }) {
  const [copied, setCopied] = useState(false);

  const handleDeeplink = () => {
    openDeeplink(url);
  };

  const handleCopy = async () => {
    await navigator.clipboard.writeText(url);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
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
  );
}
