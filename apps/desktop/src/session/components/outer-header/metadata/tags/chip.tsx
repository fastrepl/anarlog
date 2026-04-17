import { X } from "lucide-react";

import { Badge } from "@hypr/ui/components/ui/badge";
import { Button } from "@hypr/ui/components/ui/button";

import {
  useSessionTagMutations,
  useTagMappingCell,
  useTagName,
} from "~/session/hooks/storage";

export function TagChip({ mappingId }: { mappingId: string }) {
  const tagId = useTagMappingCell(mappingId, "tag_id");
  const tagName = useTagName(tagId);
  const { deleteTagMapping } = useSessionTagMutations();

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
        onClick={() => deleteTagMapping(mappingId)}
      >
        <X className="h-2.5 w-2.5" />
      </Button>
    </Badge>
  );
}
