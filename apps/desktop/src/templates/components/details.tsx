import { Pencil } from "lucide-react";

import type { TemplateSection } from "@hypr/store";

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
}) {
  if (isWebMode) {
    if (!selectedWebTemplate) {
      return <ResourceDetailEmpty message="No community templates available" />;
    }
    return (
      <WebTemplatePreview
        template={selectedWebTemplate}
        onClone={handleCloneTemplate}
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
}: {
  template: WebTemplate;
  onClone: (template: {
    title: string;
    description: string;
    category?: string;
    targets?: string[];
    sections: TemplateSection[];
  }) => void;
}) {
  return (
    <div className="flex h-full flex-1 flex-col">
      <ResourcePreviewHeader
        title={template.title || "Untitled"}
        description={template.description}
        category={template.category}
        targets={template.targets}
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
