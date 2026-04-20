import { cn } from "@hypr/utils";

export function TrafficLightsView({
  className,
  onClose,
  onMinimize,
  onMaximize,
}: {
  className?: string;
  onClose?: () => void;
  onMinimize?: () => void;
  onMaximize?: () => void;
}) {
  return (
    <div className={cn(["flex items-center gap-2", className])}>
      <button
        type="button"
        onClick={onClose}
        className="h-3 w-3 rounded-full border border-black/10 bg-[#ff5f57] transition-all hover:brightness-90"
      />
      <button
        type="button"
        onClick={onMinimize}
        className="h-3 w-3 rounded-full border border-black/10 bg-[#ffbd2e] transition-all hover:brightness-90"
      />
      <button
        type="button"
        onClick={onMaximize}
        className="h-3 w-3 rounded-full border border-black/10 bg-[#28c840] transition-all hover:brightness-90"
      />
    </div>
  );
}
