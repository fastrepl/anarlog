export function StandaloneWindowShell({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <div className="relative flex h-full flex-col">
      <div
        data-tauri-drag-region
        className="absolute inset-x-0 top-0 z-20 h-10"
      />
      {children}
    </div>
  );
}
