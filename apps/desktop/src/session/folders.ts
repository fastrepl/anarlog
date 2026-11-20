export {
  ancestorFolderPaths,
  childFolderPath,
  collectFolderPaths,
  folderDisplayName,
  folderPathMatchesFilter,
  normalizeFolderPath,
} from "@anlg/utils/folders";

// Literal containment, without normalizing or validating the path: a parent rename can make a
// stored descendant path longer than new paths may be, and it must still count as inside.
export function isInFolder(folderPath: string, folder: string): boolean {
  return (
    folderPath === folder ||
    folderPath.startsWith(`${folder}/`) ||
    folderPath.startsWith(`${folder}\\`)
  );
}
