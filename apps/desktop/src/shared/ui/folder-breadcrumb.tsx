import { type ReactNode, useMemo } from "react";

export function FolderBreadcrumb({
  folderId,
  renderBefore,
  renderAfter,
  renderSeparator,
  renderCrumb,
}: {
  folderId: string;
  renderBefore?: () => ReactNode;
  renderAfter?: () => ReactNode;
  renderSeparator?: (props: { index: number }) => ReactNode;
  renderCrumb: (props: {
    id: string;
    name: string;
    isLast: boolean;
  }) => ReactNode;
}) {
  const folderChain = useFolderChain(folderId);

  if (folderChain.length === 0) {
    return null;
  }

  return (
    <div className="flex flex-row items-center gap-1">
      {renderBefore?.()}
      {folderChain.map((id, index) => (
        <div key={id} className="flex flex-row items-center gap-1">
          {renderSeparator ? (
            renderSeparator({ index })
          ) : index > 0 || renderBefore ? (
            <span>/</span>
          ) : null}
          <FolderWrapper
            folderId={id}
            isLast={index === folderChain.length - 1}
          >
            {({ id, name, isLast }) => {
              return renderCrumb({ id, name, isLast });
            }}
          </FolderWrapper>
        </div>
      ))}
      {renderAfter?.()}
    </div>
  );
}

function FolderWrapper({
  folderId,
  isLast,
  children,
}: {
  folderId: string;
  isLast: boolean;
  children: (props: { id: string; name: string; isLast: boolean }) => ReactNode;
}) {
  const name = useMemo(() => {
    const parts = folderId.split("/");
    return parts[parts.length - 1] || "Untitled";
  }, [folderId]);

  return (
    <>
      {children({
        id: folderId,
        name,
        isLast,
      })}
    </>
  );
}

export function useFolderChain(folderId: string) {
  return useMemo(() => {
    if (!folderId) return [];
    const parts = folderId.split("/").filter(Boolean);
    return parts.map((_, i) => parts.slice(0, i + 1).join("/"));
  }, [folderId]);
}
