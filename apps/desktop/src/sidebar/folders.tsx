import { FoldersSidebar } from "~/folders/sidebar";

export function FoldersNav() {
  return (
    <div className="flex h-full flex-col overflow-hidden pb-2">
      <FoldersSidebar />
    </div>
  );
}
