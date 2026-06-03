import SwiftUI

enum DevtoolsPanelLayout {
  static let containerWidth: CGFloat = 300
  static let containerHeight: CGFloat = 560
  static let collapsedHeight: CGFloat = 44
  static let screenMargin: CGFloat = 14
}

struct DevtoolsPanelView: View {
  @State private var toastPreview = DevtoolsToastPreview.languageModel
  @State private var isCollapsed = false

  private let onCollapseChange: (Bool) -> Void

  init(onCollapseChange: @escaping (Bool) -> Void = { _ in }) {
    self.onCollapseChange = onCollapseChange
  }

  var body: some View {
    VStack(spacing: 0) {
      header
      if !isCollapsed {
        Divider()
        ScrollView(showsIndicators: false) {
          VStack(spacing: 10) {
            navigationSection
            toastsSection
            otaSection
            notificationsSection
            billingSection
            countdownSection
            errorSection
          }
          .padding(10)
        }
      }
    }
    .frame(
      width: DevtoolsPanelLayout.containerWidth,
      height: isCollapsed
        ? DevtoolsPanelLayout.collapsedHeight : DevtoolsPanelLayout.containerHeight
    )
    .background(
      RoundedRectangle(cornerRadius: 12, style: .continuous)
        .fill(Color(nsColor: .windowBackgroundColor).opacity(0.96))
    )
    .overlay(
      RoundedRectangle(cornerRadius: 12, style: .continuous)
        .strokeBorder(Color.black.opacity(0.08), lineWidth: 1)
    )
    .shadow(color: Color.black.opacity(0.20), radius: 18, y: 8)
  }

  private var header: some View {
    HStack {
      Text("Devtools")
        .font(.system(size: 13, weight: .semibold))
        .foregroundStyle(.primary)
      Spacer()
      Button {
        let nextIsCollapsed = !isCollapsed
        withAnimation(.easeInOut(duration: 0.16)) {
          isCollapsed = nextIsCollapsed
        }
        onCollapseChange(nextIsCollapsed)
      } label: {
        Image(systemName: isCollapsed ? "chevron.down" : "chevron.up")
          .font(.system(size: 11, weight: .bold))
          .foregroundStyle(.secondary)
          .frame(width: 24, height: 22)
          .background(
            RoundedRectangle(cornerRadius: 6, style: .continuous)
              .fill(Color.black.opacity(0.05))
          )
          .contentShape(Rectangle())
      }
      .buttonStyle(.plain)
      .accessibilityLabel(isCollapsed ? "Expand devtools" : "Collapse devtools")
    }
    .padding(.horizontal, 12)
    .padding(.vertical, 10)
    .contentShape(Rectangle())
  }

  private var navigationSection: some View {
    DevtoolsSection(title: "NAVIGATION") {
      DevtoolsActionButton("Onboarding") {
        RustBridge.devtoolsPanelAction("navigation:onboarding")
      }
      DevtoolsActionButton("Empty Tab") {
        RustBridge.devtoolsPanelAction("navigation:empty")
      }
      DevtoolsActionButton("Instruction: sign-in") {
        RustBridge.devtoolsPanelAction("instruction:sign-in")
      }
      DevtoolsActionButton("Instruction: billing") {
        RustBridge.devtoolsPanelAction("instruction:billing")
      }
      DevtoolsActionButton("Instruction: integration") {
        RustBridge.devtoolsPanelAction("instruction:integration")
      }
      DevtoolsActionButton("Changelog") {
        RustBridge.devtoolsPanelAction("navigation:changelog")
      }
    }
  }

  private var toastsSection: some View {
    DevtoolsSection(title: "TOASTS") {
      Menu {
        ForEach(DevtoolsToastPreview.allCases, id: \.self) { preview in
          Button(preview.label) {
            toastPreview = preview
            showToastPreview(preview)
          }
        }
      } label: {
        HStack {
          Text(toastPreview.label)
            .font(.system(size: 12, weight: .medium))
            .foregroundStyle(.primary)
            .lineLimit(1)
          Spacer()
          Image(systemName: "chevron.up.chevron.down")
            .font(.system(size: 10, weight: .semibold))
            .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 7)
        .background(
          RoundedRectangle(cornerRadius: 7, style: .continuous)
            .fill(Color.white.opacity(0.70))
        )
        .overlay(
          RoundedRectangle(cornerRadius: 7, style: .continuous)
            .strokeBorder(Color.black.opacity(0.08), lineWidth: 1)
        )
      }
      .menuStyle(.borderlessButton)

      DevtoolsActionButton("Show in App") {
        showToastPreview(toastPreview)
      }

      DevtoolsActionButton("Clear Preview") {
        RustBridge.devtoolsPanelAction("toasts:preview:clear")
      }

      DevtoolsActionButton("Reset Dismissed") {
        RustBridge.devtoolsPanelAction("toasts:reset-dismissed")
      }
    }
  }

  private var notificationsSection: some View {
    DevtoolsSection(title: "NOTIFICATIONS") {
      DevtoolsActionButton("Calendar") {
        RustBridge.devtoolsPanelAction("notifications:calendar")
      }
      DevtoolsActionButton("Mic Detected") {
        RustBridge.devtoolsPanelAction("notifications:mic-detected")
      }
      DevtoolsActionButton("Mic Options") {
        RustBridge.devtoolsPanelAction("notifications:mic-options")
      }
      DevtoolsActionButton("Auto-stop") {
        RustBridge.devtoolsPanelAction("notifications:auto-stop")
      }
      DevtoolsActionButton("Batch Done") {
        RustBridge.devtoolsPanelAction("notifications:batch-done")
      }
      DevtoolsActionButton("Clear") {
        RustBridge.devtoolsPanelAction("notifications:clear")
      }
    }
  }

  private var otaSection: some View {
    DevtoolsSection(title: "OTA") {
      DevtoolsActionButton("Available") {
        RustBridge.devtoolsPanelAction("ota:available")
      }
      DevtoolsActionButton("Downloading") {
        RustBridge.devtoolsPanelAction("ota:downloading")
      }
      DevtoolsActionButton("Ready") {
        RustBridge.devtoolsPanelAction("ota:ready")
      }
      DevtoolsActionButton("Failed") {
        RustBridge.devtoolsPanelAction("ota:failed")
      }
      DevtoolsActionButton("Clear") {
        RustBridge.devtoolsPanelAction("ota:clear")
      }
    }
  }

  private var billingSection: some View {
    DevtoolsSection(title: "BILLING") {
      DevtoolsActionButton("Trial Started") {
        RustBridge.devtoolsPanelAction("billing:trial-started")
      }
      DevtoolsActionButton("Trial Ended") {
        RustBridge.devtoolsPanelAction("billing:trial-ended")
      }
    }
  }

  private var countdownSection: some View {
    DevtoolsSection(title: "COUNTDOWN") {
      DevtoolsActionButton("+Note 20s") {
        RustBridge.devtoolsPanelAction("countdown:note-20")
      }
      DevtoolsActionButton("+Note 60s") {
        RustBridge.devtoolsPanelAction("countdown:note-60")
      }
      DevtoolsActionButton("+Note 5m") {
        RustBridge.devtoolsPanelAction("countdown:note-290")
      }
      DevtoolsActionButton("+Zoom 20s") {
        RustBridge.devtoolsPanelAction("countdown:zoom-20")
      }
      DevtoolsActionButton("+Zoom 60s") {
        RustBridge.devtoolsPanelAction("countdown:zoom-60")
      }
    }
  }

  private var errorSection: some View {
    DevtoolsSection(title: "ERROR") {
      DevtoolsActionButton("Trigger Error", role: .destructive) {
        RustBridge.devtoolsPanelAction("error:trigger")
      }
    }
  }

  private func showToastPreview(_ preview: DevtoolsToastPreview) {
    RustBridge.devtoolsPanelAction("toasts:preview:\(preview.rawValue)")
  }
}

private struct DevtoolsSection<Content: View>: View {
  let title: String
  let content: Content

  init(title: String, @ViewBuilder content: () -> Content) {
    self.title = title
    self.content = content()
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 7) {
      Text(title)
        .font(.system(size: 10, weight: .bold))
        .foregroundStyle(.secondary)
      VStack(spacing: 6) {
        content
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(9)
    .background(
      RoundedRectangle(cornerRadius: 8, style: .continuous)
        .fill(Color.black.opacity(0.035))
    )
  }
}

private struct DevtoolsActionButton: View {
  let title: String
  let role: ButtonRole?
  let action: () -> Void

  init(_ title: String, role: ButtonRole? = nil, action: @escaping () -> Void) {
    self.title = title
    self.role = role
    self.action = action
  }

  var body: some View {
    Button(role: role, action: action) {
      HStack {
        Text(title)
          .font(.system(size: 12, weight: .medium))
          .lineLimit(1)
        Spacer(minLength: 8)
      }
      .foregroundStyle(role == .destructive ? Color.red : Color.primary)
      .padding(.horizontal, 10)
      .padding(.vertical, 7)
      .frame(maxWidth: .infinity)
      .background(
        RoundedRectangle(cornerRadius: 7, style: .continuous)
          .fill(role == .destructive ? Color.red.opacity(0.08) : Color.white.opacity(0.70))
      )
      .overlay(
        RoundedRectangle(cornerRadius: 7, style: .continuous)
          .strokeBorder(
            role == .destructive ? Color.red.opacity(0.20) : Color.black.opacity(0.08),
            lineWidth: 1
          )
      )
    }
    .buttonStyle(.plain)
  }
}

private enum DevtoolsToastPreview: String, CaseIterable {
  case languageModel = "language-model"
  case transcriptionModel = "transcription-model"
  case transcriptionError = "transcription-error"
  case download
  case pro

  var label: String {
    switch self {
    case .languageModel:
      return "Language model"
    case .transcriptionModel:
      return "Transcription model"
    case .transcriptionError:
      return "Transcription error"
    case .download:
      return "Download"
    case .pro:
      return "Pro"
    }
  }
}
