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
  LaptopPhoneSyncIcon,
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
  Task01Icon,
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
  ZoomInIcon,
  ZoomOutIcon,
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

export type IconWeight = "thin" | "light" | "regular" | "bold";

export type IconProps = Omit<
  HugeiconsIconProps,
  "icon" | "strokeWidth" | "fill"
> & {
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
};

function createIcon(icon: IconSvgElement, displayName: string): Icon {
  const Component = forwardRef<SVGSVGElement, IconProps>(
    (
      { mirrored = false, strokeWidth, style, weight = "regular", ...props },
      ref,
    ) => (
      <HugeiconsIcon
        {...props}
        ref={ref}
        fill="none"
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

function createBrandIcon(
  path: string,
  displayName: string,
  viewBox = "0 0 24 24",
): Icon {
  const Component = forwardRef<SVGSVGElement, IconProps>(
    (
      {
        mirrored: _mirrored,
        strokeWidth: _strokeWidth,
        weight: _weight,
        size = 24,
        className,
        style,
        color = "currentColor",
        ...props
      },
      ref,
    ) => (
      <svg
        ref={ref}
        width={size}
        height={size}
        viewBox={viewBox}
        fill={color}
        className={className}
        style={style}
        {...props}
      >
        <path d={path} />
      </svg>
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
  Task01Icon,
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
export const Devices = /* @__PURE__ */ createIcon(
  LaptopPhoneSyncIcon,
  "Devices",
);
export const DiscordLogo = /* @__PURE__ */ createBrandIcon(
  "M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028c.462-.63.874-1.295 1.226-1.994a.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03ZM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.095 2.157 2.42 0 1.333-.956 2.418-2.157 2.418Zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.095 2.157 2.42 0 1.333-.946 2.418-2.157 2.418Z",
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
export const GithubLogo = /* @__PURE__ */ createBrandIcon(
  "M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12",
  "GithubLogo",
);
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
export const MagnifyingGlassPlus = /* @__PURE__ */ createIcon(
  ZoomInIcon,
  "MagnifyingGlassPlus",
);
export const MagnifyingGlassMinus = /* @__PURE__ */ createIcon(
  ZoomOutIcon,
  "MagnifyingGlassMinus",
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
export const XLogo = /* @__PURE__ */ createBrandIcon(
  "M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-4.714-6.231-5.401 6.231H2.744l7.727-8.822L1.254 2.25H8.08l4.253 5.622L18.244 2.25zm-1.161 17.52h1.833L7.084 4.126H5.117L17.083 19.77z",
  "XLogo",
);
