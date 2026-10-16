import { useFolderSelection } from "~/folders/selection";

export const FOLDERS_NAVIGATION_PATH = "/app/folders";

export function foldersNavigationEvent(folderPath: string) {
  return {
    path: FOLDERS_NAVIGATION_PATH,
    search: folderPath ? { path: folderPath } : null,
  };
}

export function openFoldersFromNavigation(
  payload: { path: string; search?: { path?: unknown } | null },
  openNew: (tab: { type: "folders" }) => void,
) {
  if (payload.path !== FOLDERS_NAVIGATION_PATH) {
    return false;
  }
  const folderPath = payload.search?.path;
  if (typeof folderPath === "string" && folderPath) {
    useFolderSelection.getState().setSelectedPath(folderPath);
  }
  openNew({ type: "folders" });
  return true;
}
