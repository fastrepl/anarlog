import { Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export function SidebarDownloadCard() {
  return (
    <div className="overflow-hidden rounded-xs border border-neutral-200 bg-white bg-[linear-gradient(to_right,#f5f5f5_1px,transparent_1px),linear-gradient(to_bottom,#f5f5f5_1px,transparent_1px)] bg-size-[24px_24px] bg-position-[12px_12px,12px_12px] p-4">
      <h3 className="mb-3 text-center font-serif text-sm text-stone-700">
        Try Char for yourself
      </h3>
      <Link
        to="/download/"
        className={cn([
          "group flex h-9 w-full items-center justify-center px-4 text-sm",
          "rounded-full bg-linear-to-t from-stone-600 to-stone-500 text-white",
          "hover:scale-[102%] active:scale-[98%]",
          "transition-all",
        ])}
      >
        Download for free
      </Link>
    </div>
  );
}

interface CtaCardProps {
  title?: string;
  description?: string;
  buttonText?: string;
  buttonUrl?: string;
  source?: string;
}

export function CtaCard({
  title = "Talk to the founders",
  description = "Drowning in back-to-back meetings? In 20 minutes, we'll show you how to take control of your notes and reclaim hours each week.",
  buttonText = "Book a call",
  buttonUrl = "/founders",
  source = "blog",
}: CtaCardProps) {
  const finalUrl =
    buttonUrl === "/founders" && source
      ? `${buttonUrl}?source=${source}`
      : buttonUrl;
  return (
    <div className="my-12 overflow-hidden rounded-xs border border-neutral-200 bg-white bg-[linear-gradient(to_right,#f5f5f5_1px,transparent_1px),linear-gradient(to_bottom,#f5f5f5_1px,transparent_1px)] bg-size-[24px_24px] bg-position-[12px_12px,12px_12px]">
      <div className="p-8 text-center">
        <h3 className="mb-3 font-serif text-2xl text-stone-700">{title}</h3>
        {description && (
          <p className="mx-auto mb-6 max-w-2xl text-base text-neutral-600">
            {description}
          </p>
        )}
        <a
          href={finalUrl}
          target="_blank"
          rel="noopener noreferrer"
          className={cn([
            "group inline-flex h-12 min-w-52 items-center justify-center px-6 text-base sm:text-lg",
            "rounded-full bg-linear-to-t from-stone-600 to-stone-500",
            "shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
            "transition-all",
            "text-white! no-underline!",
          ])}
        >
          {buttonText}
        </a>
      </div>
    </div>
  );
}
