import { Pencil } from "lucide-react";

import type { TemplateSection } from "@hypr/store";
import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import type { WebTemplate } from "../shared";
import { TemplateDetailScrollArea } from "./detail-scroll-area";
import { SectionsList } from "./sections-editor";
import { TemplateForm } from "./template-form";

import {
  ResourceDetailEmpty,
  ResourcePreviewHeader,
} from "~/shared/ui/resource-list";

export function TemplateDetailsColumn({
  isWebMode,
  selectedMineId,
  selectedWebTemplate,
  isSelectedWebTemplateDefault,
  handleDeleteTemplate,
  handleDuplicateTemplate,
  handleCloneTemplate,
  handleSetDefaultWebTemplate,
}: {
  isWebMode: boolean;
  selectedMineId: string | null;
  selectedWebTemplate: WebTemplate | null;
  isSelectedWebTemplateDefault: boolean;
  handleDeleteTemplate: (id: string) => void;
  handleDuplicateTemplate: (id: string) => void;
  handleCloneTemplate: (template: {
    title: string;
    description: string;
    category?: string;
    targets?: string[];
    sections: TemplateSection[];
  }) => void;
  handleSetDefaultWebTemplate: (template: WebTemplate) => void;
}) {
  if (isWebMode) {
    if (!selectedWebTemplate) {
      return <ResourceDetailEmpty message="No community templates available" />;
    }
    return (
      <WebTemplatePreview
        template={selectedWebTemplate}
        isDefault={isSelectedWebTemplateDefault}
        onClone={handleCloneTemplate}
        onSetDefault={handleSetDefaultWebTemplate}
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
  isDefault,
  onClone,
  onSetDefault,
}: {
  template: WebTemplate;
  isDefault: boolean;
  onClone: (template: {
    title: string;
    description: string;
    category?: string;
    targets?: string[];
    sections: TemplateSection[];
  }) => void;
  onSetDefault: (template: WebTemplate) => void;
}) {
  return (
    <div className="flex h-full flex-1 flex-col">
      <ResourcePreviewHeader
        title={template.title || "Untitled"}
        description={template.description}
        category={template.category}
        targets={template.targets}
        actions={
          <div className="flex items-center gap-0">
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={() => onSetDefault(template)}
              title={
                isDefault
                  ? "Used for generated summaries"
                  : "Use for generated summaries"
              }
              className={cn([
                "shrink-0 text-neutral-600 hover:text-black",
                isDefault
                  ? "bg-neutral-100 text-black hover:bg-neutral-100"
                  : null,
              ])}
            >
              {isDefault ? "Current default" : "Set as default"}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={() =>
                onClone({
                  title: template.title ?? "",
                  description: template.description ?? "",
                  category: template.category,
                  targets: template.targets,
                  sections: template.sections ?? [],
                })
              }
              className="shrink-0 text-neutral-600 hover:text-black"
            >
              <Pencil size={14} className="mr-2 shrink-0" />
              Edit
            </Button>
          </div>
        }
        actionLabel="Edit"
        actionIcon={<Pencil size={14} className="shrink-0" />}
        actionVariant="ghost"
        actionClassName="shrink-0 text-neutral-600 hover:text-black"
        onClone={() =>
          onClone({
            title: template.title ?? "",
            description: template.description ?? "",
            category: template.category,
            targets: template.targets,
            sections: template.sections ?? [],
          })
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
