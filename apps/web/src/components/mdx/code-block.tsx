import { Check, Copy } from "lucide-react";
import { type ComponentPropsWithoutRef, useRef, useState } from "react";

import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";

export function CodeBlock({
  children,
  ...props
}: ComponentPropsWithoutRef<"pre">) {
  const [copied, setCopied] = useState(false);
  const [tooltipOpen, setTooltipOpen] = useState(false);
  const preRef = useRef<HTMLPreElement>(null);

  const handleCopy = async () => {
    const textToCopy = preRef.current?.textContent ?? "";

    if (!textToCopy) return;

    await navigator.clipboard.writeText(textToCopy);
    setCopied(true);
    setTooltipOpen(true);
    setTimeout(() => {
      setCopied(false);
      setTooltipOpen(false);
    }, 2000);
  };

  return (
    <TooltipProvider delayDuration={0}>
      <div className="group relative">
        <pre ref={preRef} {...props}>
          {children}
        </pre>
        <Tooltip
          open={tooltipOpen}
          onOpenChange={(open) => {
            setTooltipOpen(open);
            if (!open) setCopied(false);
          }}
        >
          <TooltipTrigger asChild>
            <button
              type="button"
              onClick={handleCopy}
              className="absolute top-2.5 right-2.5 cursor-pointer rounded bg-stone-200/80 p-1.5 text-stone-600 opacity-0 transition-all group-hover:opacity-100 hover:bg-stone-300/80"
              aria-label={copied ? "Copied" : "Copy code"}
            >
              {copied ? (
                <Check className="h-4 w-4 text-green-600" />
              ) : (
                <Copy className="h-4 w-4" />
              )}
            </button>
          </TooltipTrigger>
          <TooltipContent className="rounded-md bg-black text-white">
            {copied ? "Copied" : "Copy"}
          </TooltipContent>
        </Tooltip>
      </div>
    </TooltipProvider>
  );
}
