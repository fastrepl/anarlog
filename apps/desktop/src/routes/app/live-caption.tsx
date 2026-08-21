import { createFileRoute } from "@tanstack/react-router";

import { LiveCaptionOverlayScreen } from "~/meeting-float/overlay/live-caption";

export const Route = createFileRoute("/app/live-caption")({
  component: LiveCaptionOverlayScreen,
});
