import { Trans } from "@lingui/react/macro";

import { FolderEditor } from "./folder-editor";
import { useActiveFolderPath } from "./selection";

import { useFolderPaths } from "~/session/queries";
import { StandardContentWrapper } from "~/shared/main";

export function TabContentFolders() {
  return (
    <StandardContentWrapper>
      <div className="h-full">
        <FoldersMain />
      </div>
    </StandardContentWrapper>
  );
}

export function FoldersMain() {
  const folders = useFolderPaths();
  const activeFolder = useActiveFolderPath(folders);

  if (!activeFolder) {
    return (
      <p className="text-muted-foreground flex h-full items-center justify-center px-6 text-sm">
        <Trans>No folders yet. Create one to group notes and materials.</Trans>
      </p>
    );
  }

  return <FolderEditor key={activeFolder} folderPath={activeFolder} />;
}
