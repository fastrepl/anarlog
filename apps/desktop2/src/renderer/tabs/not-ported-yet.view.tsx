import { WrenchIcon } from "lucide-react";

import { getStubTabLabel, type StubTabType } from "~/tabs/tabs.types";

export function NotPortedYetView({
  kind,
  hint,
}: {
  kind: StubTabType;
  hint?: string;
}) {
  return (
    <div className="grid h-full place-content-center px-6">
      <div className="flex max-w-sm flex-col items-center gap-3 text-center">
        <div className="flex size-10 items-center justify-center rounded-full border border-neutral-200 bg-neutral-50 text-neutral-500">
          <WrenchIcon size={18} />
        </div>
        <div className="text-lg font-semibold text-neutral-900">
          {getStubTabLabel(kind, hint)}
        </div>
        <p className="text-sm text-neutral-500">
          This surface is not ported to `desktop2` yet.
        </p>
      </div>
    </div>
  );
}
