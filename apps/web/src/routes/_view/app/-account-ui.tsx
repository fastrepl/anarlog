import { cn } from "@anlg/utils";

export const accountCardClassName = cn([
  "overflow-hidden rounded-[24px] border border-[#e5ddcf] bg-white",
  "shadow-[0_18px_50px_rgba(24,22,19,0.08)]",
]);

export const accountPillPrimaryClassName = cn([
  "flex h-9 cursor-pointer items-center justify-center rounded-full px-4",
  "bg-[#181613] text-sm font-medium text-white",
  "transition-colors hover:bg-[#4f4940]",
  "disabled:cursor-not-allowed disabled:opacity-50",
]);

export const accountPillSecondaryClassName = cn([
  "flex h-9 cursor-pointer items-center justify-center rounded-full border border-[#d9d1c5] bg-white px-4",
  "text-sm font-medium text-[#181613]",
  "transition-colors hover:bg-[#f7f4ef]",
  "disabled:cursor-not-allowed disabled:opacity-50",
]);

export const accountPillDangerClassName = cn([
  "flex h-9 cursor-pointer items-center justify-center rounded-full border border-red-200 bg-white px-4",
  "text-sm font-medium text-red-700",
  "transition-colors hover:border-red-300 hover:text-red-800",
  "disabled:cursor-not-allowed disabled:opacity-50",
]);
