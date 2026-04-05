import { ArrowUpIcon } from "lucide-react";

export function TodayButton({
  onClick,
  visible,
}: {
  onClick: () => void;
  visible: boolean;
}) {
  if (!visible) return null;

  return (
    <button
      onClick={onClick}
      className="absolute top-4 left-1/2 z-10 flex -translate-x-1/2 items-center gap-1.5 rounded-full bg-neutral-800 px-3 py-1.5 text-xs font-medium text-white shadow-lg transition-opacity hover:bg-neutral-700"
    >
      <ArrowUpIcon className="h-3 w-3" />
      today
    </button>
  );
}
