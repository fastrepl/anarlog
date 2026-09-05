import type { SVGProps } from "react";

import {
  ArrowCounterClockwise,
  ArrowsOutSimple,
  ArrowSquareOut,
  Check,
  CircleNotch,
  Copy,
  DownloadSimple,
  type Icon,
  MagnifyingGlassMinus,
  MagnifyingGlassPlus,
  X,
} from "./icons";

function markdownIcon(Component: Icon) {
  return ({
    strokeWidth,
    ...props
  }: SVGProps<SVGSVGElement> & { size?: number }) => {
    const width =
      typeof strokeWidth === "string"
        ? Number.parseFloat(strokeWidth)
        : strokeWidth;
    return (
      <Component
        {...props}
        strokeWidth={Number.isFinite(width) ? width : undefined}
      />
    );
  };
}

export const streamdownIcons = {
  CheckIcon: markdownIcon(Check),
  CopyIcon: markdownIcon(Copy),
  DownloadIcon: markdownIcon(DownloadSimple),
  ExternalLinkIcon: markdownIcon(ArrowSquareOut),
  Loader2Icon: markdownIcon(CircleNotch),
  Maximize2Icon: markdownIcon(ArrowsOutSimple),
  RotateCcwIcon: markdownIcon(ArrowCounterClockwise),
  XIcon: markdownIcon(X),
  ZoomInIcon: markdownIcon(MagnifyingGlassPlus),
  ZoomOutIcon: markdownIcon(MagnifyingGlassMinus),
};
