import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

export function AnarlogLogo({
  compact,
  sx,
}: {
  compact?: boolean;
} & StyleXProps) {
  return (
    <img
      {...mergeStyleXProps(sx)}
      src="/logo.svg"
      alt="Anarlog"
      width={1205}
      height={334}
      data-compact={compact ? "true" : undefined}
    />
  );
}
