part of 'kit.dart';

/// A confirmation: a Colecto card on desktop and web, a Cupertino alert on iOS
/// and a Material 3 dialog on Android. Returns true when confirmed.
Future<bool> showColectoConfirm({
  required BuildContext context,
  required String title,
  required String message,
  required String confirmLabel,
  bool danger = false,
}) async {
  final style = colectoPlatformStyle(context);
  switch (style) {
    case ColectoPlatformStyle.ios:
      final ok = await showCupertinoDialog<bool>(
        context: context,
        builder: (ctx) => CupertinoAlertDialog(
          title: Text(title),
          content: Text(message),
          actions: [
            CupertinoDialogAction(
              onPressed: () => Navigator.of(ctx).pop(false),
              child: const Text('Cancel'),
            ),
            CupertinoDialogAction(
              isDestructiveAction: danger,
              isDefaultAction: !danger,
              onPressed: () => Navigator.of(ctx).pop(true),
              child: Text(confirmLabel),
            ),
          ],
        ),
      );
      return ok ?? false;
    case ColectoPlatformStyle.android:
      final ok = await showDialog<bool>(
        context: context,
        builder: (ctx) => AlertDialog(
          title: Text(title),
          content: Text(message),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(ctx).pop(false),
              child: const Text('Cancel'),
            ),
            TextButton(
              onPressed: () => Navigator.of(ctx).pop(true),
              child: Text(
                confirmLabel,
                style: danger
                    ? TextStyle(color: ColectoPalette.of(ctx).danger)
                    : null,
              ),
            ),
          ],
        ),
      );
      return ok ?? false;
    case ColectoPlatformStyle.desktop ||
        ColectoPlatformStyle.fluent ||
        ColectoPlatformStyle.macos:
      final ok = await _showDesktopDialog<bool>(
        context: context,
        title: title,
        message: message,
        confirmLabel: confirmLabel,
        danger: danger,
        result: () => true,
      );
      return ok ?? false;
  }
}

/// The desktop designs' alert: `ShadDialog.alert` (Colecto), Fluent
/// `ContentDialog` or `MacosAlertDialog`.
Future<T?> _showDesktopDialog<T>({
  required BuildContext context,
  required String title,
  String? message,
  Widget Function(BuildContext)? content,
  required String confirmLabel,
  bool danger = false,
  required T Function() result,
}) {
  switch (colectoPlatformStyle(context)) {
    case ColectoPlatformStyle.fluent:
      return fl.showDialog<T>(
        context: context,
        barrierDismissible: true,
        builder: (ctx) => fl.ContentDialog(
          title: Text(title),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (message != null) Text(message),
              if (content != null) ...[
                const SizedBox(height: 12),
                content(ctx),
              ],
            ],
          ),
          actions: [
            ColectoButton(
              label: confirmLabel,
              variant: danger
                  ? ColectoButtonVariant.danger
                  : ColectoButtonVariant.primary,
              onPressed: () => Navigator.of(ctx).pop(result()),
            ),
            ColectoButton(
              label: 'Cancel',
              onPressed: () => Navigator.of(ctx).pop(),
            ),
          ],
        ),
      );
    case ColectoPlatformStyle.macos:
      return mac.showMacosAlertDialog<T>(
        context: context,
        barrierDismissible: true,
        builder: (ctx) => mac.MacosAlertDialog(
          appIcon: const ColectoMark(size: 56),
          title: Text(title),
          message: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (message != null) Text(message, textAlign: TextAlign.center),
              if (content != null) ...[
                const SizedBox(height: 12),
                content(ctx),
              ],
            ],
          ),
          primaryButton: mac.PushButton(
            controlSize: mac.ControlSize.large,
            color: danger ? ColectoPalette.of(ctx).danger : null,
            onPressed: () => Navigator.of(ctx).pop(result()),
            child: Text(confirmLabel),
          ),
          secondaryButton: mac.PushButton(
            controlSize: mac.ControlSize.large,
            secondary: true,
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text('Cancel'),
          ),
        ),
      );
    default:
      return shad.showShadDialog<T>(
        context: context,
        builder: (ctx) => shad.ShadDialog.alert(
          title: Text(title),
          description: message == null ? null : Text(message),
          actions: [
            ColectoButton(
              label: 'Cancel',
              onPressed: () => Navigator.of(ctx).pop(),
            ),
            ColectoButton(
              label: confirmLabel,
              variant: danger
                  ? ColectoButtonVariant.danger
                  : ColectoButtonVariant.primary,
              onPressed: () => Navigator.of(ctx).pop(result()),
            ),
          ],
          child: content == null
              ? null
              : Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: content(ctx),
                ),
        ),
      );
  }
}
