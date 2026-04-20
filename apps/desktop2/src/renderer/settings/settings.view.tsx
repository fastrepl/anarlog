import type { ReactNode } from "react";

// Settings tab body. Today it's a single-column scroll of panels; this is
// the single place to add new settings sections (appearance, account, …).
export function SettingsView({ children }: { children: ReactNode }) {
  return (
    <div className="h-full overflow-y-auto px-10 py-8">
      <div className="mx-auto flex max-w-2xl flex-col gap-8">
        <header className="flex flex-col gap-1">
          <h1 className="font-serif text-2xl font-semibold">Settings</h1>
          <p className="text-sm text-neutral-500">
            Configure the Char desktop app.
          </p>
        </header>
        <div className="flex flex-col gap-8">{children}</div>
      </div>
    </div>
  );
}
