import type { TemplateSection } from "@hypr/store";

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
  handleCloneTemplate,
}: {
  isWebMode: boolean;
  selectedMineId: string | null;
  selectedWebTemplate: WebTemplate | null;
  handleDeleteTemplate: (id: string) => void;
  handleCloneTemplate: (template: {
    title: string;
    description: string;
    sections: TemplateSection[];
  }) => void;
}) {
  if (isWebMode) {
    if (!selectedWebTemplate) {
      return <ResourceDetailEmpty message="Select a template to preview" />;
    }
    return (
      <WebTemplatePreview
        template={selectedWebTemplate}
        onClone={handleCloneTemplate}
      />
    );
  }

  if (!selectedMineId) {
    return <ResourceDetailEmpty message="Select a template to view details" />;
  }

  return (
    <TemplateForm
      key={selectedMineId}
      id={selectedMineId}
      handleDeleteTemplate={handleDeleteTemplate}
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
        onClone={() =>
          onClone({
            title: template.title ?? "",
            description: template.description ?? "",
            sections: template.sections ?? [],
          })
        }
      />

      <div className="flex-1 overflow-y-auto">
        <div className="p-6">
          <h3 className="mb-3 text-sm font-medium text-neutral-600">
            Sections
          </h3>
          <SectionsList
            disabled={true}
            items={template.sections ?? []}
            onChange={() => {}}
          />
        </div>
      </div>
    </div>
  );
}
