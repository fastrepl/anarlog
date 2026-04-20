import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

export function UpdateBannerView({
  version,
  progress,
  installing,
  onInstall,
  onDismiss,
}: {
  version: string;
  progress: number | null;
  installing: boolean;
  onInstall: () => void;
  onDismiss: () => void;
}) {
  const isDownloading = progress !== null && progress < 1;

  return (
    <div
      className={cn([
        "flex items-center justify-center gap-3 px-4 py-1.5",
        "bg-neutral-50 text-sm text-neutral-700",
      ])}
    >
      <span>New version {version} is ready.</span>
      {isDownloading ? (
        <DownloadProgress progress={progress} />
      ) : (
        <Button
          size="sm"
          onClick={onInstall}
          disabled={installing}
          className="h-7 bg-black px-3 text-xs font-medium text-white hover:bg-neutral-800"
        >
          {installing ? "Installing..." : "Update & Restart"}
        </Button>
      )}
      <Button
        type="button"
        size="sm"
        variant="ghost"
        onClick={onDismiss}
        className="h-7 px-2 text-xs font-medium text-neutral-500 hover:bg-transparent hover:text-neutral-700"
      >
        Later
      </Button>
    </div>
  );
}

function DownloadProgress({ progress }: { progress: number }) {
  const pct = Math.round(progress * 100);
  const width = `${pct}%`;

  return (
    <div className="flex items-center gap-2">
      <div className="h-1.5 w-32 overflow-hidden rounded-full bg-neutral-200">
        <div
          className="h-full rounded-full bg-neutral-500 transition-all duration-300"
          style={{ width }}
        />
      </div>
      <span className="text-xs text-neutral-500">{pct}%</span>
    </div>
  );
}
