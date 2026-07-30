import SwiftUI

private enum WatchPage: Hashable {
  case listening
  case account
}

struct ContentView: View {
  @ObservedObject var recorder: RecordingController
  @ObservedObject var syncController: WatchSyncController
  @State private var recordingAccountUserId: String?
  @State private var selectedPage = WatchPage.listening

  var body: some View {
    Group {
      if syncController.account == nil {
        PairingView(syncController: syncController)
      } else {
        TabView(selection: $selectedPage) {
          ListeningView(recorder: recorder) {
            toggleRecording()
          }
          .tag(WatchPage.listening)
          AccountSettingsView(syncController: syncController)
            .tag(WatchPage.account)
        }
        .tabViewStyle(.page(indexDisplayMode: .always))
        .onAppear {
          #if DEBUG
            if ProcessInfo.processInfo.arguments.contains("-demo-settings") {
              selectedPage = .account
            }
          #endif
        }
      }
    }
    .onChange(of: recorder.lastCompletedRecording) { _, recording in
      guard let recording, let recordingAccountUserId else {
        return
      }
      syncController.enqueueRecording(
        url: recording.url,
        recordedAt: recording.recordedAt,
        accountUserId: recordingAccountUserId
      )
      self.recordingAccountUserId = nil
    }
    .onChange(of: syncController.account) { previousAccount, account in
      if previousAccount?.userId != account?.userId {
        recorder.stopIfNeeded()
      }
    }
    .alert(
      "Unable to listen",
      isPresented: Binding(
        get: { recorder.errorMessage != nil },
        set: { isPresented in
          if !isPresented {
            recorder.dismissError()
          }
        }
      )
    ) {
      Button("OK") {
        recorder.dismissError()
      }
    } message: {
      Text(recorder.errorMessage ?? "")
    }
    .sensoryFeedback(trigger: recorder.isRecording) { _, isRecording in
      isRecording ? .start : .stop
    }
  }

  private func toggleRecording() {
    if recorder.isRecording {
      recorder.toggle()
      return
    }

    guard let accountUserId = syncController.account?.userId else {
      return
    }
    recordingAccountUserId = accountUserId
    recorder.toggle()
  }
}

private struct PairingView: View {
  @ObservedObject var syncController: WatchSyncController

  var body: some View {
    ContentUnavailableView {
      Label(
        "Connect iPhone",
        systemImage: "iphone.and.arrow.forward.inward"
      )
    } description: {
      Text("Open Anarlog on iPhone to sync.")
    } actions: {
      if syncController.activationState == .notActivated {
        ProgressView()
          .accessibilityLabel("Connecting…")
      } else {
        Button("Check again") {
          syncController.refreshAccount()
        }
        .buttonStyle(.borderedProminent)
        .tint(.white)
        .foregroundStyle(.black)
      }
    }
    .onAppear {
      syncController.refreshAccount()
    }
  }
}

private struct ListeningView: View {
  @ObservedObject var recorder: RecordingController
  let onToggle: () -> Void

  var body: some View {
    ZStack {
      (recorder.isRecording ? Color.red : Color(white: 0.08))

      if recorder.isRecording {
        Waveform(levels: recorder.levels)
          .padding(.horizontal, 14)
          .padding(.vertical, 24)
      } else {
        HStack(spacing: 14) {
          PulsingRecordDot()

          Text("Start listening")
            .font(.system(size: 21, weight: .medium, design: .rounded))
            .foregroundStyle(.white)
            .minimumScaleFactor(0.75)
            .lineLimit(1)
        }
        .padding(.horizontal, 12)
      }
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .contentShape(Rectangle())
    .onTapGesture {
      onToggle()
    }
    .ignoresSafeArea()
    .accessibilityElement(children: .ignore)
    .accessibilityLabel(
      recorder.isRecording ? "Stop listening" : "Start listening"
    )
    .accessibilityValue(recorder.isRecording ? "Recording" : "Not recording")
    .accessibilityAddTraits(.isButton)
    .accessibilityAction {
      onToggle()
    }
  }
}

private struct AccountSettingsView: View {
  @ObservedObject var syncController: WatchSyncController

  var body: some View {
    Form {
      Section {
        if let email = syncController.account?.email {
          LabeledContent("Email") {
            Text(email)
              .multilineTextAlignment(.trailing)
          }
        } else {
          Text("Connected to Anarlog")
        }

        connectionStatus

        if syncController.pendingTransferCount > 0 {
          LabeledContent("Waiting to sync") {
            Text("\(syncController.pendingTransferCount)")
          }
        }

        Button("Sync now") {
          syncController.syncNow()
        }
        .buttonStyle(.borderedProminent)
        .tint(.white)
        .foregroundStyle(.black)
      } header: {
        Label("Account", systemImage: "person.crop.circle.fill")
      }
    }
    .padding(.bottom, 18)
    .accessibilityElement(children: .contain)
  }

  @ViewBuilder
  private var connectionStatus: some View {
    switch syncController.activationState {
    case .notActivated:
      HStack(spacing: 8) {
        ProgressView()
          .progressViewStyle(.circular)
          .controlSize(.small)
          .fixedSize()
        Text("Connecting…")
          .font(.footnote)
          .lineLimit(1)
      }
      .frame(maxWidth: .infinity, alignment: .leading)
    case .activated where syncController.isPhoneReachable:
      Label(
        syncController.statusText,
        systemImage: "iphone.radiowaves.left.and.right"
      )
      .foregroundStyle(.secondary)
    case .activated, .inactive:
      Label(
        syncController.statusText,
        systemImage: "arrow.trianglehead.2.clockwise.rotate.90"
      )
      .foregroundStyle(.secondary)
    @unknown default:
      Label(
        syncController.statusText,
        systemImage: "arrow.trianglehead.2.clockwise.rotate.90"
      )
      .foregroundStyle(.secondary)
    }
  }
}

private struct PulsingRecordDot: View {
  var body: some View {
    Image(systemName: "circle.fill")
      .font(.system(size: 18))
      .foregroundStyle(.red)
      .symbolEffect(.pulse)
      .frame(width: 24, height: 24)
      .accessibilityHidden(true)
  }
}

private struct Waveform: View {
  let levels: [CGFloat]

  var body: some View {
    GeometryReader { geometry in
      HStack(spacing: 3) {
        ForEach(levels.indices, id: \.self) { index in
          Capsule()
            .fill(.white)
            .frame(maxWidth: .infinity)
            .frame(height: max(4, geometry.size.height * levels[index]))
        }
      }
      .frame(maxHeight: .infinity)
      .animation(.linear(duration: 0.08), value: levels)
    }
  }
}
