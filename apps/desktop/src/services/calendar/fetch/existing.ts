import type { Ctx } from "../ctx";
import { eventTrackingIds } from "../process/events/identity";
import { loadEventsForSync } from "../storage";
import type { ExistingEvent, IncomingEvent } from "./types";

export function fetchExistingEvents(
  ctx: Ctx,
  incoming: IncomingEvent[],
): Promise<ExistingEvent[]> {
  return loadEventsForSync(ctx, incoming.flatMap(eventTrackingIds));
}
