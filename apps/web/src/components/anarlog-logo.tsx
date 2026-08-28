export function AnarlogLogo({
  className,
  compact,
}: {
  className?: string;
  compact?: boolean;
}) {
  return (
    <img
      src="/logo.svg"
      alt="Anarlog"
      width={1205}
      height={334}
      className={className}
      data-compact={compact ? "true" : undefined}
    />
  );
}
