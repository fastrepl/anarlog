import { AnimatePresence, motion } from "motion/react";

import { cn } from "@hypr/utils";

interface NotificationBadgeProps {
  show: boolean;
  className?: string;
  size?: "sm" | "md" | "lg";
}

export function NotificationBadge({
  show,
  className,
  size = "sm",
}: NotificationBadgeProps) {
  const sizeClasses = {
    sm: "size-2",
    md: "size-3",
    lg: "size-4",
  };

  return (
    <AnimatePresence>
      {show && (
        <span className="absolute top-1 right-1 z-50">
          <span className="relative flex size-fit">
            <motion.span
              initial={{ scale: 0, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              exit={{ scale: 0, opacity: 0 }}
              transition={{ duration: 0.2, ease: "easeOut" }}
              className={cn([
                "absolute inline-flex h-full w-full animate-ping rounded-full bg-red-400 opacity-75",
              ])}
            />
            <motion.span
              initial={{ scale: 0, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              exit={{ scale: 0, opacity: 0 }}
              transition={{ duration: 0.2, ease: "easeOut" }}
              className={cn([
                "relative inline-flex rounded-full bg-red-500",
                sizeClasses[size],
                className,
              ])}
            />
          </span>
        </span>
      )}
    </AnimatePresence>
  );
}
