import { useMemo } from "react";

import * as main from "~/store/tinybase/store/main";

export function useSpeakerLabelContextVersion(
  sessionId: string | null | undefined,
) {
  const userId = main.UI.useValue("user_id", main.STORE_ID);
  const mappingIds = main.UI.useSliceRowIds(
    main.INDEXES.sessionParticipantsBySession,
    sessionId ?? "",
    main.STORE_ID,
  ) as string[];
  const mappingTable = main.UI.useTable(
    "mapping_session_participant",
    main.STORE_ID,
  );
  const humansTable = main.UI.useTable("humans", main.STORE_ID);

  return useMemo(() => {
    const participantHumanIds = mappingIds
      .map((mappingId) => mappingTable?.[mappingId]?.human_id)
      .filter(
        (humanId): humanId is string =>
          typeof humanId === "string" && humanId.length > 0,
      );

    return JSON.stringify({
      participants: [...new Set(participantHumanIds)].map((humanId) => [
        humanId,
        humansTable?.[humanId]?.name ?? null,
      ]),
      userId: typeof userId === "string" ? userId : null,
    });
  }, [humansTable, mappingIds, mappingTable, userId]);
}
