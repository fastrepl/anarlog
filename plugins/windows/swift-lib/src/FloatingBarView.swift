import AppKit
import SwiftUI

enum FloatingBarLayout {
  static let inset: CGFloat = 4
  static let screenMargin: CGFloat = 8
  static let compactHeight: CGFloat = 38
  static let compactStopWidth: CGFloat = 62
  static let compactSoloStopWidth: CGFloat = 68
  static let compactIconSize: CGFloat = 30
  static let compactGap: CGFloat = 3
  static let compactHorizontalPadding: CGFloat = 4
  static let compactCornerControlFactor: CGFloat = 0.55228475
  static let expandedWidth: CGFloat = 360
  static let expandedHeight: CGFloat = 430
  static let expandedCornerRadius: CGFloat = 21
  static let expandedPadding: CGFloat = 12
  static let waveformWidth: CGFloat = 26
  static let waveformHeight: CGFloat = 20
  static let stopSquareSize: CGFloat = 9
  static let hoverHandleGap: CGFloat = 2
  static let hoverHandleTopPadding: CGFloat = 7
  static let hoverHandleHeight: CGFloat = 12
  static let hoverHandleReservedHeight: CGFloat =
    hoverHandleTopPadding + hoverHandleHeight + hoverHandleGap
  static let hoverHandleDotSize: CGFloat = 1.6
  static let hoverHandleDotSpacing: CGFloat = 7
  static let hoverHandleHorizontalPadding: CGFloat = 17
  static let dragClickThreshold: CGFloat = 4

  static func compactControlsWidth(showsExpand: Bool) -> CGFloat {
    if showsExpand {
      return compactStopWidth + compactGap + compactIconSize
    }

    return compactSoloStopWidth
  }

  static func compactWidth(showsExpand: Bool) -> CGFloat {
    compactControlsWidth(showsExpand: showsExpand) + compactHorizontalPadding * 2
  }

  static func containerSize(isExpanded: Bool, showsExpand: Bool) -> NSSize {
    if isExpanded {
      return NSSize(
        width: expandedWidth + inset * 2,
        height: expandedHeight + hoverHandleReservedHeight + inset * 2)
    }

    return NSSize(
      width: compactWidth(showsExpand: showsExpand) + inset * 2,
      height: compactHeight + hoverHandleReservedHeight + inset * 2)
  }
}

struct FloatingBarView: View {
  @ObservedObject var model: FloatingBarViewModel
  @ObservedObject var settings: FloatingOverlaySettingsModel
  let panelOrigin: () -> NSPoint?
  let movePanel: (NSPoint) -> Void
  @State private var isBarHovered = false
  @State private var isStopHovered = false
  @State private var shouldAutoScrollTranscript = true
  @State private var suppressNextClick = false
  @State private var dragStart: FloatingBarDragStart?
  private let transcriptBottomAnchorId = "floating-transcript-bottom-anchor"

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
      alignment: .bottomTrailing
    )
    .contentShape(Rectangle())
    .simultaneousGesture(dragClickSuppressor)
    .onHover { isBarHovered = $0 }
  }

  private var compactPill: some View {
    let height =
      FloatingBarLayout.compactHeight
      + (isBarHovered ? FloatingBarLayout.hoverHandleReservedHeight : 0)
    let width = FloatingBarLayout.compactWidth(showsExpand: model.liveCaptionToggleVisible)
    let radius = FloatingBarLayout.compactHeight / 2
    let pillShape = FloatingBarSurfaceShape(
      topRadius: radius,
      bottomRadius: radius,
      cornerControlFactor: FloatingBarLayout.compactCornerControlFactor
    )

    return ZStack(alignment: .bottom) {
      if isBarHovered {
        FloatingBarHoverHandle(
          color: dragHandleDotColor,
          width: width
        )
        .frame(height: FloatingBarLayout.hoverHandleHeight)
        .padding(.top, FloatingBarLayout.hoverHandleTopPadding)
        .frame(
          width: width,
          height: FloatingBarLayout.hoverHandleReservedHeight,
          alignment: .top
        )
        .frame(maxHeight: .infinity, alignment: .top)
        .accessibilityHidden(true)
        .transition(.opacity)
      }

      floatingControls(isExpanded: false)
        .frame(
          width: FloatingBarLayout.compactControlsWidth(
            showsExpand: model.liveCaptionToggleVisible),
          height: FloatingBarLayout.compactHeight
        )
        .frame(
          width: width,
          height: FloatingBarLayout.compactHeight
        )
    }
    .frame(
      width: width,
      height: height,
      alignment: .bottom
    )
    .background(
      pillShape
        .fill(isBarHovered ? envelopeSurfaceColor : surfaceColor)
    )
    .overlay(
      pillShape
        .strokeBorder(outerStrokeColor, lineWidth: 0.5)
    )
    .overlay(
      pillShape
        .strokeBorder(innerStrokeColor, lineWidth: 0.5)
        .padding(1)
    )
    .clipShape(pillShape)
    .animation(.easeOut(duration: 0.12), value: isBarHovered)
  }

  private var expandedPanel: some View {
    let surfaceShape = FloatingBarSurfaceShape(
      topRadius: FloatingBarLayout.expandedCornerRadius,
      bottomRadius: FloatingBarLayout.expandedCornerRadius
    )

    return VStack(spacing: FloatingBarLayout.hoverHandleGap) {
      FloatingBarHoverHandle(
        color: dragHandleDotColor,
        width: FloatingBarLayout.expandedWidth
      )
      .opacity(isBarHovered ? 1 : 0)
      .scaleEffect(isBarHovered ? 1 : 0.92)
      .accessibilityHidden(true)

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
          .padding(.leading, FloatingBarLayout.expandedPadding + 4)
          .padding(
            .trailing,
            FloatingBarLayout.compactControlsWidth(showsExpand: model.liveCaptionToggleVisible)
              + 12
          )
          .frame(height: FloatingBarLayout.compactHeight)

          ScrollViewReader { proxy in
            ZStack(alignment: .bottom) {
              ScrollView(.vertical, showsIndicators: false) {
                VStack(spacing: 8) {
                  ForEach(Array(model.transcriptBubbles.enumerated()), id: \.element.id) {
                    index, bubble in
                    TranscriptBubbleView(
                      bubble: bubble,
                      showsSpeakerLabel: showsSpeakerLabel(at: index),
                      colorScheme: model.colorScheme
                    )
                    .id(bubble.id)
                  }
                  Color.clear
                    .frame(height: FloatingBarLayout.expandedPadding)
                    .id(transcriptBottomAnchorId)
                }
                .frame(maxWidth: .infinity, alignment: .bottom)
                .background(
                  TranscriptScrollObserver(isPinnedToBottom: $shouldAutoScrollTranscript)
                )
              }
              .frame(maxWidth: .infinity, maxHeight: .infinity)
              .onChange(of: model.transcriptBubbles.last?.id) { _, bubbleId in
                if bubbleId != nil, shouldAutoScrollTranscript {
                  proxy.scrollTo(transcriptBottomAnchorId, anchor: .bottom)
                }
              }

              if !shouldAutoScrollTranscript, model.transcriptBubbles.last?.id != nil {
                transcriptBottomChip {
                  performClick {
                    withAnimation(.easeOut(duration: 0.16)) {
                      proxy.scrollTo(transcriptBottomAnchorId, anchor: .bottom)
                    }
                    shouldAutoScrollTranscript = true
                  }
                }
                .padding(.bottom, 0)
                .transition(.move(edge: .bottom))
              }
            }
            .animation(.easeOut(duration: 0.12), value: shouldAutoScrollTranscript)
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
          .padding(.trailing, FloatingBarLayout.compactHorizontalPadding)
      }
      .frame(
        width: FloatingBarLayout.expandedWidth,
        height: FloatingBarLayout.expandedHeight,
        alignment: .top
      )
    }
    .padding(.top, FloatingBarLayout.hoverHandleTopPadding)
    .frame(
      width: FloatingBarLayout.expandedWidth,
      height: FloatingBarLayout.expandedHeight
        + (isBarHovered ? FloatingBarLayout.hoverHandleReservedHeight : 0),
      alignment: .bottom
    )
    .background(
      surfaceShape
        .fill(surfaceColor)
    )
    .overlay(
      surfaceShape
        .strokeBorder(outerStrokeColor, lineWidth: 0.5)
    )
    .overlay(
      surfaceShape
        .strokeBorder(innerStrokeColor, lineWidth: 0.5)
        .padding(1)
    )
    .clipShape(surfaceShape)
    .animation(.easeOut(duration: 0.12), value: isBarHovered)
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
          HStack(spacing: 6) {
            Image(systemName: "stop.fill")
              .font(.system(size: FloatingBarLayout.stopSquareSize, weight: .bold))
            Text("Stop")
              .font(.system(size: 12, weight: .semibold))
          }
          .foregroundStyle(stopColor)
        } else if model.status == .error {
          ErrorMark(color: errorAccentColor)
            .frame(
              width: FloatingBarLayout.waveformWidth,
              height: FloatingBarLayout.waveformHeight
            )
        } else {
          DancingBars(color: accentColor, amplitude: model.amplitude)
            .frame(
              width: FloatingBarLayout.waveformWidth,
              height: FloatingBarLayout.waveformHeight
            )
        }
      }
      .frame(width: width, height: height)
      .background(
        Capsule(style: .continuous)
          .fill(isStopHovered ? accentColor.opacity(0.18) : controlHoverFill)
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
      return Color(red: 0.43, green: 0.44, blue: 0.40).opacity(primarySurfaceOpacity)
    }

    return Color(red: 0.86, green: 0.85, blue: 0.82).opacity(primarySurfaceOpacity)
  }

  private var envelopeSurfaceColor: Color {
    if model.colorScheme == .dark {
      return Color(red: 0.43, green: 0.44, blue: 0.40).opacity(envelopeSurfaceOpacity)
    }

    return Color(red: 0.86, green: 0.85, blue: 0.82).opacity(envelopeSurfaceOpacity)
  }

  private var primarySurfaceOpacity: Double {
    settings.floatingBarOpacity * 0.82
  }

  private var envelopeSurfaceOpacity: Double {
    min(settings.floatingBarOpacity * 1.08, FloatingOverlayOpacity.maxFloatingBar)
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

  private var dragHandleDotColor: Color {
    primaryContentColor.opacity(model.colorScheme == .dark ? 0.48 : 0.36)
  }

  private var dragHandleSurfaceColor: Color {
    if model.colorScheme == .dark {
      return Color(red: 0.34, green: 0.35, blue: 0.32).opacity(settings.floatingBarOpacity)
    }

    return Color(red: 0.72, green: 0.72, blue: 0.68).opacity(settings.floatingBarOpacity)
  }

  private var stopColor: Color {
    normalAccentColor
  }

  private var errorAccentColor: Color {
    Color(red: 1, green: 0.25, blue: 0.24)
  }

  private var normalAccentColor: Color {
    Color(red: 1, green: 0.20, blue: 0.30)
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

  private func showsSpeakerLabel(at index: Int) -> Bool {
    guard model.transcriptBubbles.indices.contains(index) else { return false }
    guard index > model.transcriptBubbles.startIndex else { return true }

    let bubble = model.transcriptBubbles[index]
    let previousBubble = model.transcriptBubbles[index - 1]
    return bubble.speakerLabel != previousBubble.speakerLabel
      || bubble.isSelf != previousBubble.isSelf
  }

  private func transcriptBottomChip(action: @escaping () -> Void) -> some View {
    Button(action: action) {
      HStack(spacing: 6) {
        Image(systemName: "arrow.down")
          .font(.system(size: 10, weight: .semibold))
        Text("Go to bottom")
          .font(.system(size: 11, weight: .medium))
      }
      .foregroundStyle(primaryContentColor)
      .padding(.horizontal, 12)
      .padding(.vertical, 7)
      .background(
        Capsule(style: .continuous)
          .fill(transcriptChipFillColor)
      )
      .overlay(
        Capsule(style: .continuous)
          .strokeBorder(transcriptChipStrokeColor, lineWidth: 0.5)
      )
    }
    .buttonStyle(.plain)
    .accessibilityLabel("Scroll transcript to bottom")
  }

  private var transcriptChipFillColor: Color {
    if model.colorScheme == .dark {
      return Color(red: 0.18, green: 0.18, blue: 0.17)
    }

    return Color(red: 0.95, green: 0.95, blue: 0.93)
  }

  private var transcriptChipStrokeColor: Color {
    if model.colorScheme == .dark {
      return Color(red: 0.36, green: 0.36, blue: 0.34)
    }

    return Color(red: 0.76, green: 0.75, blue: 0.72)
  }
}

private struct FloatingBarDragStart {
  let panelOrigin: NSPoint
  let mouseLocation: NSPoint
}

private struct FloatingBarSurfaceShape: InsettableShape {
  let topRadius: CGFloat
  let bottomRadius: CGFloat
  let cornerControlFactor: CGFloat
  var insetAmount: CGFloat = 0

  init(
    topRadius: CGFloat,
    bottomRadius: CGFloat,
    cornerControlFactor: CGFloat = 0.447715,
    insetAmount: CGFloat = 0
  ) {
    self.topRadius = topRadius
    self.bottomRadius = bottomRadius
    self.cornerControlFactor = cornerControlFactor
    self.insetAmount = insetAmount
  }

  func path(in rect: CGRect) -> Path {
    let insetRect = rect.insetBy(dx: insetAmount, dy: insetAmount)
    let topRadius = min(topRadius, insetRect.width / 2, insetRect.height / 2)
    let bottomRadius = min(bottomRadius, insetRect.width / 2, insetRect.height / 2)
    let topControl = topRadius * cornerControlFactor
    let bottomControl = bottomRadius * cornerControlFactor
    var path = Path()

    path.move(to: CGPoint(x: insetRect.minX + topRadius, y: insetRect.minY))
    path.addLine(to: CGPoint(x: insetRect.maxX - topRadius, y: insetRect.minY))
    path.addCurve(
      to: CGPoint(x: insetRect.maxX, y: insetRect.minY + topRadius),
      control1: CGPoint(x: insetRect.maxX - topRadius + topControl, y: insetRect.minY),
      control2: CGPoint(x: insetRect.maxX, y: insetRect.minY + topRadius - topControl)
    )
    path.addLine(to: CGPoint(x: insetRect.maxX, y: insetRect.maxY - bottomRadius))
    path.addCurve(
      to: CGPoint(x: insetRect.maxX - bottomRadius, y: insetRect.maxY),
      control1: CGPoint(x: insetRect.maxX, y: insetRect.maxY - bottomRadius + bottomControl),
      control2: CGPoint(x: insetRect.maxX - bottomRadius + bottomControl, y: insetRect.maxY)
    )
    path.addLine(to: CGPoint(x: insetRect.minX + bottomRadius, y: insetRect.maxY))
    path.addCurve(
      to: CGPoint(x: insetRect.minX, y: insetRect.maxY - bottomRadius),
      control1: CGPoint(x: insetRect.minX + bottomRadius - bottomControl, y: insetRect.maxY),
      control2: CGPoint(x: insetRect.minX, y: insetRect.maxY - bottomRadius + bottomControl)
    )
    path.addLine(to: CGPoint(x: insetRect.minX, y: insetRect.minY + topRadius))
    path.addCurve(
      to: CGPoint(x: insetRect.minX + topRadius, y: insetRect.minY),
      control1: CGPoint(x: insetRect.minX, y: insetRect.minY + topRadius - topControl),
      control2: CGPoint(x: insetRect.minX + topRadius - topControl, y: insetRect.minY)
    )
    path.closeSubpath()
    return path
  }

  func inset(by amount: CGFloat) -> FloatingBarSurfaceShape {
    FloatingBarSurfaceShape(
      topRadius: topRadius,
      bottomRadius: bottomRadius,
      cornerControlFactor: cornerControlFactor,
      insetAmount: insetAmount + amount
    )
  }
}

private struct FloatingBarHoverHandle: View {
  let color: Color
  let width: CGFloat

  var body: some View {
    FloatingBarDotPattern(color: color)
      .frame(
        width: max(0, width - FloatingBarLayout.hoverHandleHorizontalPadding * 2),
        height: FloatingBarLayout.hoverHandleHeight
      )
      .padding(.horizontal, FloatingBarLayout.hoverHandleHorizontalPadding)
  }
}

private struct FloatingBarDotPattern: View {
  let color: Color

  var body: some View {
    Canvas { context, size in
      var y = FloatingBarLayout.hoverHandleDotSize / 2
      while y <= size.height {
        var x = FloatingBarLayout.hoverHandleDotSize / 2
        while x <= size.width {
          let rect = CGRect(
            x: x - FloatingBarLayout.hoverHandleDotSize / 2,
            y: y - FloatingBarLayout.hoverHandleDotSize / 2,
            width: FloatingBarLayout.hoverHandleDotSize,
            height: FloatingBarLayout.hoverHandleDotSize
          )
          context.fill(Path(ellipseIn: rect), with: .color(color))
          x += FloatingBarLayout.hoverHandleDotSpacing
        }
        y += FloatingBarLayout.hoverHandleDotSpacing
      }
    }
  }
}

private struct TranscriptScrollObserver: NSViewRepresentable {
  @Binding var isPinnedToBottom: Bool

  func makeCoordinator() -> Coordinator {
    Coordinator()
  }

  func makeNSView(context: Context) -> NSView {
    let view = NSView()
    DispatchQueue.main.async {
      context.coordinator.bind(to: view.enclosingScrollView)
    }
    return view
  }

  func updateNSView(_ view: NSView, context: Context) {
    context.coordinator.isPinnedToBottom = $isPinnedToBottom
    DispatchQueue.main.async {
      context.coordinator.bind(to: view.enclosingScrollView)
    }
  }

  final class Coordinator {
    var isPinnedToBottom: Binding<Bool>?
    private weak var scrollView: NSScrollView?
    private var boundsObserver: NSObjectProtocol?
    private let threshold: CGFloat = 20

    deinit {
      if let boundsObserver {
        NotificationCenter.default.removeObserver(boundsObserver)
      }
    }

    func bind(to scrollView: NSScrollView?) {
      guard self.scrollView !== scrollView else { return }

      if let boundsObserver {
        NotificationCenter.default.removeObserver(boundsObserver)
      }

      self.scrollView = scrollView
      guard let scrollView else { return }

      scrollView.contentView.postsBoundsChangedNotifications = true
      boundsObserver = NotificationCenter.default.addObserver(
        forName: NSView.boundsDidChangeNotification,
        object: scrollView.contentView,
        queue: .main
      ) { [weak self] _ in
        self?.updatePinnedState()
      }

      updatePinnedState()
    }

    func updatePinnedState() {
      guard let scrollView, let documentView = scrollView.documentView else { return }

      let visibleRect = scrollView.documentVisibleRect
      let documentBounds = documentView.bounds
      let isPinned: Bool
      if documentView.isFlipped {
        isPinned = visibleRect.maxY >= documentBounds.maxY - threshold
      } else {
        isPinned = visibleRect.minY <= documentBounds.minY + threshold
      }

      if isPinnedToBottom?.wrappedValue != isPinned {
        isPinnedToBottom?.wrappedValue = isPinned
      }
    }
  }
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
  let showsSpeakerLabel: Bool
  let colorScheme: FloatingBarColorScheme

  var body: some View {
    HStack {
      if bubble.isSelf {
        Spacer(minLength: 40)
      }

      VStack(alignment: bubble.isSelf ? .trailing : .leading, spacing: 4) {
        if showsSpeakerLabel {
          Text(bubble.speakerLabel)
            .font(.system(size: 10, weight: .semibold))
            .foregroundStyle(Color.white)
            .lineLimit(1)
            .padding(.horizontal, 3)
        }

        Text(bubble.text)
          .font(.system(size: 13, weight: .regular))
          .foregroundStyle(Color.white)
          .multilineTextAlignment(.leading)
          .frame(maxWidth: .infinity, alignment: .leading)
          .fixedSize(horizontal: false, vertical: true)
          .padding(.horizontal, 11)
          .padding(.vertical, 8)
          .background(bubbleBackground)
          .clipShape(RoundedRectangle(cornerRadius: 11, style: .continuous))
      }

      if !bubble.isSelf {
        Spacer(minLength: 40)
      }
    }
  }

  private var bubbleBackground: Color {
    if bubble.isSelf {
      return Color.black.opacity(colorScheme == .dark ? 0.34 : 0.24)
    }

    return Color.black.opacity(colorScheme == .dark ? 0.28 : 0.2)
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
  private let minHeight: CGFloat = 4
  private let maxHeight: CGFloat = 20

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
