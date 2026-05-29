import { Facehash } from "facehash";

import { cn } from "@hypr/utils";

export function ProfileFacehash({
  name,
  size,
  showInitial = true,
  className,
}: {
  name: string;
  size: number;
  showInitial?: boolean;
  className?: string;
}) {
  return (
    <div className={cn(["rounded-full bg-amber-50", className])}>
      <Facehash
        name={name}
        size={size}
        interactive={false}
        showInitial={showInitial}
        variant="solid"
        intensity3d="none"
      />
    </div>
  );
}
