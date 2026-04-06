import { HeartIcon, MoreHorizontalIcon } from "lucide-react";
import { useState } from "react";

import type { TemplateSection } from "@hypr/store";
import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";
import { cn } from "@hypr/utils";

import { getTemplateCreatorByline } from "../shared";
import { TemplateDetailScrollArea } from "./detail-scroll-area";
import { SectionsList } from "./sections-editor";
import { TemplateForm } from "./template-form";

import {
  ResourceDetailEmpty,
  ResourcePreviewHeader,
} from "~/shared/ui/resource-list";

type WebTemplate = {
  slug: string;
  title: string;
  description: string;
  category: string;
  targets?: string[];
  sections: TemplateSection[];
};

export function TemplateDetailsColumn({
  isWebMode,
  selectedMineId,
  selectedWebTemplate,
  handleDeleteTemplate,
  handleDuplicateTemplate,
  handleCloneTemplate,
  handleFavoriteTemplate,
  handleSetDefaultTemplate,
}: {
  isWebMode: boolean;
  selectedMineId: string | null;
  selectedWebTemplate: WebTemplate | null;
  handleDeleteTemplate: (id: string) => void;
  handleDuplicateTemplate: (id: string) => void;
  handleCloneTemplate: (template: {
    title: string;
    description: string;
    category?: string;
    targets?: string[];
    sections: TemplateSection[];
  }) => void;
  handleFavoriteTemplate: (template: {
    title: string;
    description: string;
    category?: string;
    targets?: string[];
    sections: TemplateSection[];
  }) => void;
  handleSetDefaultTemplate: (template: {
    title: string;
    description: string;
    category?: string;
    targets?: string[];
    sections: TemplateSection[];
  }) => void;
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

  if (!selectedMineId) {
    return <ResourceDetailEmpty message="No templates yet" />;
  }

  return (
    <TemplateForm
      key={selectedMineId}
      id={selectedMineId}
      handleDeleteTemplate={handleDeleteTemplate}
      handleDuplicateTemplate={handleDuplicateTemplate}
    />
  );
}

function WebTemplatePreview({
  template,
  onClone,
  onFavorite,
  onSetDefault,
}: {
  template: WebTemplate;
  onClone: (template: {
    title: string;
    description: string;
    category?: string;
    targets?: string[];
    sections: TemplateSection[];
  }) => void;
  onFavorite: (template: {
    title: string;
    description: string;
    category?: string;
    targets?: string[];
    sections: TemplateSection[];
  }) => void;
  onSetDefault: (template: {
    title: string;
    description: string;
    category?: string;
    targets?: string[];
    sections: TemplateSection[];
  }) => void;
}) {
  const nextTemplate = {
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
          <span className="shrink-0 text-sm font-normal whitespace-nowrap text-neutral-400">
            {getTemplateCreatorByline({ isUserTemplate: false })}
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
              className="shrink-0 text-neutral-600 hover:text-black"
            >
              Set as default
            </Button>
            <Button
              type="button"
              size="icon"
              variant="ghost"
              onClick={() => onFavorite(nextTemplate)}
              className="text-neutral-500 hover:text-neutral-800"
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
                    "text-neutral-500 hover:text-neutral-800",
                    actionsOpen &&
                      "bg-neutral-100 text-neutral-800 hover:bg-neutral-100",
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
      />

      <TemplateDetailScrollArea>
        <SectionsList
          disabled={true}
          items={template.sections ?? []}
          onChange={() => {}}
        />
      </TemplateDetailScrollArea>
    </div>
  );
}
