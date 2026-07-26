import type { DeepLink } from "@hypr/plugin-deeplink2";

type CommandResult<T> =
  | { status: "ok"; data: T }
  | { status: "error"; error: string };

export async function subscribeThenDrainDeepLinks({
  listen,
  takePendingDeepLinks,
  handle,
}: {
  listen: (handler: (deepLink: DeepLink) => void) => Promise<() => void>;
  takePendingDeepLinks: () => Promise<CommandResult<DeepLink[]>>;
  handle: (deepLink: DeepLink) => void;
}) {
  const unlisten = await listen(handle);

  let result: CommandResult<DeepLink[]>;
  try {
    result = await takePendingDeepLinks();
  } catch {
    return unlisten;
  }
  if (result.status === "ok") {
    for (const deepLink of result.data) {
      handle(deepLink);
    }
  }
  return unlisten;
}
