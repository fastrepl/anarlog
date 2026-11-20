import { t } from "@lingui/core/macro";

import {
  type CameraApp,
  type CameraGuidance,
  events as cameraEvents,
} from "@anlg/plugin-camera-watermark";
import { toast } from "@anlg/ui/components/ui/toast";
import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";

export const CAMERA_APP_NAMES: Record<CameraApp, string> = {
  zoom: "Zoom",
  teams: "Microsoft Teams",
  webex: "Webex",
  google_meet: "Google Meet",
};

function guidanceMessage({ kind, app: appId }: CameraGuidance) {
  const app = CAMERA_APP_NAMES[appId];
  switch (kind) {
    case "pick_camera":
      return t`Pick Anarlog Camera as your camera. Anarlog couldn't switch it in ${app}.`;
    case "check_background_first":
      return t`If ${app} blurs or replaces your background, that hides the badge. Keep it on until Anarlog Camera hides your room, then check your video before turning off ${app}'s background.`;
    case "turn_off_app_background":
      return t`Anarlog Camera is hiding your room now. Turn off ${app}'s blur or background: it hides the badge.`;
    case "app_background_left_on":
      return t`Anarlog left ${app}'s blur or background on because it couldn't confirm Anarlog Camera hides your room yet.`;
  }
}

// The camera plugin only emits structured guidance during a recording; the copy lives here.
export function CameraGuidanceToasts() {
  useMountEffect(() => {
    const unlisten = cameraEvents.cameraGuidanceEvent.listen(({ payload }) => {
      toast.warning(guidanceMessage(payload), {
        id: `camera-guidance:${payload.session_id}:${payload.kind}`,
      });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  });

  return null;
}
