export function ResourceDetailEmpty({ message }: { message: string }) {
  return (
    <div className="flex h-full items-center justify-center">
      <p className="text-muted-foreground text-sm">{message}</p>
    </div>
  );
}
