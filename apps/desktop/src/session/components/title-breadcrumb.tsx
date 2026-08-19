import { useLingui } from "@lingui/react/macro";

import { cn } from "@anlg/utils";

import { FolderPicker } from "./folder-picker";

import { useSession } from "~/session/queries";

export function NoteTitleBreadcrumb({
  sessionId,
  title,
}: {
  sessionId: string;
  title: React.ReactNode;
}) {
  const { t } = useLingui();
  const folderId = useSession(sessionId)?.folder_id ?? "";
  const hasFolder = Boolean(folderId);

  return (
    <nav
      aria-label={t`Note breadcrumb`}
      data-tauri-drag-region="false"
      className={cn([
        "flex max-w-full min-w-0 items-center gap-0.5 overflow-hidden",
        "text-xs text-neutral-700",
      ])}
    >
      <FolderPicker sessionId={sessionId} />
      {hasFolder ? (
        <span
          aria-hidden="true"
          className="text-muted-foreground shrink-0 px-0.5"
        >
          /
        </span>
      ) : null}
      <div className="min-w-0 overflow-hidden">
        <span aria-current="page" className="text-foreground font-normal">
          {title}
        </span>
      </div>
    </nav>
  );
}
