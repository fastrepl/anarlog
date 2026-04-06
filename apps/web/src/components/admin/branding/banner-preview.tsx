import { useCallback, useEffect, useRef, useState } from "react";

import { CanvasRegion } from "./canvas-region";
import type { PlatformPreset } from "./platforms";
import type { BannerState } from "./reducer";
import { templates } from "./templates";

export function BannerPreview({
  state,
  platform,
  canvasRef,
  onFieldUpdate,
  onFontSizeChange,
}: {
  state: BannerState;
  platform: PlatformPreset;
  canvasRef: React.RefObject<HTMLDivElement | null>;
  onFieldUpdate: (key: string, value: string) => void;
  onFontSizeChange: (key: string, size: number) => void;
}) {
  const wrapperRef = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(1);

  const recalcScale = useCallback(() => {
    const wrapper = wrapperRef.current;
    if (!wrapper) return;
    const padding = 48;
    const availW = wrapper.clientWidth - padding;
    const availH = wrapper.clientHeight - padding;
    const s = Math.min(availW / platform.width, availH / platform.height);
    setScale(Math.min(s, 1));
  }, [platform.width, platform.height]);

  useEffect(() => {
    recalcScale();
    const wrapper = wrapperRef.current;
    if (!wrapper) return;
    const ro = new ResizeObserver(recalcScale);
    ro.observe(wrapper);
    return () => ro.disconnect();
  }, [recalcScale]);

  const template = templates.find((t) => t.id === state.layoutId);
  if (!template) return null;

  const bgColor = state.values.bgColor || "#292524";
  const textColor = state.values.textColor || "#ffffff";
  const accentColor = state.values.accentColor;

  return (
    <div
      ref={wrapperRef}
      className="flex h-full w-full items-center justify-center bg-neutral-100"
    >
      <div
        style={{
          width: platform.width,
          height: platform.height,
          transform: `scale(${scale})`,
          transformOrigin: "center center",
        }}
      >
        <div
          ref={canvasRef}
          style={{
            width: platform.width,
            height: platform.height,
            backgroundColor: bgColor,
            position: "relative",
            overflow: "hidden",
            ...template.canvasStyle,
          }}
          className="[&[data-exporting]] [&[data-exporting]_[contenteditable]]:ring-0 [&[data-exporting]_[contenteditable]]:ring-offset-0"
        >
          {template.hasBgImage && state.values.bgImage && (
            <img
              src={state.values.bgImage}
              alt=""
              style={{
                position: "absolute",
                inset: 0,
                width: "100%",
                height: "100%",
                objectFit: "cover",
              }}
            />
          )}
          {template.hasBgImage && (
            <div
              style={{
                position: "absolute",
                inset: 0,
                backgroundColor: `rgba(0,0,0,${parseFloat(state.values.overlayOpacity || "0") || template.overlayOpacity || 0.45})`,
              }}
            />
          )}
          {template.regions.map((region) => {
            if (
              region.visibilityKey &&
              state.values[region.visibilityKey] === "false"
            ) {
              return null;
            }
            return (
              <CanvasRegion
                key={region.key}
                region={region}
                value={state.values[region.key] || ""}
                textColor={textColor}
                accentColor={accentColor}
                onUpdate={onFieldUpdate}
                onFontSizeChange={onFontSizeChange}
                fontSize={state.fontSizes[region.key]}
                values={state.values}
              />
            );
          })}
        </div>
      </div>
    </div>
  );
}
