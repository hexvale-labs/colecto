import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:colecto/app.dart';
import 'package:colecto/data/repositories/in_memory_mail_backend.dart';
import 'package:colecto/data/repositories/mail_backend.dart';
import 'package:colecto/ui/core/theme/colecto_palette.dart';
import 'package:colecto/ui/features/shell/mail_controller.dart';

/// Boots [ColectoApp] at the requested logical viewport [size] and [design],
/// registering teardown for both the virtual view and [MailController].
Future<MailController> pumpColectoApp(
  WidgetTester tester, {
  Size size = const Size(1280, 800),
  ColectoPlatformStyle design = ColectoPlatformStyle.desktop,
  MailBackend? backend,
  bool worstCase = false,
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.reset);

  final controller = MailController(
    backend: backend ?? InMemoryMailBackend(worstCase: worstCase),
  )..previewDesign(design);
  await controller.initialize();
  addTearDown(controller.dispose);

  await tester.pumpWidget(ColectoApp(controller: controller));
  await tester.pumpAndSettle();
  return controller;
}
