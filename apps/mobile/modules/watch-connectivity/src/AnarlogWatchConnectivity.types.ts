export type AnarlogWatchConnectivityModuleEvents = {
  onRecordingReceived: (recording: PendingWatchRecording) => void;
  onStateChanged: (state: WatchConnectivityState) => void;
};

export type PendingWatchRecording = {
  id: string;
  uri: string;
  filename: string;
  recordedAt: string;
  accountUserId: string;
};

export type WatchConnectivityState = {
  supported: boolean;
  activationState: string;
  paired: boolean;
  watchAppInstalled: boolean;
  reachable: boolean;
};
