import { createFileRoute } from "@tanstack/react-router";

import { ComposerScreen } from "~/composer";
import { ClassicMainLayout } from "~/main/layout";

export const Route = createFileRoute("/app/composer")({
  component: Component,
});

function Component() {
  return (
    <ClassicMainLayout includeServices={false}>
      <ComposerScreen />
    </ClassicMainLayout>
  );
}
