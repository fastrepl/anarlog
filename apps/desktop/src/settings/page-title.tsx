import type { ReactNode } from "react";

export function SettingsPageTitle({ title }: { title: ReactNode }) {
  return (
    <h2 className="font-display text-3xl leading-tight font-semibold tracking-normal">
      {title}
    </h2>
  );
}
