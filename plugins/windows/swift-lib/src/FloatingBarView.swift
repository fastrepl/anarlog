import AppKit
import SwiftUI

enum FloatingBarLayout {
  static let inset: CGFloat = 4
  static let screenMargin: CGFloat = 8
  static let compactHeight: CGFloat = 42
  static let compactStopWidth: CGFloat = 72
  static let compactSoloStopWidth: CGFloat = 78
  static let compactIconSize: CGFloat = 34
  static let compactGap: CGFloat = 4
  static let expandedWidth: CGFloat = 360
  static let expandedHeight: CGFloat = 430
  static let expandedCornerRadius: CGFloat = 20
  static let expandedPadding: CGFloat = 12
  static let waveformWidth: CGFloat = 26
  static let waveformHeight: CGFloat = 14
  static let stopSquareSize: CGFloat = 9
  static let dragClickThreshold: CGFloat = 4

  static func compactControlsWidth(showsExpand: Bool) -> CGFloat {
    if showsExpand {
      return compactStopWidth + compactGap + compactIconSize
    }

    return compactSoloStopWidth
  }

  static func compactWidth(showsExpand: Bool) -> CGFloat {
    compactControlsWidth(showsExpand: showsExpand) + inset * 2
  }

  static func containerSize(isExpanded: Bool, showsExpand: Bool) -> NSSize {
    if isExpanded {
      return NSSize(
        width: expandedWidth + inset * 2,
        height: expandedHeight + inset * 2)
    }

    return NSSize(
      width: compactWidth(showsExpand: showsExpand),
      height: compactHeight + inset * 2)
  }
}

struct FloatingBarView: View {
  @ObservedObject var model: FloatingBarViewModel
  @ObservedObject var settings: FloatingOverlaySettingsModel
  let panelOrigin: () -> NSPoint?
  let movePanel: (NSPoint) -> Void
  @State private var isStopHovered = false
  @State private var suppressNextClick = false
  @State private var dragStart: FloatingBarDragStart?

  var body: some View {
    Group {
      if model.isExpanded {
        expandedPanel
      } else {
        compactPill
      }
    }
    .padding(FloatingBarLayout.inset)
    .frame(
      width: containerSize.width,
      height: containerSize.height,
      alignment: .topTrailing
    )
    .contentShape(Rectangle())
    .simultaneousGesture(dragClickSuppressor)
  }

  private var compactPill: some View {
    floatingControls(isExpanded: false)
      .frame(
        width: FloatingBarLayout.compactControlsWidth(showsExpand: model.liveCaptionToggleVisible),
        height: FloatingBarLayout.compactHeight
      )
      .background(
        Capsule(style: .continuous)
          .fill(surfaceColor)
      )
      .overlay(
        Capsule(style: .continuous)
          .strokeBorder(outerStrokeColor, lineWidth: 0.5)
      )
      .overlay(
        Capsule(style: .continuous)
          .strokeBorder(innerStrokeColor, lineWidth: 0.5)
          .padding(1.5)
      )
  }

  private var expandedPanel: some View {
    ZStack(alignment: .topTrailing) {
      VStack(spacing: 12) {
        HStack {
          Text(model.title)
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(primaryContentColor)
            .lineLimit(1)
            .truncationMode(.tail)

          Spacer(minLength: 12)
        }
        .padding(.leading, FloatingBarLayout.expandedPadding)
        .padding(
          .trailing,
          FloatingBarLayout.compactControlsWidth(showsExpand: model.liveCaptionToggleVisible)
            + 12
        )
        .frame(height: FloatingBarLayout.compactHeight)

        ScrollViewReader { proxy in
          ScrollView(.vertical, showsIndicators: false) {
            VStack(spacing: 8) {
              ForEach(model.transcriptBubbles) { bubble in
                TranscriptBubbleView(
                  bubble: bubble,
                  accentColor: accentColor,
                  primaryContentColor: primaryContentColor,
                  secondaryContentColor: secondaryContentColor,
                  colorScheme: model.colorScheme
                )
                .id(bubble.id)
              }
            }
            .frame(maxWidth: .infinity, alignment: .bottom)
          }
          .frame(maxWidth: .infinity, maxHeight: .infinity)
          .onChange(of: model.transcriptBubbles.last?.id) { _, bubbleId in
            if let bubbleId {
              proxy.scrollTo(bubbleId, anchor: .bottom)
            }
          }
        }
        .padding(.horizontal, FloatingBarLayout.expandedPadding)
        .padding(.bottom, FloatingBarLayout.expandedPadding)
      }
      .frame(
        width: FloatingBarLayout.expandedWidth,
        height: FloatingBarLayout.expandedHeight,
        alignment: .top
      )

      floatingControls(isExpanded: true)
        .frame(
          width: FloatingBarLayout.compactControlsWidth(
            showsExpand: model.liveCaptionToggleVisible),
          height: FloatingBarLayout.compactHeight
        )
    }
    .frame(
      width: FloatingBarLayout.expandedWidth,
      height: FloatingBarLayout.expandedHeight,
      alignment: .top
    )
    .background(
      RoundedRectangle(
        cornerRadius: FloatingBarLayout.expandedCornerRadius,
        style: .continuous
      )
      .fill(surfaceColor)
    )
    .overlay(
      RoundedRectangle(
        cornerRadius: FloatingBarLayout.expandedCornerRadius,
        style: .continuous
      )
      .strokeBorder(outerStrokeColor, lineWidth: 0.5)
    )
    .overlay(
      RoundedRectangle(
        cornerRadius: FloatingBarLayout.expandedCornerRadius,
        style: .continuous
      )
      .strokeBorder(innerStrokeColor, lineWidth: 0.5)
      .padding(1.5)
    )
  }

  private func floatingControls(isExpanded: Bool) -> some View {
    HStack(spacing: FloatingBarLayout.compactGap) {
      audioControl(
        width: model.liveCaptionToggleVisible
          ? FloatingBarLayout.compactStopWidth : FloatingBarLayout.compactSoloStopWidth,
        height: FloatingBarLayout.compactIconSize
      )

      if model.liveCaptionToggleVisible {
        FloatingIconButton(
          systemName: isExpanded
            ? "arrow.down.right.and.arrow.up.left" : "arrow.up.left.and.arrow.down.right",
          accessibilityLabel: isExpanded ? "Collapse live transcript" : "Expand live transcript",
          color: primaryContentColor,
          hoverFill: controlHoverFill,
          size: FloatingBarLayout.compactIconSize,
          action: { performClick { setExpanded(!isExpanded) } }
        )
      }
    }
  }

  private func audioControl(width: CGFloat, height: CGFloat) -> some View {
    Button(action: { performClick(RustBridge.stopListening) }) {
      Group {
        if isStopHovered {
          Rectangle()
            .fill(stopColor)
            .frame(
              width: FloatingBarLayout.stopSquareSize,
              height: FloatingBarLayout.stopSquareSize
            )
        } else if model.status == .error {
          ErrorMark(color: errorAccentColor)
        } else {
          DancingBars(color: accentColor, amplitude: model.amplitude)
        }
      }
      .frame(
        width: FloatingBarLayout.waveformWidth,
        height: FloatingBarLayout.waveformHeight
      )
      .frame(width: width, height: height)
      .background(
        Capsule(style: .continuous)
          .fill(isStopHovered ? accentColor.opacity(0.16) : controlHoverFill)
      )
      .contentShape(Capsule(style: .continuous))
    }
    .buttonStyle(.plain)
    .accessibilityLabel("Stop listening")
    .onHover { isStopHovered = $0 }
  }

  private var containerSize: NSSize {
    FloatingBarLayout.containerSize(
      isExpanded: model.isExpanded,
      showsExpand: model.liveCaptionToggleVisible
    )
  }

  private var accentColor: Color {
    model.status == .error ? errorAccentColor : normalAccentColor
  }

  private var surfaceColor: Color {
    if model.colorScheme == .dark {
      return Color(red: 0.43, green: 0.44, blue: 0.40).opacity(settings.floatingBarOpacity)
    }

    return Color(red: 0.86, green: 0.85, blue: 0.82).opacity(settings.floatingBarOpacity)
  }

  private var primaryContentColor: Color {
    if model.colorScheme == .dark {
      return .white
    }

    return Color(red: 0.12, green: 0.11, blue: 0.10)
  }

  private var secondaryContentColor: Color {
    primaryContentColor.opacity(model.colorScheme == .dark ? 0.66 : 0.46)
  }

  private var controlHoverFill: Color {
    primaryContentColor.opacity(model.colorScheme == .dark ? 0.08 : 0.07)
  }

  private var outerStrokeColor: Color {
    primaryContentColor.opacity(model.colorScheme == .dark ? 0.14 : 0.12)
  }

  private var innerStrokeColor: Color {
    primaryContentColor.opacity(model.colorScheme == .dark ? 0.28 : 0.18)
  }

  private var stopColor: Color {
    normalAccentColor
  }

  private var errorAccentColor: Color {
    Color(red: 1, green: 0.25, blue: 0.24)
  }

  private var normalAccentColor: Color {
    Color(red: 1, green: 0.45, blue: 0.48)
  }

  private var dragClickSuppressor: some Gesture {
    DragGesture(
      minimumDistance: FloatingBarLayout.dragClickThreshold,
      coordinateSpace: .global
    )
    .onChanged { _ in
      suppressNextClick = true

      let mouseLocation = NSEvent.mouseLocation
      let start =
        dragStart
        ?? panelOrigin().map {
          FloatingBarDragStart(panelOrigin: $0, mouseLocation: mouseLocation)
        }

      guard let start else { return }
      dragStart = start

      movePanel(
        NSPoint(
          x: start.panelOrigin.x + mouseLocation.x - start.mouseLocation.x,
          y: start.panelOrigin.y + mouseLocation.y - start.mouseLocation.y
        )
      )
    }
    .onEnded { _ in
      dragStart = nil
      DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) {
        suppressNextClick = false
      }
    }
  }

  private func performClick(_ action: () -> Void) {
    if suppressNextClick {
      suppressNextClick = false
      return
    }

    action()
  }

  private func setExpanded(_ expanded: Bool) {
    model.isExpanded = expanded
    settings.setLiveCaptionMinimized(!expanded)
    if !expanded {
      LiveCaptionManager.shared.hide(clearText: false)
    }
  }
}

private struct FloatingBarDragStart {
  let panelOrigin: NSPoint
  let mouseLocation: NSPoint
}

private struct FloatingIconButton: View {
  let systemName: String
  let accessibilityLabel: String
  let color: Color
  let hoverFill: Color
  let size: CGFloat
  let action: () -> Void
  @State private var isHovered = false

  var body: some View {
    Button(action: action) {
      Image(systemName: systemName)
        .font(.system(size: 12, weight: .semibold))
        .foregroundStyle(color)
        .frame(width: size, height: size)
        .background(
          Circle()
            .fill(isHovered ? hoverFill : Color.clear)
        )
        .contentShape(Circle())
    }
    .buttonStyle(.plain)
    .accessibilityLabel(accessibilityLabel)
    .onHover { isHovered = $0 }
  }
}

private struct TranscriptBubbleView: View {
  let bubble: FloatingTranscriptBubblePayload
  let accentColor: Color
  let primaryContentColor: Color
  let secondaryContentColor: Color
  let colorScheme: FloatingBarColorScheme

  var body: some View {
    HStack {
      if bubble.isSelf {
        Spacer(minLength: 40)
      }

      VStack(alignment: bubble.isSelf ? .trailing : .leading, spacing: 3) {
        Text(bubble.speakerLabel)
          .font(.system(size: 10, weight: .semibold))
          .foregroundStyle(secondaryContentColor)
          .lineLimit(1)

        Text(bubble.text)
          .font(.system(size: 13, weight: .medium))
          .foregroundStyle(primaryContentColor)
          .multilineTextAlignment(bubble.isSelf ? .trailing : .leading)
          .fixedSize(horizontal: false, vertical: true)
      }
      .padding(.horizontal, 11)
      .padding(.vertical, 8)
      .background(bubbleBackground)
      .clipShape(RoundedRectangle(cornerRadius: 11, style: .continuous))
      .opacity(bubble.isFinal ? 1 : 0.68)

      if !bubble.isSelf {
        Spacer(minLength: 40)
      }
    }
  }

  private var bubbleBackground: Color {
    if bubble.isSelf {
      return accentColor.opacity(colorScheme == .dark ? 0.28 : 0.2)
    }

    return primaryContentColor.opacity(colorScheme == .dark ? 0.1 : 0.08)
  }
}

private struct ErrorMark: View {
  let color: Color

  var body: some View {
    VStack(spacing: 1.5) {
      Capsule(style: .continuous)
        .fill(color)
        .frame(width: 3.2, height: 8)
      Circle()
        .fill(color)
        .frame(width: 3.2, height: 3.2)
    }
  }
}

private struct DancingBars: View {
  let color: Color
  let amplitude: Double

  private let barCount = 5
  private let barWidth: CGFloat = 3
  private let barSpacing: CGFloat = 2
  private let minHeight: CGFloat = 2
  private let maxHeight: CGFloat = 14

  var body: some View {
    TimelineView(.animation(minimumInterval: 1.0 / 30.0, paused: false)) { timeline in
      HStack(spacing: barSpacing) {
        let t = timeline.date.timeIntervalSinceReferenceDate
        ForEach(0..<barCount, id: \.self) { index in
          Capsule(style: .continuous)
            .fill(color)
            .frame(width: barWidth, height: barHeight(index: index, time: t))
        }
      }
      .frame(maxHeight: .infinity, alignment: .center)
    }
  }

  private func barHeight(index: Int, time: TimeInterval) -> CGFloat {
    let normalized = min(max(amplitude, 0), 1)
    let center = Double(barCount - 1) / 2
    let distance = abs(Double(index) - center) / max(center, 1)
    let envelope = 1 - distance * 0.42
    let phase = time * 8.5 + Double(index) * 0.68
    let wave = sin(phase) * 0.5 + 0.5
    let drive = 0.4 + normalized * 0.9
    let height = maxHeight * CGFloat(drive * envelope * (0.4 + wave * 0.6))
    return max(minHeight, min(maxHeight, height))
  }
}
