import { prepare, layout } from "@chenglou/pretext";
import { useCallback, useEffect, useRef, useState } from "react";

import type { Region } from "./templates";

function CharLogoSvg({ color, height }: { color: string; height: string }) {
  return (
    <svg
      height={height}
      viewBox="0 0 103 30"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <path
        d="M7.871 4.147C7.871 5.658 7.082 7.039 6.099 8.214C4.65 9.946 3.77 12.161 3.77 14.575C3.77 16.99 4.65 19.205 6.099 20.937C7.082 22.112 7.871 23.493 7.871 25.004V29.151H2.965V24.319C2.965 22.735 2.165 21.249 0.822 20.34L0 19.783V9.235L0.822 8.678C2.165 7.769 2.965 6.284 2.965 4.699V0L7.871 0V4.147Z"
        fill={color}
      />
      <path
        d="M94.746 4.147C94.746 5.658 95.535 7.039 96.519 8.214C97.967 9.946 98.847 12.161 98.847 14.575C98.847 16.99 97.967 19.205 96.519 20.937C95.535 22.112 94.746 23.493 94.746 25.004V29.151H99.653V24.319C99.653 22.735 100.452 21.249 101.795 20.34L102.617 19.783V9.235L101.795 8.678C100.452 7.769 99.653 6.284 99.653 4.699V0L94.746 0V4.147Z"
        fill={color}
      />
      <path
        d="M90.369 4.536H86.669C84.596 4.536 82.721 5.667 81.73 7.429V4.536H73.026V8.029H78.244V20.821H73.026V24.313H90.311V20.821H82.425V12.447C82.425 10.262 84.191 8.494 86.365 8.494H90.369V4.536Z"
        fill={color}
      />
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        d="M60.901 4.071C63.781 4.071 66.142 5.182 67.798 6.995V4.536H71.284V24.313H67.798V21.805C66.128 23.645 63.753 24.778 60.901 24.778C55.064 24.778 51.331 20.074 51.331 14.425C51.331 11.606 52.225 9.021 53.882 7.131C55.546 5.235 57.954 4.071 60.901 4.071ZM61.365 7.912C59.5 7.912 58.023 8.638 57.005 9.793C55.981 10.956 55.396 12.586 55.396 14.425C55.396 18.088 57.776 20.937 61.365 20.937C64.954 20.937 67.334 18.088 67.334 14.425C67.334 12.586 66.749 10.956 65.725 9.793C64.708 8.638 63.231 7.912 61.365 7.912Z"
        fill={color}
      />
      <path
        d="M49.589 12.098C49.589 7.924 46.214 4.536 42.048 4.536H41.195C39.142 4.536 36.977 5.657 35.905 7.463V0H32.188V24.313H36.369V12.447C36.369 11.405 36.912 10.422 37.78 9.684C38.648 8.944 39.793 8.494 40.891 8.494H41.06C43.345 8.494 45.407 10.359 45.407 12.564V24.313H49.589V12.098Z"
        fill={color}
      />
      <path
        d="M26.243 17.328C25.77 19.561 23.754 21.053 20.995 21.053C17.296 21.053 14.852 18.146 14.852 14.425C14.852 12.556 15.453 10.897 16.506 9.713C17.552 8.536 19.074 7.796 20.995 7.796C23.793 7.796 25.772 9.443 26.26 11.533L26.365 11.983H30.559L30.436 11.297C29.689 7.153 26.043 4.071 20.995 4.071C17.864 4.071 15.3 5.224 13.522 7.117C11.749 9.005 10.787 11.595 10.787 14.425C10.787 20.113 14.807 24.778 20.995 24.778C25.907 24.778 29.753 22.074 30.427 17.535L30.527 16.866H26.341L26.243 17.328Z"
        fill={color}
      />
    </svg>
  );
}

function useTextMeasure(
  text: string,
  font: string,
  lineHeight: number,
  maxWidth: number,
) {
  const [height, setHeight] = useState(0);
  useEffect(() => {
    if (!text || !maxWidth) return;
    try {
      const prepared = prepare(text, font);
      const result = layout(prepared, maxWidth, lineHeight);
      setHeight(result.height);
    } catch {
      // fallback: don't set height
    }
  }, [text, font, lineHeight, maxWidth]);
  return height;
}

function CharLogoCompactSvg({
  color,
  height,
}: {
  color: string;
  height: string;
}) {
  return (
    <svg
      height={height}
      viewBox="0 0 26 30"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <path
        d="M7.871 4.147C7.871 5.658 7.082 7.039 6.099 8.214C4.65 9.946 3.77 12.161 3.77 14.575C3.77 16.99 4.65 19.205 6.099 20.937C7.082 22.112 7.871 23.493 7.871 25.004V29.151H2.965V24.319C2.965 22.735 2.165 21.249 0.822 20.34L0 19.783V9.235L0.822 8.678C2.165 7.769 2.965 6.284 2.965 4.699V0L7.871 0V4.147Z"
        fill={color}
      />
      <g transform="translate(-76.875, 0)">
        <path
          d="M94.746 4.147C94.746 5.658 95.535 7.039 96.519 8.214C97.967 9.946 98.847 12.161 98.847 14.575C98.847 16.99 97.967 19.205 96.519 20.937C95.535 22.112 94.746 23.493 94.746 25.004V29.151H99.653V24.319C99.653 22.735 100.452 21.249 101.795 20.34L102.617 19.783V9.235L101.795 8.678C100.452 7.769 99.653 6.284 99.653 4.699V0L94.746 0V4.147Z"
          fill={color}
        />
      </g>
    </svg>
  );
}

export function CanvasRegion({
  region,
  value,
  textColor,
  accentColor,
  onUpdate,
  onFontSizeChange,
  fontSize,
  values,
}: {
  region: Region;
  value: string;
  textColor: string;
  accentColor?: string;
  onUpdate: (key: string, value: string) => void;
  onFontSizeChange?: (key: string, size: number) => void;
  fontSize?: number;
  values?: Record<string, string>;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [hovered, setHovered] = useState(false);
  const [editing, setEditing] = useState(false);
  const resizing = useRef(false);

  const currentFontSize =
    fontSize || parseFloat(region.style.fontSize as string) || 24;

  const handleBlur = useCallback(() => {
    setEditing(false);
    const el = ref.current;
    if (!el) return;
    const text = el.textContent || "";
    if (text !== value) {
      onUpdate(region.key, text);
    }
  }, [region.key, value, onUpdate]);

  const resizeMode = region.resize;
  const canResize = resizeMode !== false && resizeMode !== undefined;

  const handleResizeStart = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      resizing.current = true;
      const startY = e.clientY;
      const startSize = currentFontSize;
      // top-right: drag up = bigger (negative delta = grow)
      // bottom-right: drag down = bigger (positive delta = grow)
      const direction = resizeMode === "top-right" ? -1 : 1;

      const onMove = (me: MouseEvent) => {
        if (!resizing.current) return;
        const delta = me.clientY - startY;
        const newSize = Math.max(
          10,
          Math.min(200, startSize + delta * direction * 0.5),
        );
        onFontSizeChange?.(region.key, Math.round(newSize));
      };

      const onUp = () => {
        resizing.current = false;
        document.removeEventListener("mousemove", onMove);
        document.removeEventListener("mouseup", onUp);
      };

      document.addEventListener("mousemove", onMove);
      document.addEventListener("mouseup", onUp);
    },
    [currentFontSize, onFontSizeChange, region.key],
  );

  const fontStr = `${region.style.fontWeight || 400} ${currentFontSize}px ${region.style.fontFamily || "sans-serif"}`;
  const lineHeightNum =
    parseFloat((region.style.lineHeight as string) || "1.3") * currentFontSize;
  const _measuredHeight = useTextMeasure(
    value,
    fontStr,
    lineHeightNum,
    ref.current?.clientWidth || 500,
  );

  if (region.type === "logo") {
    const variant = values?.logoVariant || "full";
    const LogoComponent =
      variant === "compact" ? CharLogoCompactSvg : CharLogoSvg;
    return (
      <div
        style={region.style}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
      >
        <div
          style={{
            position: "relative",
            borderRadius: "4px",
            outline: hovered ? `2px solid ${textColor}30` : "none",
            outlineOffset: "4px",
            transition: "outline 0.15s",
          }}
        >
          <LogoComponent
            color={textColor}
            height={(region.style.height as string) || "48px"}
          />
        </div>
      </div>
    );
  }

  if (region.type === "avatar") {
    const src = values?.[region.key];
    return (
      <div
        style={region.style}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
      >
        {src ? (
          <img
            src={src}
            alt=""
            style={{
              width: "100%",
              height: "100%",
              borderRadius: "50%",
              objectFit: "cover",
            }}
          />
        ) : (
          <div
            style={{
              width: "100%",
              height: "100%",
              borderRadius: "50%",
              backgroundColor: `${textColor}15`,
              border: `1px solid ${textColor}20`,
            }}
          />
        )}
      </div>
    );
  }

  if (
    region.type === "shape" &&
    !region.editable &&
    region.key !== "quotemark"
  ) {
    return <div style={region.style} />;
  }

  if (region.type === "author-block") {
    const avatarSrc = values?.avatar;
    const authorName = values?.author || "";
    const authorRole = values?.role || "";
    return (
      <div style={region.style}>
        {avatarSrc ? (
          <img
            src={avatarSrc}
            alt=""
            style={{
              width: "56px",
              height: "56px",
              borderRadius: "50%",
              objectFit: "cover",
              flexShrink: 0,
            }}
          />
        ) : (
          <div
            style={{
              width: "56px",
              height: "56px",
              borderRadius: "50%",
              backgroundColor: `${textColor}15`,
              border: `1px solid ${textColor}20`,
              flexShrink: 0,
            }}
          />
        )}
        <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
          <div
            contentEditable
            suppressContentEditableWarning
            onBlur={(e) =>
              onUpdate("author", e.currentTarget.textContent || "")
            }
            style={{
              fontSize: "22px",
              fontWeight: "600",
              fontFamily: "Geist, system-ui, sans-serif",
              lineHeight: "1.2",
              color: textColor,
              outline: "none",
            }}
          >
            {authorName}
          </div>
          <div
            contentEditable
            suppressContentEditableWarning
            onBlur={(e) => onUpdate("role", e.currentTarget.textContent || "")}
            style={{
              fontSize: "18px",
              fontWeight: "400",
              fontFamily: "Geist, system-ui, sans-serif",
              lineHeight: "1.2",
              color: textColor,
              opacity: 0.4,
              outline: "none",
            }}
          >
            {authorRole}
          </div>
        </div>
      </div>
    );
  }

  if (region.type === "shape" && region.key === "quotemark") {
    return (
      <div
        style={{
          ...region.style,
          color: textColor,
          userSelect: "none",
        }}
      >
        {"\u201C"}
      </div>
    );
  }

  const regionColor =
    region.key === "label" && accentColor ? accentColor : textColor;
  const showControls = hovered || editing;

  return (
    <div
      style={{
        ...region.style,
        fontSize: `${currentFontSize}px`,
        color: regionColor,
        outline: "none",
        cursor: region.editable ? "text" : "default",
        position: "absolute",
      }}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => {
        if (!editing) setHovered(false);
      }}
    >
      <div
        style={{
          position: "relative",
          borderRadius: "6px",
          outline: showControls ? `2px solid rgba(255,255,255,0.35)` : "none",
          outlineOffset: "6px",
          transition: "outline 0.15s",
          padding: "2px",
        }}
      >
        <div
          ref={ref}
          contentEditable={region.editable}
          suppressContentEditableWarning
          onFocus={() => setEditing(true)}
          onBlur={handleBlur}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              (e.target as HTMLElement).blur();
            }
          }}
          style={{ outline: "none" }}
        >
          {value}
        </div>

        {showControls && canResize && onFontSizeChange && (
          <div
            onMouseDown={handleResizeStart}
            style={{
              position: "absolute",
              ...(resizeMode === "top-right"
                ? { top: "-10px", right: "-10px" }
                : { bottom: "-10px", right: "-10px" }),
              width: "12px",
              height: "12px",
              borderRadius: "3px",
              backgroundColor: "rgba(255,255,255,0.8)",
              border: "1px solid rgba(0,0,0,0.15)",
              cursor: "ns-resize",
              transition: "opacity 0.15s",
            }}
          />
        )}
      </div>
    </div>
  );
}
