import { X } from "lucide-react";

import { Badge } from "@hypr/ui/components/ui/badge";
import { Button } from "@hypr/ui/components/ui/button";

import * as main from "~/store/tinybase/store/main";

export function TagChip({ mappingId }: { mappingId: string }) {
  const store = main.UI.useStore(main.STORE_ID);
  const tagId = main.UI.useCell(
    "mapping_tag_session",
    mappingId,
    "tag_id",
    main.STORE_ID,
  ) as string | undefined;
  const tagName = main.UI.useCell(
    "tags",
    tagId ?? "",
    "name",
    main.STORE_ID,
  ) as string | undefined;

  if (!tagId || !tagName) {
    return null;
  }

  return (
    <Badge
      variant="secondary"
      className="bg-muted hover:bg-muted/80 flex items-center gap-1 px-2 py-0.5 text-xs"
    >
      #{tagName}
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className="ml-0.5 h-3 w-3 p-0 hover:bg-transparent"
        onClick={() => {
          store?.delRow("mapping_tag_session", mappingId);
        }}
      >
        <X className="h-2.5 w-2.5" />
      </Button>
    </Badge>
  );
}
