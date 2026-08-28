import { Trans, useLingui } from "@lingui/react/macro";
import { DotsThree, Heart, Plus } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useState } from "react";

import { colors } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";

import { AutoTemplateDetails } from "./auto-form";
import { type WebTemplate } from "./codec";
import { type UserTemplate, type UserTemplateDraft } from "./queries";
import { SectionsList } from "./sections-editor";
import { TemplateForm } from "./template-form";
import { TemplateIconGlyph } from "./template-icon";
import { getTemplateCreatorLabel } from "./utils";

import {
  ResourceDetailEmpty,
  ResourcePreviewHeader,
} from "~/shared/ui/resource-list";

export function TemplateDetailsColumn({
  isAutoSelected,
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
  isAutoSelected: boolean;
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
  const { t } = useLingui();
  if (isAutoSelected) {
    return <AutoTemplateDetails />;
  }

  if (isWebMode) {
    if (!selectedWebTemplate) {
      return (
        <ResourceDetailEmpty message={t`No community templates available`} />
      );
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
    <div {...stylex.props(styles.empty)}>
      <p {...stylex.props(styles.emptyText)}>
        <Trans>No templates yet</Trans>
      </p>
      <Button
        type="button"
        size="sm"
        variant="outline"
        onClick={onCreate}
        sx={styles.createButton}
      >
        <Plus {...stylex.props(styles.icon)} />
        <Trans>Create template</Trans>
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
  const { t } = useLingui();
  const nextTemplate: UserTemplateDraft = {
    title: template.title ?? "",
    description: template.description ?? "",
    category: template.category,
    icon: template.icon,
    targets: template.targets,
    sections: template.sections ?? [],
  };
  const [actionsOpen, setActionsOpen] = useState(false);

  return (
    <div {...stylex.props(styles.root)}>
      <ResourcePreviewHeader
        icon={
          <TemplateIconGlyph icon={template.icon} sx={styles.templateIcon} />
        }
        title={template.title || t`Untitled`}
        description={template.description}
        targets={template.targets}
        titleMeta={
          <span {...stylex.props(styles.creator)}>
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
              sx={styles.setDefaultButton}
            >
              <Trans>Set as default</Trans>
            </Button>
            <Button
              type="button"
              size="icon"
              variant="ghost"
              onClick={() => onFavorite(nextTemplate)}
              sx={styles.actionButton}
              title={t`Favorite template`}
              aria-label={t`Favorite template`}
            >
              <Heart {...stylex.props(styles.icon)} />
            </Button>
            <DropdownMenu open={actionsOpen} onOpenChange={setActionsOpen}>
              <DropdownMenuTrigger asChild>
                <Button
                  type="button"
                  size="icon"
                  variant="ghost"
                  sx={[
                    styles.actionButton,
                    actionsOpen && styles.openActionButton,
                  ]}
                  aria-label={t`Template actions`}
                >
                  <DotsThree {...stylex.props(styles.icon)} />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent variant="app" align="end">
                <AppFloatingPanel sx={styles.menuPanel}>
                  <DropdownMenuItem
                    onClick={() => onClone(nextTemplate)}
                    sx={styles.menuItem}
                  >
                    <Trans>Duplicate</Trans>
                  </DropdownMenuItem>
                </AppFloatingPanel>
              </DropdownMenuContent>
            </DropdownMenu>
          </>
        }
      >
        <div {...stylex.props(styles.sections)}>
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

const styles = stylex.create({
  actionButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
  },
  createButton: {
    gap: "0.5rem",
  },
  creator: {
    color: colors.mutedForeground,
    flexShrink: 0,
    fontSize: "0.75rem",
    fontWeight: 400,
    lineHeight: "1rem",
    whiteSpace: "nowrap",
  },
  empty: {
    alignItems: "center",
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
    height: "100%",
    justifyContent: "center",
  },
  emptyText: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  menuItem: {
    cursor: "pointer",
  },
  menuPanel: {
    overflow: "hidden",
    padding: "0.25rem",
  },
  openActionButton: {
    backgroundColor: {
      default: colors.muted,
      ":hover": colors.accent,
    },
    color: colors.foreground,
  },
  root: {
    display: "flex",
    flex: "1",
    flexDirection: "column",
    height: "100%",
  },
  sections: {
    marginTop: "1.5rem",
  },
  setDefaultButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": "black",
    },
    flexShrink: 0,
  },
  templateIcon: {
    fontSize: "0.875rem",
    height: "1rem",
    width: "1rem",
  },
});
