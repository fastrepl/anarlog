import SwiftUI

enum LiveCaptionLayout {
  static let minWidth: CGFloat = 260
  static let defaultWidth: CGFloat = 440
  static let maxWidth: CGFloat = 640
  static let minLineCount = 1
  static let defaultLineCount = 1
  static let maxLineCount = 4
  static let lineHeight: CGFloat = 22
  static let horizontalPadding: CGFloat = 16
  static let verticalPadding: CGFloat = 10
  static let controlsWidth: CGFloat = 24
  static let controlsGap: CGFloat = 8
  static let cornerRadius: CGFloat = 12
  static let screenMargin: CGFloat = 12
  static let topOffset: CGFloat = 18
  static let minimizedSize = NSSize(width: 42, height: 36)

  static func height(forLineCount lineCount: Int) -> CGFloat {
    let clampedLineCount = min(max(lineCount, minLineCount), maxLineCount)
    return verticalPadding * 2 + lineHeight * CGFloat(clampedLineCount)
  }

  static func lineCount(forHeight height: CGFloat) -> Int {
    let rawLineCount = ((height - verticalPadding * 2) / lineHeight).rounded()
    return min(max(Int(rawLineCount), minLineCount), maxLineCount)
  }
}

struct LiveCaptionView: View {
  @ObservedObject var model: LiveCaptionViewModel
  @ObservedObject var settings: FloatingOverlaySettingsModel
  let onSetMinimized: (Bool) -> Void
  @State private var isHovered = false

  var body: some View {
    Group {
      if settings.liveCaptionMinimized {
        minimizedBody
      } else {
        expandedBody
      }
    }
    .onHover { isHovered = $0 }
  }

  private var expandedBody: some View {
    HStack(alignment: .center, spacing: LiveCaptionLayout.controlsGap) {
      Text(model.text)
        .font(.system(size: 16, weight: .medium, design: .default))
        .lineSpacing(0)
        .foregroundStyle(.white)
        .multilineTextAlignment(.center)
        .lineLimit(model.lineCount)
        .truncationMode(.tail)
        .fixedSize(horizontal: false, vertical: true)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)

      VStack(spacing: 2) {
        CaptionControlButton(systemName: "minus") {
          onSetMinimized(true)
        }
        .accessibilityLabel("Minimize transcript")
      }
      .frame(width: LiveCaptionLayout.controlsWidth)
      .opacity(isHovered ? 1 : 0)
      .allowsHitTesting(isHovered)
    }
    .padding(.horizontal, LiveCaptionLayout.horizontalPadding)
    .padding(.vertical, LiveCaptionLayout.verticalPadding)
    .background(captionBackground)
    .overlay(alignment: .bottomLeading) {
      CaptionOpacityDragToggle(
        value: settings.liveCaptionOpacity,
        onChange: settings.setLiveCaptionOpacity
      )
      .opacity(isHovered ? 1 : 0)
      .allowsHitTesting(isHovered)
      .padding(.leading, 9)
      .padding(.bottom, 6)
    }
    .overlay(alignment: .bottomTrailing) {
      ResizeHint()
        .opacity(isHovered ? 0.55 : 0)
        .padding(6)
    }
    .contentShape(RoundedRectangle(cornerRadius: LiveCaptionLayout.cornerRadius))
  }

  private var minimizedBody: some View {
    Button(action: { onSetMinimized(false) }) {
      Image(systemName: "captions.bubble")
        .font(.system(size: 14, weight: .semibold))
        .foregroundStyle(.white)
        .frame(
          width: LiveCaptionLayout.minimizedSize.width,
          height: LiveCaptionLayout.minimizedSize.height
        )
        .background(captionBackground)
    }
    .buttonStyle(.plain)
    .accessibilityLabel("Restore transcript")
  }

  private var captionBackground: some View {
    RoundedRectangle(cornerRadius: LiveCaptionLayout.cornerRadius, style: .continuous)
      .fill(
        Color.black.opacity(
          min(
            max(settings.liveCaptionOpacity, FloatingOverlayOpacity.minLiveCaption),
            FloatingOverlayOpacity.max
          )))
  }
}

private struct CaptionControlButton: View {
  let systemName: String
  let action: () -> Void

  var body: some View {
    Button(action: action) {
      Image(systemName: systemName)
        .font(.system(size: 10, weight: .semibold))
        .foregroundStyle(.white.opacity(0.82))
        .frame(width: 20, height: 20)
        .background(Circle().fill(Color.white.opacity(0.12)))
    }
    .buttonStyle(.plain)
  }
}

private struct CaptionOpacityDragToggle: View {
  let value: Double
  let onChange: (Double) -> Void

  private let trackWidth: CGFloat = 52
  private let trackHeight: CGFloat = 14
  private let thumbSize: CGFloat = 10

  var body: some View {
    ZStack(alignment: .leading) {
      Capsule(style: .continuous)
        .fill(Color.white.opacity(0.12))
      Capsule(style: .continuous)
        .fill(Color.white.opacity(0.24))
        .frame(width: fillWidth)
      Circle()
        .fill(Color.white.opacity(0.9))
        .frame(width: thumbSize, height: thumbSize)
        .offset(x: thumbOffset)
    }
    .frame(width: trackWidth, height: trackHeight)
    .contentShape(Capsule(style: .continuous))
    .gesture(
      DragGesture(minimumDistance: 0)
        .onChanged { gesture in
          onChange(opacity(forX: gesture.location.x))
        }
    )
    .accessibilityElement()
    .accessibilityLabel("Transcript opacity")
    .accessibilityValue("\(Int((clampedValue * 100).rounded()))%")
  }

  private var clampedValue: Double {
    min(max(value, FloatingOverlayOpacity.minLiveCaption), FloatingOverlayOpacity.max)
  }

  private var progress: CGFloat {
    let range = FloatingOverlayOpacity.max - FloatingOverlayOpacity.minLiveCaption
    return CGFloat((clampedValue - FloatingOverlayOpacity.minLiveCaption) / range)
  }

  private var thumbOffset: CGFloat {
    progress * (trackWidth - thumbSize)
  }

  private var fillWidth: CGFloat {
    thumbSize + thumbOffset
  }

  private func opacity(forX x: CGFloat) -> Double {
    let usableWidth = trackWidth - thumbSize
    let clampedX = min(max(x - thumbSize / 2, 0), usableWidth)
    let nextProgress = Double(clampedX / usableWidth)
    return FloatingOverlayOpacity.minLiveCaption
      + nextProgress * (FloatingOverlayOpacity.max - FloatingOverlayOpacity.minLiveCaption)
  }
}

private struct ResizeHint: View {
  var body: some View {
    VStack(alignment: .trailing, spacing: 2) {
      Capsule()
        .fill(.white)
        .frame(width: 6, height: 1)
      Capsule()
        .fill(.white)
        .frame(width: 10, height: 1)
    }
    .accessibilityHidden(true)
  }
}
