import { useConfigValue } from "~/shared/config";

export function useTimeFormat() {
  return useConfigValue("use_24_hour_time") ? "HH:mm" : "h:mm a";
}
