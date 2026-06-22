import { FolderIcon } from "lucide-react";
import { useMemo } from "react";

import { cn } from "@hypr/utils";

import * as main from "~/store/tinybase/store/main";

export function NoteTitleBreadcrumb({
  sessionId,
  title,
}: {
  sessionId: string;
  title: React.ReactNode;
}) {
  const folderId = main.UI.useCell(
    "sessions",
    sessionId,
    "folder_id",
    main.STORE_ID,
  ) as string | undefined;
  const folderChain = useFolderChain(folderId);

  return (
    <nav
      aria-label="Note breadcrumb"
      data-tauri-drag-region="false"
      className={cn([
        "ml-1.5 flex max-w-full min-w-0 items-center overflow-hidden",
        "text-xs text-neutral-700",
      ])}
    >
      <ol className="flex min-w-0 flex-nowrap items-center gap-0.5 overflow-hidden">
        {folderChain.length > 0 ? (
          <>
            <li className="mr-1 shrink-0">
              <FolderIcon aria-hidden="true" className="size-3" />
            </li>
            {folderChain.map((folder, index) => (
              <BreadcrumbFolderCrumb
                key={folder.id}
                name={folder.name}
                showSeparator={index > 0}
              />
            ))}
            <BreadcrumbSeparator />
          </>
        ) : null}
        <li className="min-w-0 overflow-hidden">
          <span aria-current="page" className="text-foreground font-normal">
            {title}
          </span>
        </li>
      </ol>
    </nav>
  );
}

function BreadcrumbFolderCrumb({
  name,
  showSeparator,
}: {
  name: string;
  showSeparator: boolean;
}) {
  return (
    <>
      {showSeparator ? <BreadcrumbSeparator /> : null}
      <li className="min-w-0 overflow-hidden">
        <span className="truncate text-neutral-600">{name}</span>
      </li>
    </>
  );
}

function BreadcrumbSeparator() {
  return (
    <li aria-hidden="true" className="text-muted-foreground shrink-0 px-0.5">
      /
    </li>
  );
}

function useFolderChain(folderId: string | undefined) {
  return useMemo(() => {
    const parts = (folderId ?? "").split("/").filter(Boolean);
    return parts.map((_, index) => {
      const id = parts.slice(0, index + 1).join("/");
      return {
        id,
        name: parts[index] || "Untitled",
      };
    });
  }, [folderId]);
}
