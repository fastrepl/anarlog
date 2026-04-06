import { createFileRoute } from "@tanstack/react-router";
import { useCallback, useReducer, useRef } from "react";

import { BannerPreview } from "@/components/admin/branding/banner-preview";
import { BannerSidebar } from "@/components/admin/branding/banner-sidebar";
import { platforms } from "@/components/admin/branding/platforms";
import {
  bannerReducer,
  createInitialState,
} from "@/components/admin/branding/reducer";

export const Route = createFileRoute("/admin/branding/")({
  component: BrandingPage,
});

function BrandingPage() {
  const [state, dispatch] = useReducer(
    bannerReducer,
    undefined,
    createInitialState,
  );
  const canvasRef = useRef<HTMLDivElement>(null);
  const platform =
    platforms.find((p) => p.id === state.platformId) || platforms[0];

  const handleFieldUpdate = useCallback((key: string, value: string) => {
    dispatch({ type: "SET_FIELD", key, value });
  }, []);

  const handleFontSizeChange = useCallback((key: string, size: number) => {
    dispatch({ type: "SET_FONT_SIZE", key, size });
  }, []);

  return (
    <div className="flex h-full">
      <div className="w-80 shrink-0 border-r border-neutral-200 bg-white">
        <BannerSidebar
          state={state}
          dispatch={dispatch}
          canvasRef={canvasRef}
        />
      </div>
      <div className="min-w-0 flex-1">
        <BannerPreview
          state={state}
          platform={platform}
          canvasRef={canvasRef}
          onFieldUpdate={handleFieldUpdate}
          onFontSizeChange={handleFontSizeChange}
        />
      </div>
    </div>
  );
}
