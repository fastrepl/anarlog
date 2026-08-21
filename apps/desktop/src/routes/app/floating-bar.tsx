import { createFileRoute } from "@tanstack/react-router";

import { FloatingBarOverlayScreen } from "~/meeting-float/overlay";

export const Route = createFileRoute("/app/floating-bar")({
  component: FloatingBarOverlayScreen,
});
