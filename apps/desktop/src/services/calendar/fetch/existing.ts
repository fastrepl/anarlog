import type { Ctx } from "../ctx";
import { loadEventsForSync } from "../storage";
import type { ExistingEvent, IncomingEvent } from "./types";

export function fetchExistingEvents(
  ctx: Ctx,
  incoming: IncomingEvent[],
): Promise<ExistingEvent[]> {
  return loadEventsForSync(
    ctx,
    incoming.map((event) => event.tracking_id_event),
  );
}
