export {
  getDefaultSttModel,
  getPreferredProviderModel,
} from "~/stt/model-selection";

export function resolveLiveLanguageSupportMode({
  isOnDeviceModel,
  useLiveOnDeviceModel,
  liveSupported,
}: {
  isOnDeviceModel: boolean;
  useLiveOnDeviceModel: boolean;
  liveSupported: boolean | undefined;
}): boolean | undefined {
  return isOnDeviceModel
    ? useLiveOnDeviceModel && liveSupported
    : liveSupported;
}
