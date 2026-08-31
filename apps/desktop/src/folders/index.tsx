import { Trans } from "@lingui/react/macro";

import { cn } from "@anlg/utils";

import { FolderEditor } from "./folder-editor";
import { useActiveFolderPath } from "./selection";

import { useFolderPaths } from "~/session/queries";
import { SettingsPageTitle } from "~/settings/page-title";
import { StandardContentWrapper } from "~/shared/main";

export function TabContentFolders() {
  return (
    <StandardContentWrapper>
      <div className="bg-card dark:bg-accent flex w-full flex-1 flex-col overflow-hidden">
        <div className="relative w-full flex-1 overflow-hidden">
          <div
            className={cn([
              "scroll-fade-y scrollbar-hide h-full w-full flex-1 overflow-y-auto p-6",
            ])}
          >
            <FoldersMain />
          </div>
        </div>
      </div>
    </StandardContentWrapper>
  );
}

export function FoldersMain() {
  const folders = useFolderPaths();
  const activeFolder = useActiveFolderPath(folders);

  return (
    <div className="flex max-w-3xl flex-col gap-8">
      <div className="min-w-0">
        <SettingsPageTitle title={<Trans>Folders</Trans>} />
        <p className="text-muted-foreground mt-2 max-w-xl text-sm leading-5">
          <Trans>
            Chat follows these instructions for notes in the folder. Add a
            syllabus or other files as materials.
          </Trans>
        </p>
      </div>

      {activeFolder ? (
        <FolderEditor folderPath={activeFolder} />
      ) : (
        <p className="text-muted-foreground text-sm">
          <Trans>
            No folders yet. Create one to group notes and materials.
          </Trans>
        </p>
      )}
    </div>
  );
}
