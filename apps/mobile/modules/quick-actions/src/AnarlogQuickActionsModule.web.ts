import { NativeModule, registerWebModule } from "expo";

import type {
  AnarlogQuickActionsModuleEvents,
  QuickAction,
} from "./AnarlogQuickActions.types";

class AnarlogQuickActionsModule extends NativeModule<AnarlogQuickActionsModuleEvents> {
  consumePendingAction(): QuickAction | null {
    return null;
  }
}

export default registerWebModule(
  AnarlogQuickActionsModule,
  "AnarlogQuickActions",
);
