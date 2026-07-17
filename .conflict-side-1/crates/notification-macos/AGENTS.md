# notification-macos

Native macOS notification with two forms: Collapsed and Expanded.

## Progress Bar (Auto-dismiss Timer)

- Blue bar at bottom shows time remaining until auto-dismiss
- `progressRatio` (0.0-1.0) is the source of truth for visual state
- Timer runs only in Collapsed form when not hovered
- Pauses on: hover, expand
- Resumes on: unhover (if collapsed), collapse
- Must resume at exact paused position (capture from presentation layer, not recalculate)

## Event Handlers

- `expanded_accept`: User clicks action button in expanded view
- `collapsed_confirm`: User clicks collapsed notification body
- `collapsed_timeout`: Auto-dismiss timer completed
- `dismiss`: User clicks close button
