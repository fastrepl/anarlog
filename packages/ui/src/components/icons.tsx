import {
  AddCircleIcon,
  AddTeamIcon,
  Airplane01Icon,
  AlertCircleIcon,
  AppWindowIcon,
  ArrowDown01Icon,
  ArrowDown02Icon,
  ArrowExpandIcon,
  ArrowLeft01Icon,
  ArrowLeft02Icon,
  ArrowRight01Icon,
  ArrowRight02Icon,
  ArrowShrinkIcon,
  ArrowTurnBackwardIcon,
  ArrowTurnForwardIcon,
  ArrowUp01Icon,
  ArrowUp02Icon,
  ArrowUpDownIcon,
  ArrowUpRight02Icon,
  AudioWaveformIcon,
  BalanceScaleIcon,
  BanIcon,
  BankIcon,
  BellIcon,
  BookOpen01Icon,
  BookOpenTextIcon,
  BrainIcon,
  Briefcase01Icon,
  Bug01Icon,
  Building03Icon,
  Calendar03Icon,
  Calendar04Icon,
  CalendarOffIcon,
  Camera01Icon,
  CancelCircleIcon,
  ChartBarBigIcon,
  ChartLineData01Icon,
  CheckIcon,
  CheckmarkCircle02Icon,
  CircleIcon,
  CircleMinusIcon,
  ClipboardListIcon,
  Clock01Icon,
  CloudAlertIcon,
  CloudIcon,
  CloudOffIcon,
  CodeIcon,
  Coffee01Icon,
  CompassIcon,
  ComputerIcon,
  CopyIcon,
  CpuIcon,
  CreditCardIcon,
  CrownIcon,
  Cursor01Icon,
  DatabaseIcon,
  DiscordIcon,
  DollarSignIcon,
  Download01Icon,
  ElectricPlugsIcon,
  Exchange01Icon,
  ExternalLinkIcon,
  EyeIcon,
  EyeOffIcon,
  File01Icon,
  FileDownloadIcon,
  FileSpreadsheetIcon,
  FileTextIcon,
  FilterIcon,
  FireIcon,
  Flag01Icon,
  FloppyDiskIcon,
  Folder01Icon,
  FolderOpenIcon,
  GitMergeIcon,
  GitPullRequestIcon,
  GithubIcon,
  GlobeIcon,
  GraduationCapIcon,
  GripVerticalIcon,
  HammerIcon,
  HandshakeIcon,
  HardDriveIcon,
  Heading01Icon,
  Heading02Icon,
  Heading03Icon,
  HeadphonesIcon,
  HeadsetIcon,
  HeartIcon,
  HighlighterIcon,
  HistoryIcon,
  House01Icon,
  Image01Icon,
  InfoIcon,
  KanbanIcon,
  Key01Icon,
  Leaf01Icon,
  LeftToRightListBulletIcon,
  LeftToRightListNumberIcon,
  LightbulbIcon,
  LinkIcon,
  ListChecksIcon,
  LoaderCircleIcon,
  LockKeyIcon,
  LockKeyholeOpenIcon,
  LockPasswordIcon,
  Login03Icon,
  MagicWand01Icon,
  Mail01Icon,
  Mail02Icon,
  MailOpen01Icon,
  MapIcon,
  MapPinIcon,
  Megaphone01Icon,
  Message01Icon,
  MessageCircleIcon,
  MessageMultiple01Icon,
  MessageSquareMoreIcon,
  Mic01Icon,
  MicOff01Icon,
  MinusIcon,
  MoonIcon,
  MoreHorizontalIcon,
  MoreVerticalIcon,
  MusicNote01Icon,
  NewTwitterIcon,
  NoteEditIcon,
  NoteIcon,
  NotebookIcon,
  PackageIcon,
  PaletteIcon,
  PaperclipIcon,
  PauseIcon,
  PencilEdit01Icon,
  PencilIcon,
  PencilLineIcon,
  PhoneIcon,
  PictureInPictureIcon,
  PieChartIcon,
  PinIcon,
  PlayIcon,
  Plug01Icon,
  PlusIcon,
  Presentation01Icon,
  Pulse01Icon,
  PuzzleIcon,
  QuotesIcon,
  RadioButtonIcon,
  RefreshCcwIcon,
  RefreshIcon,
  RepeatIcon,
  RocketIcon,
  RotateCcwIcon,
  RotateClockwiseIcon,
  Search01Icon,
  SentIcon,
  Settings01Icon,
  Settings02Icon,
  Share08Icon,
  Shield01Icon,
  ShieldCheckIcon,
  ShoppingBag01Icon,
  ShuffleIcon,
  SidebarLeftIcon,
  SignpostIcon,
  SmartphoneIcon,
  SortByUp01Icon,
  SortDescendingIcon,
  SparkleIcon,
  SquareIcon,
  StarIcon,
  StethoscopeIcon,
  Sun01Icon,
  Target01Icon,
  TextAlignLeftIcon,
  TextBoldIcon,
  TextFontIcon,
  TextIcon,
  TextItalicIcon,
  TextSquareIcon,
  TextStrikethroughIcon,
  TextUnderlineIcon,
  TrashIcon,
  TrendingUpIcon,
  TriangleAlertIcon,
  TrophyIcon,
  Upload01Icon,
  UserGroupIcon,
  UserIcon,
  UserPlusIcon,
  UserSearch01Icon,
  UserSwitchIcon,
  UsersIcon,
  Video01Icon,
  ViewSidebarLeftIcon,
  VolumeHighIcon,
  VolumeXIcon,
  Watch01Icon,
  Wrench01Icon,
  XIcon,
  ZapIcon,
} from "@hugeicons/core-free-icons";
import {
  HugeiconsIcon,
  type HugeiconsIconProps,
  type IconSvgElement,
} from "@hugeicons/react";
import {
  forwardRef,
  type ForwardRefExoticComponent,
  type PropsWithoutRef,
  type RefAttributes,
} from "react";

export type IconWeight =
  | "thin"
  | "light"
  | "regular"
  | "bold"
  | "fill"
  | "duotone";

export type IconProps = Omit<HugeiconsIconProps, "icon" | "strokeWidth"> & {
  mirrored?: boolean;
  strokeWidth?: number;
  weight?: IconWeight;
};

export type Icon = ForwardRefExoticComponent<
  PropsWithoutRef<IconProps> & RefAttributes<SVGSVGElement>
>;

const strokeWidthByWeight: Record<IconWeight, number> = {
  thin: 1,
  light: 1.25,
  regular: 1.5,
  bold: 2,
  fill: 2.25,
  duotone: 1.5,
};

function createIcon(icon: IconSvgElement, displayName: string): Icon {
  const Component = forwardRef<SVGSVGElement, IconProps>(
    (
      {
        fill,
        mirrored = false,
        strokeWidth,
        style,
        weight = "regular",
        ...props
      },
      ref,
    ) => (
      <HugeiconsIcon
        {...props}
        ref={ref}
        fill={fill ?? (weight === "fill" ? "currentColor" : undefined)}
        icon={icon}
        strokeWidth={strokeWidth ?? strokeWidthByWeight[weight]}
        style={
          mirrored
            ? {
                ...style,
                transform: [style?.transform, "scaleX(-1)"]
                  .filter(Boolean)
                  .join(" "),
                transformOrigin: style?.transformOrigin ?? "center",
              }
            : style
        }
      />
    ),
  );

  Component.displayName = displayName;
  return Component;
}

export const Airplane = /* @__PURE__ */ createIcon(Airplane01Icon, "Airplane");
export const AppWindow = /* @__PURE__ */ createIcon(AppWindowIcon, "AppWindow");
export const ArrowClockwise = /* @__PURE__ */ createIcon(
  RotateClockwiseIcon,
  "ArrowClockwise",
);
export const ArrowCounterClockwise = /* @__PURE__ */ createIcon(
  RotateCcwIcon,
  "ArrowCounterClockwise",
);
export const ArrowDown = /* @__PURE__ */ createIcon(
  ArrowDown02Icon,
  "ArrowDown",
);
export const ArrowElbowDownLeft = /* @__PURE__ */ createIcon(
  ArrowTurnBackwardIcon,
  "ArrowElbowDownLeft",
);
export const ArrowElbowDownRight = /* @__PURE__ */ createIcon(
  ArrowTurnForwardIcon,
  "ArrowElbowDownRight",
);
export const ArrowLeft = /* @__PURE__ */ createIcon(
  ArrowLeft02Icon,
  "ArrowLeft",
);
export const ArrowRight = /* @__PURE__ */ createIcon(
  ArrowRight02Icon,
  "ArrowRight",
);
export const ArrowsClockwise = /* @__PURE__ */ createIcon(
  RefreshIcon,
  "ArrowsClockwise",
);
export const ArrowsCounterClockwise = /* @__PURE__ */ createIcon(
  RefreshCcwIcon,
  "ArrowsCounterClockwise",
);
export const ArrowsDownUp = /* @__PURE__ */ createIcon(
  ArrowUpDownIcon,
  "ArrowsDownUp",
);
export const ArrowsInSimple = /* @__PURE__ */ createIcon(
  ArrowShrinkIcon,
  "ArrowsInSimple",
);
export const ArrowsMerge = /* @__PURE__ */ createIcon(
  GitMergeIcon,
  "ArrowsMerge",
);
export const ArrowsOutSimple = /* @__PURE__ */ createIcon(
  ArrowExpandIcon,
  "ArrowsOutSimple",
);
export const ArrowSquareOut = /* @__PURE__ */ createIcon(
  ExternalLinkIcon,
  "ArrowSquareOut",
);
export const ArrowUp = /* @__PURE__ */ createIcon(ArrowUp02Icon, "ArrowUp");
export const ArrowUpRight = /* @__PURE__ */ createIcon(
  ArrowUpRight02Icon,
  "ArrowUpRight",
);
export const Bank = /* @__PURE__ */ createIcon(BankIcon, "Bank");
export const Bell = /* @__PURE__ */ createIcon(BellIcon, "Bell");
export const BookOpen = /* @__PURE__ */ createIcon(BookOpen01Icon, "BookOpen");
export const BookOpenText = /* @__PURE__ */ createIcon(
  BookOpenTextIcon,
  "BookOpenText",
);
export const Brain = /* @__PURE__ */ createIcon(BrainIcon, "Brain");
export const Briefcase = /* @__PURE__ */ createIcon(
  Briefcase01Icon,
  "Briefcase",
);
export const Bug = /* @__PURE__ */ createIcon(Bug01Icon, "Bug");
export const Buildings = /* @__PURE__ */ createIcon(
  Building03Icon,
  "Buildings",
);
export const CalendarBlank = /* @__PURE__ */ createIcon(
  Calendar04Icon,
  "CalendarBlank",
);
export const CalendarDots = /* @__PURE__ */ createIcon(
  Calendar03Icon,
  "CalendarDots",
);
export const CalendarSlash = /* @__PURE__ */ createIcon(
  CalendarOffIcon,
  "CalendarSlash",
);
export const Camera = /* @__PURE__ */ createIcon(Camera01Icon, "Camera");
export const CaretDown = /* @__PURE__ */ createIcon(
  ArrowDown01Icon,
  "CaretDown",
);
export const CaretLeft = /* @__PURE__ */ createIcon(
  ArrowLeft01Icon,
  "CaretLeft",
);
export const CaretRight = /* @__PURE__ */ createIcon(
  ArrowRight01Icon,
  "CaretRight",
);
export const CaretUp = /* @__PURE__ */ createIcon(ArrowUp01Icon, "CaretUp");
export const ChartBar = /* @__PURE__ */ createIcon(ChartBarBigIcon, "ChartBar");
export const ChartLineUp = /* @__PURE__ */ createIcon(
  ChartLineData01Icon,
  "ChartLineUp",
);
export const ChartPie = /* @__PURE__ */ createIcon(PieChartIcon, "ChartPie");
export const Chat = /* @__PURE__ */ createIcon(Message01Icon, "Chat");
export const ChatCenteredDots = /* @__PURE__ */ createIcon(
  MessageSquareMoreIcon,
  "ChatCenteredDots",
);
export const ChatCircle = /* @__PURE__ */ createIcon(
  MessageCircleIcon,
  "ChatCircle",
);
export const Chats = /* @__PURE__ */ createIcon(MessageMultiple01Icon, "Chats");
export const Check = /* @__PURE__ */ createIcon(CheckIcon, "Check");
export const CheckCircle = /* @__PURE__ */ createIcon(
  CheckmarkCircle02Icon,
  "CheckCircle",
);
export const Circle = /* @__PURE__ */ createIcon(CircleIcon, "Circle");
export const CircleNotch = /* @__PURE__ */ createIcon(
  LoaderCircleIcon,
  "CircleNotch",
);
export const CirclesThreePlus = /* @__PURE__ */ createIcon(
  AddTeamIcon,
  "CirclesThreePlus",
);
export const ClipboardText = /* @__PURE__ */ createIcon(
  ClipboardListIcon,
  "ClipboardText",
);
export const Clock = /* @__PURE__ */ createIcon(Clock01Icon, "Clock");
export const ClockCounterClockwise = /* @__PURE__ */ createIcon(
  HistoryIcon,
  "ClockCounterClockwise",
);
export const Cloud = /* @__PURE__ */ createIcon(CloudIcon, "Cloud");
export const CloudSlash = /* @__PURE__ */ createIcon(
  CloudOffIcon,
  "CloudSlash",
);
export const CloudWarning = /* @__PURE__ */ createIcon(
  CloudAlertIcon,
  "CloudWarning",
);
export const Code = /* @__PURE__ */ createIcon(CodeIcon, "Code");
export const Coffee = /* @__PURE__ */ createIcon(Coffee01Icon, "Coffee");
export const Compass = /* @__PURE__ */ createIcon(CompassIcon, "Compass");
export const Copy = /* @__PURE__ */ createIcon(CopyIcon, "Copy");
export const Cpu = /* @__PURE__ */ createIcon(CpuIcon, "Cpu");
export const CreditCard = /* @__PURE__ */ createIcon(
  CreditCardIcon,
  "CreditCard",
);
export const Crown = /* @__PURE__ */ createIcon(CrownIcon, "Crown");
export const CurrencyDollar = /* @__PURE__ */ createIcon(
  DollarSignIcon,
  "CurrencyDollar",
);
export const Cursor = /* @__PURE__ */ createIcon(Cursor01Icon, "Cursor");
export const Database = /* @__PURE__ */ createIcon(DatabaseIcon, "Database");
export const Desktop = /* @__PURE__ */ createIcon(ComputerIcon, "Desktop");
export const DeviceMobile = /* @__PURE__ */ createIcon(
  SmartphoneIcon,
  "DeviceMobile",
);
export const DiscordLogo = /* @__PURE__ */ createIcon(
  DiscordIcon,
  "DiscordLogo",
);
export const DotsSixVertical = /* @__PURE__ */ createIcon(
  GripVerticalIcon,
  "DotsSixVertical",
);
export const DotsThree = /* @__PURE__ */ createIcon(
  MoreHorizontalIcon,
  "DotsThree",
);
export const DotsThreeVertical = /* @__PURE__ */ createIcon(
  MoreVerticalIcon,
  "DotsThreeVertical",
);
export const DownloadSimple = /* @__PURE__ */ createIcon(
  Download01Icon,
  "DownloadSimple",
);
export const Envelope = /* @__PURE__ */ createIcon(Mail01Icon, "Envelope");
export const EnvelopeOpen = /* @__PURE__ */ createIcon(
  MailOpen01Icon,
  "EnvelopeOpen",
);
export const EnvelopeSimple = /* @__PURE__ */ createIcon(
  Mail02Icon,
  "EnvelopeSimple",
);
export const Eye = /* @__PURE__ */ createIcon(EyeIcon, "Eye");
export const EyeSlash = /* @__PURE__ */ createIcon(EyeOffIcon, "EyeSlash");
export const File = /* @__PURE__ */ createIcon(File01Icon, "File");
export const FileArrowDown = /* @__PURE__ */ createIcon(
  FileDownloadIcon,
  "FileArrowDown",
);
export const FileText = /* @__PURE__ */ createIcon(FileTextIcon, "FileText");
export const FileXls = /* @__PURE__ */ createIcon(
  FileSpreadsheetIcon,
  "FileXls",
);
export const Fire = /* @__PURE__ */ createIcon(FireIcon, "Fire");
export const Flag = /* @__PURE__ */ createIcon(Flag01Icon, "Flag");
export const FloppyDisk = /* @__PURE__ */ createIcon(
  FloppyDiskIcon,
  "FloppyDisk",
);
export const Folder = /* @__PURE__ */ createIcon(Folder01Icon, "Folder");
export const FolderOpen = /* @__PURE__ */ createIcon(
  FolderOpenIcon,
  "FolderOpen",
);
export const FolderSimple = /* @__PURE__ */ createIcon(
  Folder01Icon,
  "FolderSimple",
);
export const FunnelSimple = /* @__PURE__ */ createIcon(
  FilterIcon,
  "FunnelSimple",
);
export const Gear = /* @__PURE__ */ createIcon(Settings02Icon, "Gear");
export const GearSix = /* @__PURE__ */ createIcon(Settings01Icon, "GearSix");
export const GithubLogo = /* @__PURE__ */ createIcon(GithubIcon, "GithubLogo");
export const GitMerge = /* @__PURE__ */ createIcon(GitMergeIcon, "GitMerge");
export const GitPullRequest = /* @__PURE__ */ createIcon(
  GitPullRequestIcon,
  "GitPullRequest",
);
export const Globe = /* @__PURE__ */ createIcon(GlobeIcon, "Globe");
export const GraduationCap = /* @__PURE__ */ createIcon(
  GraduationCapIcon,
  "GraduationCap",
);
export const Hammer = /* @__PURE__ */ createIcon(HammerIcon, "Hammer");
export const Handshake = /* @__PURE__ */ createIcon(HandshakeIcon, "Handshake");
export const HardDrive = /* @__PURE__ */ createIcon(HardDriveIcon, "HardDrive");
export const Headphones = /* @__PURE__ */ createIcon(
  HeadphonesIcon,
  "Headphones",
);
export const Headset = /* @__PURE__ */ createIcon(HeadsetIcon, "Headset");
export const Heart = /* @__PURE__ */ createIcon(HeartIcon, "Heart");
export const Highlighter = /* @__PURE__ */ createIcon(
  HighlighterIcon,
  "Highlighter",
);
export const House = /* @__PURE__ */ createIcon(House01Icon, "House");
export const Image = /* @__PURE__ */ createIcon(Image01Icon, "Image");
export const Info = /* @__PURE__ */ createIcon(InfoIcon, "Info");
export const Kanban = /* @__PURE__ */ createIcon(KanbanIcon, "Kanban");
export const Key = /* @__PURE__ */ createIcon(Key01Icon, "Key");
export const Leaf = /* @__PURE__ */ createIcon(Leaf01Icon, "Leaf");
export const Lightbulb = /* @__PURE__ */ createIcon(LightbulbIcon, "Lightbulb");
export const Lightning = /* @__PURE__ */ createIcon(ZapIcon, "Lightning");
export const Link = /* @__PURE__ */ createIcon(LinkIcon, "Link");
export const ListBullets = /* @__PURE__ */ createIcon(
  LeftToRightListBulletIcon,
  "ListBullets",
);
export const ListChecks = /* @__PURE__ */ createIcon(
  ListChecksIcon,
  "ListChecks",
);
export const ListNumbers = /* @__PURE__ */ createIcon(
  LeftToRightListNumberIcon,
  "ListNumbers",
);
export const Lock = /* @__PURE__ */ createIcon(LockPasswordIcon, "Lock");
export const LockKey = /* @__PURE__ */ createIcon(LockKeyIcon, "LockKey");
export const LockOpen = /* @__PURE__ */ createIcon(
  LockKeyholeOpenIcon,
  "LockOpen",
);
export const LockSimple = /* @__PURE__ */ createIcon(
  LockPasswordIcon,
  "LockSimple",
);
export const MagicWand = /* @__PURE__ */ createIcon(
  MagicWand01Icon,
  "MagicWand",
);
export const MagnifyingGlass = /* @__PURE__ */ createIcon(
  Search01Icon,
  "MagnifyingGlass",
);
export const MapPin = /* @__PURE__ */ createIcon(MapPinIcon, "MapPin");
export const MapTrifold = /* @__PURE__ */ createIcon(MapIcon, "MapTrifold");
export const Megaphone = /* @__PURE__ */ createIcon(
  Megaphone01Icon,
  "Megaphone",
);
export const Microphone = /* @__PURE__ */ createIcon(Mic01Icon, "Microphone");
export const MicrophoneSlash = /* @__PURE__ */ createIcon(
  MicOff01Icon,
  "MicrophoneSlash",
);
export const Minus = /* @__PURE__ */ createIcon(MinusIcon, "Minus");
export const MinusCircle = /* @__PURE__ */ createIcon(
  CircleMinusIcon,
  "MinusCircle",
);
export const Moon = /* @__PURE__ */ createIcon(MoonIcon, "Moon");
export const MusicNote = /* @__PURE__ */ createIcon(
  MusicNote01Icon,
  "MusicNote",
);
export const Note = /* @__PURE__ */ createIcon(NoteIcon, "Note");
export const Notebook = /* @__PURE__ */ createIcon(NotebookIcon, "Notebook");
export const NotePencil = /* @__PURE__ */ createIcon(
  NoteEditIcon,
  "NotePencil",
);
export const Package = /* @__PURE__ */ createIcon(PackageIcon, "Package");
export const Palette = /* @__PURE__ */ createIcon(PaletteIcon, "Palette");
export const Paperclip = /* @__PURE__ */ createIcon(PaperclipIcon, "Paperclip");
export const PaperPlaneTilt = /* @__PURE__ */ createIcon(
  SentIcon,
  "PaperPlaneTilt",
);
export const Pause = /* @__PURE__ */ createIcon(PauseIcon, "Pause");
export const Pencil = /* @__PURE__ */ createIcon(PencilIcon, "Pencil");
export const PencilSimple = /* @__PURE__ */ createIcon(
  PencilEdit01Icon,
  "PencilSimple",
);
export const PencilSimpleLine = /* @__PURE__ */ createIcon(
  PencilLineIcon,
  "PencilSimpleLine",
);
export const Phone = /* @__PURE__ */ createIcon(PhoneIcon, "Phone");
export const PictureInPicture = /* @__PURE__ */ createIcon(
  PictureInPictureIcon,
  "PictureInPicture",
);
export const Play = /* @__PURE__ */ createIcon(PlayIcon, "Play");
export const Plugs = /* @__PURE__ */ createIcon(Plug01Icon, "Plugs");
export const PlugsConnected = /* @__PURE__ */ createIcon(
  ElectricPlugsIcon,
  "PlugsConnected",
);
export const Plus = /* @__PURE__ */ createIcon(PlusIcon, "Plus");
export const PlusCircle = /* @__PURE__ */ createIcon(
  AddCircleIcon,
  "PlusCircle",
);
export const Presentation = /* @__PURE__ */ createIcon(
  Presentation01Icon,
  "Presentation",
);
export const Prohibit = /* @__PURE__ */ createIcon(BanIcon, "Prohibit");
export const Pulse = /* @__PURE__ */ createIcon(Pulse01Icon, "Pulse");
export const PushPin = /* @__PURE__ */ createIcon(PinIcon, "PushPin");
export const PuzzlePiece = /* @__PURE__ */ createIcon(
  PuzzleIcon,
  "PuzzlePiece",
);
export const Quotes = /* @__PURE__ */ createIcon(QuotesIcon, "Quotes");
export const RadioButton = /* @__PURE__ */ createIcon(
  RadioButtonIcon,
  "RadioButton",
);
export const Repeat = /* @__PURE__ */ createIcon(RepeatIcon, "Repeat");
export const Rocket = /* @__PURE__ */ createIcon(RocketIcon, "Rocket");
export const Scales = /* @__PURE__ */ createIcon(BalanceScaleIcon, "Scales");
export const ShareNetwork = /* @__PURE__ */ createIcon(
  Share08Icon,
  "ShareNetwork",
);
export const Shield = /* @__PURE__ */ createIcon(Shield01Icon, "Shield");
export const ShieldCheck = /* @__PURE__ */ createIcon(
  ShieldCheckIcon,
  "ShieldCheck",
);
export const ShoppingBag = /* @__PURE__ */ createIcon(
  ShoppingBag01Icon,
  "ShoppingBag",
);
export const Shuffle = /* @__PURE__ */ createIcon(ShuffleIcon, "Shuffle");
export const Sidebar = /* @__PURE__ */ createIcon(
  ViewSidebarLeftIcon,
  "Sidebar",
);
export const SidebarSimple = /* @__PURE__ */ createIcon(
  SidebarLeftIcon,
  "SidebarSimple",
);
export const SignIn = /* @__PURE__ */ createIcon(Login03Icon, "SignIn");
export const Signpost = /* @__PURE__ */ createIcon(SignpostIcon, "Signpost");
export const SortAscending = /* @__PURE__ */ createIcon(
  SortByUp01Icon,
  "SortAscending",
);
export const SortDescending = /* @__PURE__ */ createIcon(
  SortDescendingIcon,
  "SortDescending",
);
export const Sparkle = /* @__PURE__ */ createIcon(SparkleIcon, "Sparkle");
export const SpeakerHigh = /* @__PURE__ */ createIcon(
  VolumeHighIcon,
  "SpeakerHigh",
);
export const SpeakerX = /* @__PURE__ */ createIcon(VolumeXIcon, "SpeakerX");
export const Square = /* @__PURE__ */ createIcon(SquareIcon, "Square");
export const Star = /* @__PURE__ */ createIcon(StarIcon, "Star");
export const Stethoscope = /* @__PURE__ */ createIcon(
  StethoscopeIcon,
  "Stethoscope",
);
export const Sun = /* @__PURE__ */ createIcon(Sun01Icon, "Sun");
export const Swap = /* @__PURE__ */ createIcon(Exchange01Icon, "Swap");
export const Target = /* @__PURE__ */ createIcon(Target01Icon, "Target");
export const TextAa = /* @__PURE__ */ createIcon(TextFontIcon, "TextAa");
export const TextAlignLeft = /* @__PURE__ */ createIcon(
  TextAlignLeftIcon,
  "TextAlignLeft",
);
export const TextB = /* @__PURE__ */ createIcon(TextBoldIcon, "TextB");
export const Textbox = /* @__PURE__ */ createIcon(TextSquareIcon, "Textbox");
export const TextHOne = /* @__PURE__ */ createIcon(Heading01Icon, "TextHOne");
export const TextHThree = /* @__PURE__ */ createIcon(
  Heading03Icon,
  "TextHThree",
);
export const TextHTwo = /* @__PURE__ */ createIcon(Heading02Icon, "TextHTwo");
export const TextItalic = /* @__PURE__ */ createIcon(
  TextItalicIcon,
  "TextItalic",
);
export const TextStrikethrough = /* @__PURE__ */ createIcon(
  TextStrikethroughIcon,
  "TextStrikethrough",
);
export const TextT = /* @__PURE__ */ createIcon(TextIcon, "TextT");
export const TextUnderline = /* @__PURE__ */ createIcon(
  TextUnderlineIcon,
  "TextUnderline",
);
export const Trash = /* @__PURE__ */ createIcon(TrashIcon, "Trash");
export const TrendUp = /* @__PURE__ */ createIcon(TrendingUpIcon, "TrendUp");
export const Trophy = /* @__PURE__ */ createIcon(TrophyIcon, "Trophy");
export const UploadSimple = /* @__PURE__ */ createIcon(
  Upload01Icon,
  "UploadSimple",
);
export const User = /* @__PURE__ */ createIcon(UserIcon, "User");
export const UserFocus = /* @__PURE__ */ createIcon(
  UserSearch01Icon,
  "UserFocus",
);
export const UserPlus = /* @__PURE__ */ createIcon(UserPlusIcon, "UserPlus");
export const Users = /* @__PURE__ */ createIcon(UsersIcon, "Users");
export const UsersThree = /* @__PURE__ */ createIcon(
  UserGroupIcon,
  "UsersThree",
);
export const UserSwitch = /* @__PURE__ */ createIcon(
  UserSwitchIcon,
  "UserSwitch",
);
export const VideoCamera = /* @__PURE__ */ createIcon(
  Video01Icon,
  "VideoCamera",
);
export const Warning = /* @__PURE__ */ createIcon(TriangleAlertIcon, "Warning");
export const WarningCircle = /* @__PURE__ */ createIcon(
  AlertCircleIcon,
  "WarningCircle",
);
export const Watch = /* @__PURE__ */ createIcon(Watch01Icon, "Watch");
export const Waveform = /* @__PURE__ */ createIcon(
  AudioWaveformIcon,
  "Waveform",
);
export const Wrench = /* @__PURE__ */ createIcon(Wrench01Icon, "Wrench");
export const X = /* @__PURE__ */ createIcon(XIcon, "X");
export const XCircle = /* @__PURE__ */ createIcon(CancelCircleIcon, "XCircle");
export const XLogo = /* @__PURE__ */ createIcon(NewTwitterIcon, "XLogo");
