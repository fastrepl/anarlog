import type { ReactNode } from "react";

export function SettingsPageTitle({ title }: { title: ReactNode }) {
  return (
    <h2 className="font-hand text-3xl leading-none font-semibold tracking-normal">
      {title}
    </h2>
  );
}
