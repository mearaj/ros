import 'package:flutter/material.dart';

/// Marks a control surface where drag-select must not compete with scrolling
/// or tapping.
///
/// The app-wide [SelectionArea] makes ordinary text copyable. That is correct
/// for statuses, receipts, and reports, but it steals horizontal drags from
/// tab strips, chip rows, and image grids—leaving later options truncated and
/// unreachable. Wrap only interactive chrome here; never wrap status or report
/// text.
class InteractiveChrome extends StatelessWidget {
  const InteractiveChrome({required this.child, super.key});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return SelectionContainer.disabled(child: child);
  }
}
