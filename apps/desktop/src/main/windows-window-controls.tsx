import { t } from "@lingui/core/macro";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useState } from "react";

import { cn } from "@anlg/utils";

import { useMountEffect } from "~/shared/hooks/useMountEffect";

const appWindow = getCurrentWindow();

export function WindowsWindowControls({ onClose }: { onClose?: () => void }) {
  const [isMaximized, setIsMaximized] = useState(false);

  const syncMaximized = useCallback(() => {
    void appWindow
      .isMaximized()
      .then(setIsMaximized)
      .catch(() => setIsMaximized(false));
  }, []);

  useMountEffect(() => {
    let cancelled = false;
    let unlistenResize: (() => void) | undefined;

    const sync = () => {
      if (!cancelled) {
        syncMaximized();
      }
    };

    sync();
    void appWindow
      .onResized(sync)
      .then((unlisten) => {
        if (cancelled) {
          unlisten();
          return;
        }

        unlistenResize = unlisten;
      })
      .catch(() => {});

    return () => {
      cancelled = true;
      unlistenResize?.();
    };
  });

  const toggleMaximize = useCallback(async () => {
    await appWindow.toggleMaximize();
    syncMaximized();
  }, [syncMaximized]);

  return (
    <div className="flex shrink-0" data-tauri-drag-region="false">
      <WindowControlButton
        ariaLabel={t`Minimize`}
        onClick={() => void appWindow.minimize()}
      >
        <span className="h-px w-2.5 bg-current" />
      </WindowControlButton>
      <WindowControlButton
        ariaLabel={isMaximized ? t`Restore` : t`Maximize`}
        onClick={() => void toggleMaximize()}
      >
        {isMaximized ? <RestoreIcon /> : <MaximizeIcon />}
      </WindowControlButton>
      <WindowControlButton
        ariaLabel={t`Close`}
        close
        onClick={onClose ?? (() => void appWindow.close())}
      >
        <span className="relative size-3">
          <span className="absolute top-[5.5px] left-0 h-px w-3 rotate-45 bg-current" />
          <span className="absolute top-[5.5px] left-0 h-px w-3 -rotate-45 bg-current" />
        </span>
      </WindowControlButton>
    </div>
  );
}

function WindowControlButton({
  ariaLabel,
  children,
  close = false,
  onClick,
}: {
  ariaLabel: string;
  children: React.ReactNode;
  close?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={ariaLabel}
      data-tauri-drag-region="false"
      className={cn([
        "text-foreground flex h-10 w-[46px] items-center justify-center transition-colors",
        close
          ? "hover:bg-[#c42b1c] hover:text-white"
          : "hover:bg-foreground/10",
        "focus-visible:ring-ring focus-visible:ring-2 focus-visible:outline-hidden focus-visible:ring-inset",
      ])}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

function MaximizeIcon() {
  return <span className="size-2.5 border border-current" />;
}

function RestoreIcon() {
  return (
    <span className="relative size-3">
      <span className="absolute top-0.5 right-0 size-2 border border-current" />
      <span className="bg-background absolute bottom-0.5 left-0 size-2 border border-current" />
    </span>
  );
}
