import Cocoa

extension NotificationManager {
  func createExpandedNotificationView(notification: NotificationInstance) -> NSView {
    let container = NSStackView()
    container.orientation = .vertical
    container.alignment = .leading
    container.distribution = .fill
    container.spacing = 12

    let headerStack = NSStackView()
    headerStack.orientation = .horizontal
    headerStack.alignment = .centerY
    headerStack.distribution = .fill
    headerStack.spacing = 8

    let title = notification.payload.eventDetails?.what ?? notification.payload.title
    let titleLabel = NSTextField(labelWithString: title)
    titleLabel.font = NSFont.systemFont(ofSize: Fonts.expandedTitleSize, weight: Fonts.titleWeight)
    titleLabel.textColor = NSColor.labelColor
    titleLabel.lineBreakMode = .byTruncatingTail
    titleLabel.maximumNumberOfLines = 1
    titleLabel.setContentHuggingPriority(.defaultLow, for: .horizontal)
    titleLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

    let collapseButton = CollapseButton()
    collapseButton.notification = notification
    collapseButton.setContentHuggingPriority(.required, for: .horizontal)

    headerStack.addArrangedSubview(titleLabel)
    headerStack.addArrangedSubview(collapseButton)
    container.addArrangedSubview(headerStack)
    headerStack.widthAnchor.constraint(equalTo: container.widthAnchor).isActive = true

    if let participants = notification.payload.participants, !participants.isEmpty {
      let participantsStack = createParticipantsSection(participants: participants)
      container.addArrangedSubview(participantsStack)
    }

    let separator = NSBox()
    separator.boxType = .separator
    separator.translatesAutoresizingMaskIntoConstraints = false
    container.addArrangedSubview(separator)
    separator.widthAnchor.constraint(equalTo: container.widthAnchor).isActive = true

    if let eventDetails = notification.payload.eventDetails {
      let detailsStack = createDetailsSection(
        eventDetails: eventDetails,
        participants: notification.payload.participants
      )
      container.addArrangedSubview(detailsStack)
    }

    let (actionStack, timerLabel) = createActionSection(notification: notification)
    container.addArrangedSubview(actionStack)
    actionStack.widthAnchor.constraint(equalTo: container.widthAnchor).isActive = true

    notification.bindExpandedTimerLabel(timerLabel)

    return container
  }

  func createParticipantsSection(participants: [Participant]) -> NSStackView {
    let stack = NSStackView()
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 4

    for participant in participants {
      let row = createParticipantRow(participant: participant)
      stack.addArrangedSubview(row)
    }

    return stack
  }

  func createParticipantRow(participant: Participant) -> NSView {
    let row = NSStackView()
    row.orientation = .horizontal
    row.alignment = .centerY
    row.spacing = 6

    let name = participant.name ?? ""
    let displayText = name.isEmpty ? participant.email : "\(name) (\(participant.email))"
    let label = NSTextField(labelWithString: displayText)
    label.font = NSFont.systemFont(ofSize: Fonts.detailValueSize, weight: Fonts.bodyWeight)
    label.textColor = NSColor.labelColor

    let status = ParticipantStatusDisplay(from: participant.status)
    let statusIcon = NSTextField(labelWithString: status.icon)
    statusIcon.font = NSFont.systemFont(ofSize: Fonts.detailValueSize)
    statusIcon.textColor = status.color

    row.addArrangedSubview(label)
    row.addArrangedSubview(statusIcon)

    return row
  }

  func createDetailsSection(eventDetails: EventDetails, participants: [Participant]?)
    -> NSStackView
  {
    let stack = NSStackView()
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 8

    let whatRow = createDetailRow(label: "What:", value: eventDetails.what)
    stack.addArrangedSubview(whatRow)

    if let timezone = eventDetails.timezone {
      let timezoneRow = createDetailRow(label: "Invitee Time Zone:", value: timezone)
      stack.addArrangedSubview(timezoneRow)
    }

    if let participants = participants, !participants.isEmpty {
      let whoValue = participants.map { p in
        let name = p.name ?? ""
        return name.isEmpty ? p.email : "\(name)\n\(p.email)"
      }.joined(separator: "\n")
      let whoRow = createDetailRow(label: "Who:", value: whoValue)
      stack.addArrangedSubview(whoRow)
    }

    if let location = eventDetails.location {
      let whereRow = createDetailRow(label: "Where:", value: location)
      stack.addArrangedSubview(whereRow)
    }

    return stack
  }

  func createDetailRow(label: String, value: String) -> NSView {
    let container = NSStackView()
    container.orientation = .vertical
    container.alignment = .leading
    container.spacing = 2

    let labelField = NSTextField(labelWithString: label)
    labelField.font = NSFont.systemFont(ofSize: Fonts.detailLabelSize, weight: Fonts.buttonWeight)
    labelField.textColor = NSColor.secondaryLabelColor

    let valueField = NSTextField(labelWithString: value)
    valueField.font = NSFont.systemFont(ofSize: Fonts.detailValueSize, weight: Fonts.bodyWeight)
    valueField.textColor = NSColor.labelColor
    valueField.maximumNumberOfLines = 0
    valueField.lineBreakMode = .byWordWrapping

    container.addArrangedSubview(labelField)
    container.addArrangedSubview(valueField)

    return container
  }

  func createActionSection(notification: NotificationInstance) -> (
    NSStackView, NSTextField
  ) {
    let stack = NSStackView()
    stack.orientation = .vertical
    stack.alignment = .centerX
    stack.spacing = 8

    let actionButton = ActionButton()
    let actionLabel = notification.payload.actionLabel ?? "Accept"
    actionButton.title = "  \(actionLabel)"
    actionButton.notification = notification
    actionButton.font = NSFont.systemFont(
      ofSize: Fonts.actionButtonSize, weight: Fonts.buttonWeight)
    actionButton.layer?.cornerRadius = 10
    actionButton.layer?.backgroundColor = Colors.actionButtonBg
    actionButton.contentTintColor = NSColor.white
    actionButton.translatesAutoresizingMaskIntoConstraints = false
    actionButton.heightAnchor.constraint(equalToConstant: 36).isActive = true

    let timerLabel = NSTextField(labelWithString: "")
    timerLabel.font = NSFont.systemFont(ofSize: Fonts.bodySize, weight: Fonts.bodyWeight)
    timerLabel.textColor = NSColor.secondaryLabelColor
    timerLabel.alignment = .center

    stack.addArrangedSubview(actionButton)
    stack.addArrangedSubview(timerLabel)

    actionButton.widthAnchor.constraint(equalTo: stack.widthAnchor, constant: -24).isActive = true

    return (stack, timerLabel)
  }
}
