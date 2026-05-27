export function ResourceDetailEmpty({ message }: { message: string }) {
  return (
    <div className="flex h-full items-center justify-center">
      <p className="text-sm text-neutral-500">{message}</p>
    </div>
  );
}
