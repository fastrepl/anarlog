import type { ReactNode } from "react";

export function SettingsPageTitle({ title }: { title: ReactNode }) {
  return (
    <h2 className="text-[28px] leading-tight font-semibold tracking-tight">
      {title}
    </h2>
  );
}
