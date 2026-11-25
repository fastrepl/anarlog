import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  type CameraBackground,
  type CameraPrefs,
  type CameraScene,
  type CameraWatermarkState,
  commands as cameraCommands,
  type Result,
} from "@anlg/plugin-camera-watermark";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";

import { CAMERA_APP_NAMES } from "~/services/camera-guidance";
import {
  SETTING_CONTROL_CLASS,
  SettingRow,
  SettingSwitchRow,
} from "~/settings/setting-row";

const CAMERA_STATE_QUERY_KEY = ["camera-watermark", "state"] as const;

function unwrap<T>(result: Result<T, string>) {
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

function backgroundValue(background: CameraBackground) {
  return background.kind === "scene" ? background.scene : background.kind;
}

function backgroundFromValue(value: string): CameraBackground {
  if (value === "none" || value === "blur") {
    return { kind: value };
  }
  return { kind: "scene", scene: value as CameraScene };
}

export function CameraWatermarkSettings() {
  const { t } = useLingui();
  const queryClient = useQueryClient();
  const stateQuery = useQuery({
    queryKey: CAMERA_STATE_QUERY_KEY,
    queryFn: () => cameraCommands.getState(),
  });
  const setState = (state: CameraWatermarkState) =>
    queryClient.setQueryData(CAMERA_STATE_QUERY_KEY, state);

  const savePrefs = useMutation({
    mutationFn: async (change: Partial<CameraPrefs>) => {
      if (change.enabled !== undefined) {
        return unwrap(await cameraCommands.setEnabled(change.enabled));
      }
      if (change.auto_switch !== undefined) {
        return unwrap(await cameraCommands.setAutoSwitch(change.auto_switch));
      }
      if (change.background !== undefined) {
        return unwrap(await cameraCommands.setBackground(change.background));
      }
      throw new Error("No camera setting to change");
    },
    onMutate: async (change) => {
      await queryClient.cancelQueries({ queryKey: CAMERA_STATE_QUERY_KEY });
      const previous = queryClient.getQueryData<CameraWatermarkState>(
        CAMERA_STATE_QUERY_KEY,
      );
      if (previous) {
        setState({ ...previous, prefs: { ...previous.prefs, ...change } });
      }
      return { previous };
    },
    onError: (_error, _change, context) => {
      if (context?.previous) {
        setState(context.previous);
      }
    },
    onSuccess: setState,
  });
  const addExtension = useMutation({
    mutationFn: () => cameraCommands.addExtension(),
    onSuccess: setState,
  });
  const allowCamera = useMutation({
    mutationFn: () => cameraCommands.allowCamera(),
    onSuccess: setState,
  });
  const openExtensionSettings = useMutation({
    mutationFn: async () =>
      unwrap(await cameraCommands.openExtensionSettings()),
  });

  const state = stateQuery.data;
  if (!state || state.availability.status === "unavailable") {
    return null;
  }

  const available = state.availability.status === "available";
  const { enabled, auto_switch: autoSwitch, background } = state.prefs;
  const switchApps = state.automation.switches_camera
    .map((app) => CAMERA_APP_NAMES[app])
    .join(", ");
  const extensionFailure = state.extension_failure ?? t`unknown error`;
  const error =
    savePrefs.error ??
    addExtension.error ??
    allowCamera.error ??
    openExtensionSettings.error;

  return (
    <div>
      <h2 className="mb-4 font-sans text-lg font-semibold">
        <Trans>Camera</Trans>
      </h2>
      <div className="flex flex-col gap-4">
        <SettingSwitchRow
          title={<Trans>Anarlog Camera</Trans>}
          description={
            !available ? <Trans>Not included in this build.</Trans> : undefined
          }
          checked={enabled}
          onChange={(value) => savePrefs.mutate({ enabled: value })}
          disabled={!available}
        />
        {available && enabled && state.extension !== "active" && (
          <SettingRow
            title={<Trans>Add Anarlog Camera to macOS</Trans>}
            description={
              state.extension === "needs_approval" ? (
                <Trans>
                  Approve Anarlog Camera in System Settings under Login Items
                  &amp; Extensions.
                </Trans>
              ) : state.extension === "failed" ? (
                <Trans>macOS couldn't add the camera: {extensionFailure}</Trans>
              ) : (
                <Trans>macOS asks you to approve the camera once.</Trans>
              )
            }
            controlWidth="content"
          >
            {(labelProps) =>
              state.extension === "needs_approval" ? (
                <Button
                  {...labelProps}
                  variant="outline"
                  onClick={() => openExtensionSettings.mutate()}
                >
                  <Trans>Open System Settings</Trans>
                </Button>
              ) : (
                <Button
                  {...labelProps}
                  variant="outline"
                  disabled={addExtension.isPending}
                  onClick={() => addExtension.mutate()}
                >
                  {addExtension.isPending ? (
                    <Trans>Adding…</Trans>
                  ) : (
                    <Trans>Add camera</Trans>
                  )}
                </Button>
              )
            }
          </SettingRow>
        )}
        {available && enabled && state.camera_access !== "allowed" && (
          <SettingRow
            title={<Trans>Camera access</Trans>}
            description={
              state.camera_access === "denied" ? (
                <Trans>
                  Turn on Anarlog Camera in System Settings under Privacy &amp;
                  Security, Camera.
                </Trans>
              ) : (
                <Trans>Anarlog Camera needs your camera to show you.</Trans>
              )
            }
            controlWidth="content"
          >
            {(labelProps) =>
              state.camera_access === "not_asked" ? (
                <Button
                  {...labelProps}
                  variant="outline"
                  disabled={allowCamera.isPending}
                  onClick={() => allowCamera.mutate()}
                >
                  <Trans>Allow camera</Trans>
                </Button>
              ) : null
            }
          </SettingRow>
        )}
        {available && switchApps && (
          <SettingSwitchRow
            title={<Trans>Switch my camera automatically</Trans>}
            description={
              <Trans>
                When Anarlog records a call in {switchApps}, switch its camera
                to Anarlog Camera.
              </Trans>
            }
            checked={enabled && autoSwitch}
            onChange={(value) => savePrefs.mutate({ auto_switch: value })}
            disabled={!enabled}
          />
        )}
        {available && (
          <SettingRow title={<Trans>Background</Trans>}>
            {(labelProps) => (
              <Select
                value={backgroundValue(background)}
                onValueChange={(value) =>
                  savePrefs.mutate({ background: backgroundFromValue(value) })
                }
                disabled={!enabled}
              >
                <SelectTrigger
                  {...labelProps}
                  className={SETTING_CONTROL_CLASS}
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="none">
                    <Trans>None</Trans>
                  </SelectItem>
                  <SelectItem value="blur">
                    <Trans>Blur</Trans>
                  </SelectItem>
                  <SelectItem value="studio">
                    <Trans>Studio</Trans>
                  </SelectItem>
                  <SelectItem value="dusk">
                    <Trans>Dusk</Trans>
                  </SelectItem>
                  <SelectItem value="daylight">
                    <Trans>Daylight</Trans>
                  </SelectItem>
                </SelectContent>
              </Select>
            )}
          </SettingRow>
        )}
        {error && (
          <p role="alert" className="text-xs text-red-500">
            {error.message}
          </p>
        )}
      </div>
    </div>
  );
}
