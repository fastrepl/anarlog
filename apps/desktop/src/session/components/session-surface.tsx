import { StandardTabWrapper } from "~/shared/main";

export function SessionSurface({
  header,
  children,
  floatingButton,
}: {
  header?: React.ReactNode;
  children: React.ReactNode;
  floatingButton?: React.ReactNode;
}) {
  return (
    <StandardTabWrapper floatingButton={floatingButton}>
      <div data-session-surface className="flex h-full flex-col">
        {header ? (
          <div data-tauri-drag-region className="px-1">
            {header}
          </div>
        ) : null}
        <div className="min-h-0 flex-1 px-2">{children}</div>
      </div>
    </StandardTabWrapper>
  );
}
