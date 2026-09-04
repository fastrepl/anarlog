import { Trans } from "@lingui/react/macro";
import { useMutation } from "@tanstack/react-query";
import { useMemo, useState } from "react";

import {
  CirclesThreePlus,
  FileText,
  FolderSimple,
  Lightning,
} from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn } from "@anlg/utils";

import type { SharedResource, SharedResourceType } from "./client";
import { useSharedResources } from "./hooks";

import { useOptionalAuth } from "~/auth";
import {
  GlassDialogCancelButton,
  GlassDialogContent,
} from "~/shared/ui/glass-dialog";

export function SharedResourceLibrarySection(props: {
  resourceType: SharedResourceType;
  search?: string;
  onImport: (resource: SharedResource) => Promise<void>;
}) {
  const auth = useOptionalAuth();
  if (!auth) return null;
  return <AuthenticatedSharedResourceLibrarySection {...props} />;
}

function AuthenticatedSharedResourceLibrarySection({
  resourceType,
  search = "",
  onImport,
}: {
  resourceType: SharedResourceType;
  search?: string;
  onImport: (resource: SharedResource) => Promise<void>;
}) {
  const resources = useSharedResources(resourceType);
  const [selected, setSelected] = useState<SharedResource | null>(null);
  const available = useMemo(() => {
    const query = search.trim().toLowerCase();
    return (resources.data ?? []).filter(
      (resource) =>
        resource.accessKind !== "owner" &&
        (!query || resource.title.toLowerCase().includes(query)),
    );
  }, [resources.data, search]);
  const importResource = useMutation({
    mutationFn: async () => {
      if (!selected) throw new Error("Choose an item to add");
      await onImport(selected);
    },
    onSuccess: () => {
      sonnerToast.success("Added a copy to your library");
      setSelected(null);
    },
    onError: (error) => {
      sonnerToast.error(
        error instanceof Error ? error.message : "Could not add this item",
      );
    },
  });

  if (available.length === 0) return null;

  return (
    <section className="border-border/60 mt-3 border-t pt-3">
      <div className="text-muted-foreground mb-1 flex items-center gap-1.5 px-3 text-[11px] font-medium tracking-wide uppercase">
        <CirclesThreePlus className="size-3.5" />
        <Trans>Shared with me</Trans>
      </div>
      <ul className="flex flex-col">
        {available.map((resource) => (
          <li key={resource.shareId}>
            <button
              type="button"
              className={cn([
                "hover:bg-accent/50 flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left transition-colors",
              ])}
              onClick={() => setSelected(resource)}
            >
              <ResourceIcon resourceType={resource.resourceType} />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-sm">{resource.title}</span>
                <span className="text-muted-foreground block truncate text-[11px]">
                  {resource.workspaceName ?? resource.ownerEmail}
                </span>
              </span>
            </button>
          </li>
        ))}
      </ul>

      <Dialog
        open={selected !== null}
        onOpenChange={(open) => {
          if (!open && !importResource.isPending) setSelected(null);
        }}
      >
        <GlassDialogContent>
          <DialogHeader className="gap-1">
            <DialogTitle>{selected?.title}</DialogTitle>
            <DialogDescription>
              {selected ? resourceDescription(selected) : null}
            </DialogDescription>
          </DialogHeader>
          <p className="text-muted-foreground text-xs leading-5">
            <Trans>
              This adds a copy you can edit without changing the shared item.
            </Trans>
          </p>
          <DialogFooter className="grid grid-cols-2 gap-2 sm:grid-cols-2 sm:justify-normal">
            <GlassDialogCancelButton
              type="button"
              disabled={importResource.isPending}
              onClick={() => setSelected(null)}
            >
              <Trans>Cancel</Trans>
            </GlassDialogCancelButton>
            <Button
              type="button"
              className="rounded-full"
              disabled={importResource.isPending}
              onClick={() => importResource.mutate()}
            >
              <Trans>Add a copy</Trans>
            </Button>
          </DialogFooter>
        </GlassDialogContent>
      </Dialog>
    </section>
  );
}

function ResourceIcon({ resourceType }: { resourceType: SharedResourceType }) {
  const className = "text-muted-foreground size-4 shrink-0";
  if (resourceType === "folder") {
    return <FolderSimple className={className} />;
  }
  if (resourceType === "template") {
    return <FileText className={className} />;
  }
  return <Lightning className={className} />;
}

function resourceDescription(resource: SharedResource): string {
  const payload = resource.payload;
  if (resource.resourceType === "folder") {
    const count = Array.isArray(payload.notes) ? payload.notes.length : 0;
    return `${count} ${count === 1 ? "note" : "notes"} shared by ${resource.ownerEmail}`;
  }
  if (resource.resourceType === "template") {
    const template = payload.template;
    if (
      typeof template === "object" &&
      template !== null &&
      "description" in template &&
      typeof template.description === "string" &&
      template.description
    ) {
      return template.description;
    }
  }
  if (resource.resourceType === "automation") {
    const workflow = payload.workflow;
    const count =
      typeof workflow === "object" &&
      workflow !== null &&
      "steps" in workflow &&
      Array.isArray(workflow.steps)
        ? workflow.steps.length
        : 0;
    return `${count} ${count === 1 ? "step" : "steps"} shared by ${resource.ownerEmail}`;
  }
  return `Shared by ${resource.ownerEmail}`;
}
