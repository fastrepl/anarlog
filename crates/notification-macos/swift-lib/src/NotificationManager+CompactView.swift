import Cocoa

extension NotificationManager {
  private func compactPrimaryButton(notification: NotificationInstance) -> NSButton {
    if notification.payload.hasOptions {
      let optionsButton = OptionsButton()
      optionsButton.title = "Options"
      optionsButton.options = notification.payload.options ?? []
      optionsButton.notification = notification
      return optionsButton
    }

    let actionButton = CompactActionButton()
    actionButton.title = notification.payload.actionLabel ?? "Take Notes"
    actionButton.notification = notification
    return actionButton
  }

  private func compactFooterButton() -> NSButton {
    let button = FooterActionButton()
    button.translatesAutoresizingMaskIntoConstraints = false
    button.heightAnchor.constraint(equalToConstant: 20).isActive = true
    button.widthAnchor.constraint(greaterThanOrEqualToConstant: 40).isActive = true
    return button
  }

  func createNotificationView(notification: NotificationInstance) -> NSView {
    let root = NSView()
    root.translatesAutoresizingMaskIntoConstraints = false
    let footer = notification.payload.footer

    let topRow = NSStackView()
    topRow.orientation = .horizontal
    topRow.alignment = .centerY
    topRow.distribution = .fill
    topRow.spacing = 8
    topRow.translatesAutoresizingMaskIntoConstraints = false

    let textStack = NSStackView()
    textStack.orientation = .vertical
    textStack.spacing = 2
    textStack.alignment = .leading
    textStack.distribution = .fill

    textStack.setContentHuggingPriority(.defaultLow, for: .horizontal)
    textStack.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

    let titleLabel = NSTextField(labelWithString: notification.payload.title)
    titleLabel.font = NSFont.systemFont(ofSize: Fonts.titleSize, weight: Fonts.titleWeight)
    titleLabel.textColor = NSColor.labelColor
    titleLabel.lineBreakMode = .byTruncatingTail
    titleLabel.maximumNumberOfLines = 1
    titleLabel.allowsDefaultTighteningForTruncation = true
    titleLabel.usesSingleLineMode = true
    titleLabel.cell?.truncatesLastVisibleLine = true

    titleLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

    textStack.addArrangedSubview(titleLabel)
    if !notification.payload.message.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
      let bodyLabel = NSTextField(labelWithString: notification.payload.message)
      bodyLabel.font = NSFont.systemFont(ofSize: Fonts.bodySize, weight: Fonts.bodyWeight)
      bodyLabel.textColor = NSColor.secondaryLabelColor
      bodyLabel.lineBreakMode = .byTruncatingTail
      bodyLabel.maximumNumberOfLines = 1
      bodyLabel.usesSingleLineMode = true
      bodyLabel.cell?.truncatesLastVisibleLine = true

      bodyLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
      textStack.addArrangedSubview(bodyLabel)
    }

    if let iconImageView = createNotificationIconView(for: notification.payload) {
      let iconContainer = NSView()
      iconContainer.wantsLayer = true
      iconContainer.layer?.cornerRadius = 6
      iconContainer.translatesAutoresizingMaskIntoConstraints = false
      iconContainer.widthAnchor.constraint(equalToConstant: Layout.compactIconContainerSize)
        .isActive = true
      iconContainer.heightAnchor.constraint(equalToConstant: Layout.compactIconContainerSize)
        .isActive = true

      iconContainer.addSubview(iconImageView)
      NSLayoutConstraint.activate([
        iconImageView.centerXAnchor.constraint(equalTo: iconContainer.centerXAnchor),
        iconImageView.centerYAnchor.constraint(equalTo: iconContainer.centerYAnchor),
        iconImageView.widthAnchor.constraint(equalToConstant: Layout.compactIconSize),
        iconImageView.heightAnchor.constraint(equalToConstant: Layout.compactIconSize),
      ])

      topRow.addArrangedSubview(iconContainer)
    }
    topRow.addArrangedSubview(textStack)

    let primaryButton = compactPrimaryButton(notification: notification)
    if let compactActionButton = primaryButton as? CompactActionButton {
      notification.compactActionButton = compactActionButton
    }
    primaryButton.setContentHuggingPriority(.required, for: .horizontal)
    topRow.addArrangedSubview(primaryButton)

    root.addSubview(topRow)
    var constraints = [
      topRow.leadingAnchor.constraint(
        equalTo: root.leadingAnchor, constant: Layout.contentPaddingHorizontal),
      topRow.trailingAnchor.constraint(
        equalTo: root.trailingAnchor, constant: -Layout.contentPaddingHorizontal),
      topRow.topAnchor.constraint(equalTo: root.topAnchor, constant: Layout.contentPaddingVertical),
      topRow.heightAnchor.constraint(
        equalToConstant: Layout.notificationHeight - (Layout.contentPaddingVertical * 2)),
    ]

    if let footer {
      let divider = NSBox()
      divider.boxType = .separator
      divider.translatesAutoresizingMaskIntoConstraints = false

      let footerRow = NSStackView()
      footerRow.orientation = .horizontal
      footerRow.alignment = .centerY
      footerRow.distribution = .fill
      footerRow.spacing = 8
      footerRow.translatesAutoresizingMaskIntoConstraints = false

      let footerLabel = NSTextField(labelWithString: footer.text)
      footerLabel.font = NSFont.systemFont(ofSize: 10, weight: .medium)
      footerLabel.textColor = NSColor.secondaryLabelColor
      footerLabel.lineBreakMode = .byTruncatingTail
      footerLabel.maximumNumberOfLines = 1
      footerLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

      let footerButton = compactFooterButton()
      footerButton.title = footer.actionLabel
      if let footerActionButton = footerButton as? FooterActionButton {
        footerActionButton.notification = notification
      }
      footerButton.setContentHuggingPriority(.required, for: .horizontal)

      footerRow.addArrangedSubview(footerLabel)
      footerRow.addArrangedSubview(footerButton)

      root.addSubview(divider)
      root.addSubview(footerRow)

      constraints.append(contentsOf: [
        divider.leadingAnchor.constraint(equalTo: root.leadingAnchor),
        divider.trailingAnchor.constraint(equalTo: root.trailingAnchor),
        divider.topAnchor.constraint(equalTo: topRow.bottomAnchor, constant: 4),

        footerRow.leadingAnchor.constraint(
          equalTo: root.leadingAnchor, constant: Layout.contentPaddingHorizontal),
        footerRow.trailingAnchor.constraint(
          equalTo: root.trailingAnchor, constant: -Layout.contentPaddingHorizontal),
        footerRow.topAnchor.constraint(equalTo: divider.bottomAnchor, constant: 3),
        footerRow.bottomAnchor.constraint(equalTo: root.bottomAnchor, constant: -4),
        footerRow.heightAnchor.constraint(equalToConstant: 20),
      ])
    } else {
      constraints.append(
        topRow.bottomAnchor.constraint(
          equalTo: root.bottomAnchor, constant: -Layout.contentPaddingVertical)
      )
    }

    NSLayoutConstraint.activate(constraints)

    return root
  }
}
