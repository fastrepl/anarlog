export function StandaloneWindowShell({
  children,
  topDragRegion = true,
}: {
  children: React.ReactNode;
  topDragRegion?: boolean;
}) {
  return (
    <div className="relative flex h-full flex-col">
      {topDragRegion ? (
        <div
          data-tauri-drag-region
          data-standalone-window-top-drag-region
          className="absolute inset-x-0 top-0 z-20 h-10"
        />
      ) : null}
      {children}
    </div>
  );
}
