import Cocoa
import SwiftUI

enum FloatingOverlaySettingsPanelLayout {
  static let width: CGFloat = 276
  static let height: CGFloat = 282
  static let cornerRadius: CGFloat = 16
  static let margin: CGFloat = 10
}

final class FloatingOverlaySettingsPanelManager {
  static let shared = FloatingOverlaySettingsPanelManager()

  private let settings = FloatingOverlaySettingsModel.shared
  private var panel: NSPanel?

  private init() {}

  func toggle(anchor: NSPanel?) {
    runOnMain { [weak self] in
      guard let self else { return }

      if let panel = self.panel, panel.isVisible {
        self.hidePanel()
        return
      }

      self.show(anchor: anchor)
    }
  }

  func hide() {
    runOnMain { [weak self] in
      self?.hidePanel()
    }
  }

  private func show(anchor: NSPanel?) {
    let panel = self.panel ?? createPanel()
    if panel.contentView == nil {
      panel.contentView = NSHostingView(
        rootView: FloatingOverlaySettingsPanelView(settings: settings))
    }

    position(panel, anchor: anchor)
    panel.orderFrontRegardless()
    self.panel = panel
  }

  private func hidePanel() {
    guard let panel else { return }
    panel.orderOut(nil)
  }

  private func createPanel() -> NSPanel {
    let panel = NSPanel(
      contentRect: NSRect(
        x: 0,
        y: 0,
        width: FloatingOverlaySettingsPanelLayout.width,
        height: FloatingOverlaySettingsPanelLayout.height),
      styleMask: [.borderless, .nonactivatingPanel],
      backing: .buffered,
      defer: false
    )

    panel.level = .floating
    panel.isFloatingPanel = true
    panel.hidesOnDeactivate = false
    panel.isOpaque = false
    panel.backgroundColor = .clear
    panel.hasShadow = true
    panel.sharingType = .none
    panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
    return panel
  }

  private func position(_ panel: NSPanel, anchor: NSPanel?) {
    let screen = anchor?.screen ?? NSScreen.main ?? NSScreen.screens.first
    guard let screen else { return }

    let frame = screen.visibleFrame
    let size = panel.frame.size
    let margin = FloatingOverlaySettingsPanelLayout.margin
    let anchorFrame =
      anchor?.frame
      ?? NSRect(
        x: frame.maxX - margin,
        y: frame.midY,
        width: 0,
        height: 0)

    var x = anchorFrame.minX - size.width - margin
    if x < frame.minX + margin {
      x = anchorFrame.maxX + margin
    }

    let minY = frame.minY + margin
    let maxY = frame.maxY - size.height - margin
    let y = min(max(anchorFrame.midY - size.height / 2, minY), maxY)
    panel.setFrameOrigin(NSPoint(x: x, y: y))
  }

  private func runOnMain(_ block: @escaping () -> Void) {
    if Thread.isMainThread {
      block()
      return
    }

    DispatchQueue.main.async(execute: block)
  }
}

private struct FloatingOverlaySettingsPanelView: View {
  @ObservedObject var settings: FloatingOverlaySettingsModel

  var body: some View {
    VStack(alignment: .leading, spacing: 14) {
      HStack {
        Text("Floating bar")
          .font(.system(size: 14, weight: .semibold))
        Spacer()
        Button(action: { FloatingOverlaySettingsPanelManager.shared.hide() }) {
          Image(systemName: "xmark")
            .font(.system(size: 11, weight: .semibold))
        }
        .buttonStyle(.plain)
        .foregroundStyle(.secondary)
        .accessibilityLabel("Close settings")
      }

      OpacitySlider(
        title: "Bar opacity",
        value: settings.floatingBarOpacity,
        onChange: settings.setFloatingBarOpacity)

      Divider()

      OpacitySlider(
        title: "Transcript opacity",
        value: settings.liveCaptionOpacity,
        range: FloatingOverlayOpacity.minLiveCaption...FloatingOverlayOpacity.maxLiveCaption,
        onChange: settings.setLiveCaptionOpacity)

      VStack(alignment: .leading, spacing: 8) {
        Text("Transcript position")
          .font(.system(size: 12, weight: .medium))
          .foregroundStyle(.secondary)

        LazyVGrid(
          columns: Array(repeating: GridItem(.flexible(), spacing: 6), count: 3),
          spacing: 6
        ) {
          ForEach(LiveCaptionPosition.allCases, id: \.self) { position in
            Button(action: { settings.setLiveCaptionPosition(position) }) {
              Text(shortTitle(for: position))
                .font(.system(size: 11, weight: .medium))
                .frame(maxWidth: .infinity)
                .padding(.vertical, 6)
                .background(
                  position == settings.liveCaptionPosition ? Color.accentColor : Color.clear
                )
                .foregroundStyle(position == settings.liveCaptionPosition ? .white : .primary)
                .clipShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
            }
            .buttonStyle(.plain)
            .accessibilityLabel(position.title)
          }
        }
      }

      Button(
        action: { settings.setLiveCaptionMinimized(!settings.liveCaptionMinimized) }
      ) {
        HStack {
          Image(systemName: settings.liveCaptionMinimized ? "eye" : "eye.slash")
            .font(.system(size: 11, weight: .bold))
          Text(settings.liveCaptionMinimized ? "Show transcript" : "Hide transcript")
          Spacer()
        }
        .font(.system(size: 12, weight: .medium))
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(Color.primary.opacity(0.06))
        .clipShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
      }
      .buttonStyle(.plain)
    }
    .padding(14)
    .frame(
      width: FloatingOverlaySettingsPanelLayout.width,
      height: FloatingOverlaySettingsPanelLayout.height,
      alignment: .top
    )
    .background(
      RoundedRectangle(
        cornerRadius: FloatingOverlaySettingsPanelLayout.cornerRadius, style: .continuous
      )
      .fill(.regularMaterial)
    )
    .overlay(
      RoundedRectangle(
        cornerRadius: FloatingOverlaySettingsPanelLayout.cornerRadius, style: .continuous
      )
      .strokeBorder(Color.primary.opacity(0.12), lineWidth: 0.5)
    )
  }

  private func shortTitle(for position: LiveCaptionPosition) -> String {
    switch position {
    case .topCenter:
      return "Top"
    case .topLeft:
      return "TL"
    case .topRight:
      return "TR"
    case .bottomLeft:
      return "BL"
    case .bottomRight:
      return "BR"
    case .bottomCenter:
      return "Bottom"
    }
  }
}

private struct OpacitySlider: View {
  let title: String
  let value: Double
  var range: ClosedRange<Double> =
    FloatingOverlayOpacity.minFloatingBar...FloatingOverlayOpacity.maxFloatingBar
  let onChange: (Double) -> Void

  var body: some View {
    VStack(alignment: .leading, spacing: 6) {
      HStack {
        Text(title)
          .font(.system(size: 12, weight: .medium))
          .foregroundStyle(.secondary)
        Spacer()
        Text("\(Int((value * 100).rounded()))%")
          .font(.system(size: 11, weight: .medium))
          .foregroundStyle(.secondary)
      }
      Slider(value: Binding(get: { value }, set: onChange), in: range)
    }
  }
}
