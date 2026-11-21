import 'package:material_ui/material_ui.dart';

import '../../../domain/models/preferences_models.dart';
import '../theme/colecto_palette.dart';

/// 3x3 visual corner picker for toast notification placement
/// (matching Colecto's `CornerPicker.svelte`).
class CornerPicker extends StatelessWidget {
  const CornerPicker({super.key, required this.value, required this.onChanged});

  final NotificationCorner value;
  final ValueChanged<NotificationCorner> onChanged;

  static const _grid = <List<NotificationCorner?>>[
    [
      NotificationCorner.topLeft,
      NotificationCorner.top,
      NotificationCorner.topRight,
    ],
    [NotificationCorner.left, null, NotificationCorner.right],
    [
      NotificationCorner.bottomLeft,
      NotificationCorner.bottom,
      NotificationCorner.bottomRight,
    ],
  ];

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    return Container(
      width: 156,
      padding: const EdgeInsets.all(6),
      decoration: BoxDecoration(
        color: p.sunk,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: p.line),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (var r = 0; r < 3; r++) ...[
            if (r > 0) const SizedBox(height: 5),
            Row(
              children: [
                for (var c = 0; c < 3; c++) ...[
                  if (c > 0) const SizedBox(width: 5),
                  Expanded(child: _cell(context, p, _grid[r][c])),
                ],
              ],
            ),
          ],
        ],
      ),
    );
  }

  Widget _cell(
    BuildContext context,
    ColectoPalette p,
    NotificationCorner? corner,
  ) {
    if (corner == null) {
      return Container(
        height: 28,
        alignment: Alignment.center,
        child: Icon(Icons.mail_outline, size: 13, color: p.ink3),
      );
    }
    final selected = corner == value;
    return Tooltip(
      message: corner.label,
      child: MouseRegion(
        cursor: SystemMouseCursors.click,
        child: GestureDetector(
          onTap: () => onChanged(corner),
          child: Container(
            height: 28,
            decoration: BoxDecoration(
              color: selected ? p.accent : p.control,
              borderRadius: BorderRadius.circular(5),
              border: Border.all(color: selected ? p.accent : p.line),
            ),
            alignment: Alignment.center,
            child: Container(
              width: 8,
              height: 4,
              decoration: BoxDecoration(
                color: selected ? p.accentInk : p.ink3,
                borderRadius: BorderRadius.circular(2),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
