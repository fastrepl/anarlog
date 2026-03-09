import type {
  HumanStorage,
  MappingSessionParticipantStorage,
} from "@hypr/store";

import type { Ctx } from "../../ctx";
import type { ParticipantsSyncOutput } from "./types";

import { id } from "~/shared/utils";

export function executeForParticipantsSync(
  ctx: Ctx,
  out: ParticipantsSyncOutput,
): void {
  const userId = ctx.store.getValue("user_id");
  if (!userId) {
    return;
  }

  ctx.store.transaction(() => {
    for (const human of out.humansToCreate) {
      ctx.store.setRow("humans", human.id, {
        user_id: String(userId),
        name: human.name,
        email: human.email,
        org_id: "",
        job_title: "",
        linkedin_username: "",
        memo: "",
        pinned: false,
      } satisfies HumanStorage);
    }

    for (const mappingId of out.toDelete) {
      ctx.store.delRow("mapping_session_participant", mappingId);
    }

    for (const mapping of out.toAdd) {
      ctx.store.setRow("mapping_session_participant", id(), {
        user_id: String(userId),
        session_id: mapping.sessionId,
        human_id: mapping.humanId,
        source: "auto",
      } satisfies MappingSessionParticipantStorage);
    }
  });
}
