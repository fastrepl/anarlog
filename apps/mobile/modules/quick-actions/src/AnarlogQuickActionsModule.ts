import { NativeModule, requireOptionalNativeModule } from "expo";

import type {
  AnarlogQuickActionsModuleEvents,
  QuickAction,
} from "./AnarlogQuickActions.types";

declare class AnarlogQuickActionsModule extends NativeModule<AnarlogQuickActionsModuleEvents> {
  consumePendingAction(): QuickAction | null;
}

export default requireOptionalNativeModule<AnarlogQuickActionsModule>(
  "AnarlogQuickActions",
);
