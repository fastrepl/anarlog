import { HeartIcon, MoreHorizontalIcon, PlusIcon } from "lucide-react";
import { useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";
import { cn } from "@hypr/utils";

import { type WebTemplate } from "./codec";
import { type UserTemplate, type UserTemplateDraft } from "./queries";
import { SectionsList } from "./sections-editor";
import { TemplateForm } from "./template-form";
import { getTemplateCreatorLabel } from "./utils";

import {
  ResourceDetailEmpty,
  ResourcePreviewHeader,
} from "~/shared/ui/resource-list";

export function TemplateDetailsColumn({
  isWebMode,
  selectedMineTemplate,
  selectedWebTemplate,
  handleCreateTemplate,
  handleDeleteTemplate,
  handleDuplicateTemplate,
  handleCloneTemplate,
  handleFavoriteTemplate,
  handleSetDefaultTemplate,
}: {
  isWebMode: boolean;
  selectedMineTemplate: UserTemplate | null;
  selectedWebTemplate: WebTemplate | null;
  handleCreateTemplate: () => void;
  handleDeleteTemplate: (id: string) => void;
  handleDuplicateTemplate: (id: string) => void;
  handleCloneTemplate: (template: UserTemplateDraft) => void;
  handleFavoriteTemplate: (template: UserTemplateDraft) => void;
  handleSetDefaultTemplate: (template: UserTemplateDraft) => void;
}) {
  if (isWebMode) {
    if (!selectedWebTemplate) {
      return <ResourceDetailEmpty message="No community templates available" />;
    }
    return (
      <WebTemplatePreview
        template={selectedWebTemplate}
        onClone={handleCloneTemplate}
        onFavorite={handleFavoriteTemplate}
        onSetDefault={handleSetDefaultTemplate}
      />
    );
  }

  if (!selectedMineTemplate) {
    return <TemplateDetailEmpty onCreate={handleCreateTemplate} />;
  }

  return (
    <TemplateForm
      key={selectedMineTemplate.id}
      template={selectedMineTemplate}
      handleDeleteTemplate={handleDeleteTemplate}
      handleDuplicateTemplate={handleDuplicateTemplate}
    />
  );
}

function TemplateDetailEmpty({ onCreate }: { onCreate: () => void }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3">
      <p className="text-muted-foreground text-sm">No templates yet</p>
      <Button
        type="button"
        size="sm"
        variant="outline"
        onClick={onCreate}
        className="gap-2"
      >
        <PlusIcon className="size-4" />
        Create template
      </Button>
    </div>
  );
}

function WebTemplatePreview({
  template,
  onClone,
  onFavorite,
  onSetDefault,
}: {
  template: WebTemplate;
  onClone: (template: UserTemplateDraft) => void;
  onFavorite: (template: UserTemplateDraft) => void;
  onSetDefault: (template: UserTemplateDraft) => void;
}) {
  const nextTemplate: UserTemplateDraft = {
    title: template.title ?? "",
    description: template.description ?? "",
    category: template.category,
    targets: template.targets,
    sections: template.sections ?? [],
  };
  const [actionsOpen, setActionsOpen] = useState(false);

  return (
    <div className="flex h-full flex-1 flex-col">
      <ResourcePreviewHeader
        title={template.title || "Untitled"}
        description={template.description}
        category={template.category}
        targets={template.targets}
        titleMeta={
          <span className="text-muted-foreground shrink-0 text-sm font-normal whitespace-nowrap">
            {getTemplateCreatorLabel({
              isUserTemplate: false,
              format: "short",
            })}
          </span>
        }
        footer={null}
        actions={
          <>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={() => onSetDefault(nextTemplate)}
              className="text-muted-foreground shrink-0 hover:text-black"
            >
              Set as default
            </Button>
            <Button
              type="button"
              size="icon"
              variant="ghost"
              onClick={() => onFavorite(nextTemplate)}
              className="text-muted-foreground hover:text-foreground"
              title="Favorite template"
              aria-label="Favorite template"
            >
              <HeartIcon className="size-4" />
            </Button>
            <DropdownMenu open={actionsOpen} onOpenChange={setActionsOpen}>
              <DropdownMenuTrigger asChild>
                <Button
                  type="button"
                  size="icon"
                  variant="ghost"
                  className={cn([
                    "text-muted-foreground hover:text-foreground",
                    actionsOpen && "bg-muted text-foreground hover:bg-accent",
                  ])}
                  aria-label="Template actions"
                >
                  <MoreHorizontalIcon className="size-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent variant="app" align="end">
                <AppFloatingPanel className="overflow-hidden p-1">
                  <DropdownMenuItem
                    onClick={() => onClone(nextTemplate)}
                    className="cursor-pointer"
                  >
                    Duplicate
                  </DropdownMenuItem>
                </AppFloatingPanel>
              </DropdownMenuContent>
            </DropdownMenu>
          </>
        }
      >
        <div className="mt-6">
          <SectionsList
            disabled={true}
            items={template.sections ?? []}
            onChange={() => {}}
          />
        </div>
      </ResourcePreviewHeader>
    </div>
  );
}
