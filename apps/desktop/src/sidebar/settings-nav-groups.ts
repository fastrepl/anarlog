import { useLingui } from "@lingui/react/macro";

import {
  ArrowsClockwise,
  Bell,
  BookOpen,
  Buildings,
  CalendarDots,
  ChartLineUp,
  Code,
  CreditCard,
  DownloadSimple,
  FileText,
  FolderSimple,
  Gear,
  Lightning,
  type Icon,
  Lock,
  Microphone,
  ShieldCheck,
  Sparkle,
  Sun,
  User,
  Users,
  UsersThree,
  VideoCamera,
  Waveform,
} from "@anlg/ui/components/icons";

import { privacyMessages } from "~/settings/general/app-settings";
import { useMyWorkspacesWithMirror } from "~/settings/team/mirror";
import { type SettingsTab, type TabInput } from "~/store/zustand/tabs";

type SettingsNavItem =
  | {
      id: SettingsTab;
      label: string;
      icon: Icon;
      requiresPro?: boolean;
      searchTerms?: string[];
    }
  | {
      id: "automations" | "calendar" | "contacts" | "folders" | "templates";
      label: string;
      icon: Icon;
      destination: TabInput;
      requiresPro?: boolean;
      searchTerms?: string[];
    };

export type SettingsNavGroup = { label: string; items: SettingsNavItem[] };

export function useSettingsNavGroups(): SettingsNavGroup[] {
  const { i18n, t } = useLingui();
  const workspaces = useMyWorkspacesWithMirror();
  const hasExistingWorkspace = (workspaces.data?.length ?? 0) > 0;

  return [
    {
      label: t`App`,
      items: [
        {
          id: "app",
          label: t`General`,
          icon: Gear,
          searchTerms: [
            t`Start Anarlog at login`,
            t`Automatically install updates`,
            t`Show release notes`,
            t`Show app in Dock`,
            t`Show tray icon`,
            t`Language & Region`,
            t`Main language`,
            t`Use main language for summaries`,
            t`Additional spoken languages`,
            t`Timezone`,
            t`Week starts on`,
            t`Storage`,
            t`Export location`,
          ],
        },
        {
          id: "account",
          label: t`Account`,
          icon: User,
          searchTerms: [
            t`Your info`,
            t`Connected accounts`,
            t`Manage sign-in methods`,
            t`Sign out`,
          ],
        },
        {
          id: "billing",
          label: t`Billing`,
          icon: CreditCard,
          searchTerms: [t`Your plan`, t`Manage billing`, t`Add payment method`],
        },
        {
          id: "referrals",
          label: t`Invite friends`,
          icon: Users,
          searchTerms: [t`Free months earned`],
        },
        {
          id: "insights",
          label: t`Insights`,
          icon: ChartLineUp,
          searchTerms: [t`Hours transcribed`, t`Daily conversations`],
        },
        {
          id: "team",
          label: t`Teams`,
          icon: UsersThree,
          searchTerms: [t`Members`, t`Join automatically with a work email`],
          requiresPro: !workspaces.isLoading && !hasExistingWorkspace,
        },
        {
          id: "sync",
          label: t`Sync`,
          icon: ArrowsClockwise,
          searchTerms: [
            t`Cloud sync`,
            t`Devices`,
            t`Sync now`,
            t`Use recovery key instead`,
          ],
          requiresPro: true,
        },
        {
          id: "appearance",
          label: t`Appearance`,
          icon: Sun,
          searchTerms: [
            t`Theme`,
            t`Light`,
            t`Dark`,
            t`System`,
            t`Use 24-hour time`,
            t`App icon`,
            t`Notes list`,
            t`Folder`,
            t`Tags`,
          ],
        },
        {
          id: "notifications",
          label: t`Notifications`,
          icon: Bell,
          searchTerms: [
            t`Disable all notifications`,
            t`Summary complete`,
            t`Cloud sync complete`,
            t`Event notifications`,
            t`Recording status prompts`,
            t`Completion sound`,
            t`Bounce app icon`,
            t`Microphone detection`,
            t`Detection delay`,
            t`Exclude apps from detection`,
            t`Respect Do-Not-Disturb mode`,
          ],
        },
      ],
    },
    {
      label: "AI",
      items: [
        {
          id: "transcription",
          label: t`Transcription`,
          icon: Waveform,
          searchTerms: [t`Configure Providers`, t`API Key`],
        },
        {
          id: "dictation",
          label: t`Dictation`,
          icon: Microphone,
          searchTerms: [
            t`Enable dictation`,
            t`Microphone`,
            t`Live transcript preview`,
            t`Hands-free dictation`,
            t`Click here, then use your dictation shortcut.`,
            t`Last dictation`,
          ],
          requiresPro: true,
        },
        {
          id: "intelligence",
          label: t`Intelligence`,
          icon: Sparkle,
          searchTerms: [t`Configure Providers`, t`API Key`],
        },
        {
          id: "dictionary",
          label: t`Dictionary`,
          icon: BookOpen,
          searchTerms: [t`Add names, jargon, or product terms to prefer`],
          requiresPro: true,
        },
      ],
    },
    {
      label: t`Workspace`,
      items: [
        {
          id: "meetings",
          label: t`Meetings`,
          icon: VideoCamera,
          searchTerms: [
            t`Default sharing`,
            t`Primary recording device`,
            t`Start when meeting begins`,
            t`Join scheduled meetings`,
            t`Stop when meeting ends`,
            t`Post recording disclosure in meeting chat`,
            t`Capture meeting chat in Memos`,
            t`Capture shared screens in Memos`,
            t`Show floating bar`,
            t`Microphone`,
            t`Audio file retention`,
            t`Choose how long recordings stay on this device.`,
            t`Remember speakers`,
          ],
        },
        {
          id: "folders",
          label: t`Folders`,
          icon: FolderSimple,
          destination: { type: "folders" },
        },
        {
          id: "calendar",
          label: t`Calendar`,
          icon: CalendarDots,
          destination: { type: "calendar" },
        },
        {
          id: "contacts",
          label: t`Contacts`,
          icon: Users,
          destination: { type: "contacts" },
        },
        {
          id: "templates",
          label: t`Templates`,
          icon: FileText,
          destination: { type: "templates" },
        },
        {
          id: "automations",
          label: t`Automations`,
          icon: Lightning,
          destination: { type: "automations" },
          requiresPro: true,
        },
      ],
    },
    {
      label: t`Data`,
      items: [
        { id: "imports", label: t`Imports`, icon: DownloadSimple },
        { id: "crm", label: t`CRM`, icon: Buildings },
      ],
    },
    {
      label: t`Advanced`,
      items: [
        {
          id: "privacy",
          label: i18n._(privacyMessages.title),
          icon: ShieldCheck,
          searchTerms: [
            t`Lock app`,
            t`Share usage data`,
            t`Send sanitized crash and error reports to help improve Anarlog.`,
          ],
        },
        {
          id: "permissions",
          label: t`Permissions`,
          icon: Lock,
          searchTerms: [
            t`Microphone`,
            t`System audio`,
            t`Accessibility`,
            t`Calendar`,
          ],
        },
        {
          id: "developers",
          label: t`Developers`,
          icon: Code,
          searchTerms: [
            t`CLI & MCP`,
            t`Anarlog CLI`,
            t`MCP server`,
            t`Agent skills`,
            t`Cloud API & Connectors`,
            t`Cloud API keys`,
            t`Webhooks`,
          ],
        },
      ],
    },
  ];
}
