import { FolderIcon } from "lucide-react";

import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbSeparator,
} from "@hypr/ui/components/ui/breadcrumb";
import { Button } from "@hypr/ui/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";

import { useBillingAccess } from "~/auth/billing";
import { FolderBreadcrumb } from "~/shared/ui/folder-breadcrumb";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";

export function FolderChain({ sessionId }: { sessionId: string }) {
  const { isPro } = useBillingAccess();
  const folderId = main.UI.useCell(
    "sessions",
    sessionId,
    "folder_id",
    main.STORE_ID,
  );

  if (!folderId) {
    return <UnassignedFolderBreadcrumb />;
  }

  return (
    <Breadcrumb className="ml-1.5 w-full min-w-0">
      <BreadcrumbList className="w-full flex-nowrap gap-0.5 overflow-hidden font-mono text-xs text-neutral-700">
        <FolderIcon className="mr-1 h-3 w-3 shrink-0" />
        <RenderFolderBreadcrumb folderId={folderId} isPro={isPro} />
      </BreadcrumbList>
    </Breadcrumb>
  );
}

function UnassignedFolderBreadcrumb() {
  return (
    <Breadcrumb className="ml-1.5 w-full min-w-0">
      <BreadcrumbList className="w-full flex-nowrap gap-0.5 overflow-hidden font-mono text-xs text-neutral-700">
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <BreadcrumbItem className="flex items-center gap-1.5 text-neutral-400">
              <FolderIcon className="h-3 w-3 shrink-0" />
              <span className="cursor-default">Unassigned</span>
            </BreadcrumbItem>
          </TooltipTrigger>
          <TooltipContent side="bottom">Coming soon</TooltipContent>
        </Tooltip>
      </BreadcrumbList>
    </Breadcrumb>
  );
}

function RenderFolderBreadcrumb({
  folderId,
  isPro,
}: {
  folderId: string;
  isPro: boolean;
}) {
  const openNew = useTabs((state) => state.openNew);

  return (
    <FolderBreadcrumb
      folderId={folderId}
      renderSeparator={({ index }) =>
        index > 0 ? <BreadcrumbSeparator className="shrink-0" /> : null
      }
      renderCrumb={({ id, name }) => (
        <BreadcrumbItem className="overflow-hidden">
          {isPro ? (
            <BreadcrumbLink asChild>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => openNew({ type: "folders", id })}
                className="truncate px-0 text-neutral-600 hover:text-black"
              >
                {name}
              </Button>
            </BreadcrumbLink>
          ) : (
            <span className="truncate text-neutral-600">{name}</span>
          )}
        </BreadcrumbItem>
      )}
    />
  );
}
