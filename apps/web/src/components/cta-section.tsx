import { cn } from "@hypr/utils";

import { DownloadButton } from "@/components/download-button";
import { GithubStars } from "@/components/github-stars";
import { getPlatformCTA, usePlatform } from "@/hooks/use-platform";

export function CTASection({
  title = "Your meetings. Your data. Your control.",
  description = "Start taking meeting notes with AI—without the lock-in",
  heroInputRef,
}: {
  title?: string;
  description?: string;
  heroInputRef?: React.RefObject<HTMLInputElement | null>;
}) {
  const platform = usePlatform();
  const platformCTA = getPlatformCTA(platform);

  const getButtonLabel = () => {
    if (platform === "mobile") {
      return "Get reminder";
    }
    return platformCTA.label;
  };

  const handleCTAClick = () => {
    if (platformCTA.action === "waitlist" && heroInputRef?.current) {
      window.scrollTo({ top: 0, behavior: "smooth" });
      setTimeout(() => {
        if (heroInputRef.current) {
          heroInputRef.current.focus();
          heroInputRef.current.parentElement?.classList.add(
            "animate-shake",
            "border-stone-600",
          );
          setTimeout(() => {
            heroInputRef.current?.parentElement?.classList.remove(
              "animate-shake",
              "border-stone-600",
            );
          }, 500);
        }
      }, 500);
    }
  };

  return (
    <section className="laptop:px-0 px-4 py-16">
      <div className="flex flex-col items-center gap-6 text-center">
        <h2 className="text-color font-mono text-2xl tracking-wide md:text-6xl">
          {title}
        </h2>
        <p className="text-fg-muted mx-auto max-w-2xl text-lg">{description}</p>
        <div className="flex flex-col items-center justify-center gap-4 pt-6 sm:flex-row">
          {platformCTA.action === "download" ? (
            <DownloadButton />
          ) : (
            <button
              onClick={handleCTAClick}
              className={cn([
                "group flex h-12 items-center justify-center px-6 text-base sm:text-lg",
                "rounded-full bg-linear-to-t from-stone-600 to-stone-500 text-white",
                "shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
                "transition-all",
              ])}
            >
              {getButtonLabel()}
              <svg
                xmlns="http://www.w3.org/2000/svg"
                fill="none"
                viewBox="0 0 24 24"
                strokeWidth="1.5"
                stroke="currentColor"
                className="ml-2 h-5 w-5 transition-transform group-hover:translate-x-1"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="m12.75 15 3-3m0 0-3-3m3 3h-7.5M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z"
                />
              </svg>
            </button>
          )}
          <div className="hidden sm:block">
            <GithubStars />
          </div>
        </div>
      </div>
    </section>
  );
}
