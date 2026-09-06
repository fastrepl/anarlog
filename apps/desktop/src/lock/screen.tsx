import { Trans, useLingui } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";

import { Lock } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import { cn } from "@anlg/utils";

export function LockScreen({
  title,
  description,
  action,
  authenticating,
  onUnlock,
  className,
}: {
  title: string;
  description?: string;
  action?: string;
  authenticating: boolean;
  onUnlock: () => void;
  className?: string;
}) {
  return (
    <div
      data-lock-screen
      data-tauri-drag-region
      className={cn([
        "flex h-full min-h-0 w-full flex-1 flex-col items-center justify-center",
        className ?? "bg-background",
      ])}
    >
      <div className="flex max-w-sm flex-col items-center px-6 text-center">
        <div className="bg-muted flex size-14 items-center justify-center rounded-full">
          <Lock className="text-foreground size-6" />
        </div>
        <h1 className="mt-5 text-lg font-semibold">{title}</h1>
        {description ? (
          <p className="text-muted-foreground mt-2 text-sm">{description}</p>
        ) : null}
        <Button
          className="mt-6"
          disabled={authenticating}
          onClick={onUnlock}
          data-tauri-drag-region="false"
        >
          {authenticating ? (
            <Trans>Authenticating…</Trans>
          ) : action ? (
            action
          ) : (
            <Trans>Unlock</Trans>
          )}
        </Button>
      </div>
    </div>
  );
}

export function useDeviceAuthHint() {
  const { t } = useLingui();
  switch (platform()) {
    case "macos":
      return t`Use Touch ID or enter your password to view.`;
    case "windows":
      return t`Use Windows Hello face, PIN, or password to view.`;
    default:
      return t`Authenticate to continue.`;
  }
}

export function NoteLockScreen({
  sessionTitle,
  authenticating,
  onUnlock,
}: {
  sessionTitle?: string;
  authenticating: boolean;
  onUnlock: () => void;
}) {
  const { t } = useLingui();
  const hint = useDeviceAuthHint();
  return (
    <LockScreen
      title={sessionTitle || t`Note is Locked`}
      description={hint}
      action={t`View Note`}
      authenticating={authenticating}
      onUnlock={onUnlock}
      className="bg-transparent"
    />
  );
}
