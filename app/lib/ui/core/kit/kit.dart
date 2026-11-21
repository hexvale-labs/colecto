/// The Colecto kit: the components every screen is built from.
///
/// Screens never use a design library directly. Each component here has one
/// API and maps it onto the active design (`colectoPlatformStyle`):
///
/// | Design | Library | Used on |
/// | --- | --- | --- |
/// | `desktop` | `shadcn_ui`, with Colecto's palette | Linux, Web |
/// | `fluent` | `fluent_ui` | Windows |
/// | `macos` | `macos_ui` | macOS |
/// | `android` | `material_ui` (+ `dynamic_color`) | Android |
/// | `ios` | `cupertino_ui` | iOS |
library;

import 'dart:math' as math;
import 'dart:ui' show ImageFilter;

import 'package:cupertino_ui/cupertino_ui.dart';
import 'package:fluent_ui/fluent_ui.dart' as fl;
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:macos_ui/macos_ui.dart' as mac;
import 'package:material_ui/material_ui.dart';
import 'package:shadcn_ui/shadcn_ui.dart' as shad;

import '../format/when.dart';
import '../theme/colecto_palette.dart';
import '../widgets/colecto_mark.dart';

part 'buttons.dart';
part 'controls.dart';
part 'design_scope.dart';
part 'dialogs.dart';
part 'display.dart';
part 'fields.dart';
part 'layout.dart';
part 'menus.dart';
part 'pressable.dart';
part 'text.dart';
