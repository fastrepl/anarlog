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
      className={className}
      data-compact={compact ? "true" : undefined}
    />
  );
}
