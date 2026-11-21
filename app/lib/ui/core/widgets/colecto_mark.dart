import 'package:flutter/widgets.dart';

import '../theme/colecto_palette.dart';

/// The mark, shared with Colecto (`docs/branding/logo-square.svg`): an open
/// mint C with a paper plane flying through it. Drawn from the SVG's own
/// geometry so it stays crisp at every size without an SVG renderer.
class ColectoMark extends StatelessWidget {
  const ColectoMark({super.key, this.size = 24});

  final double size;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      image: true,
      label: 'Colecto',
      child: SizedBox.square(
        dimension: size,
        child: const CustomPaint(painter: _MarkPainter()),
      ),
    );
  }
}

class _MarkPainter extends CustomPainter {
  const _MarkPainter();

  static const _tile = Color(0xFF1C211E);
  static const _mint = Color(0xFFA2D6B1);
  static const _planeLight = Color(0xFFE7EDE8);
  static const _planeShade = Color(0xFFB7C6BB);

  @override
  void paint(Canvas canvas, Size size) {
    canvas.scale(size.width / 120);
    // <rect x="6" y="6" width="108" height="108" rx="28" fill="#1C211E" />
    canvas.drawRRect(
      RRect.fromLTRBR(6, 6, 114, 114, const Radius.circular(28)),
      Paint()..color = _tile,
    );

    final arcPaint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeCap = StrokeCap.round;

    // Top arc behind the plane: M84 36A34 34 0 0 0 36 36
    final topArc = Path()
      ..moveTo(84, 36)
      ..arcToPoint(
        const Offset(36, 36),
        radius: const Radius.circular(34),
        largeArc: false,
        clockwise: false,
      );
    canvas.drawPath(
      topArc,
      arcPaint
        ..strokeWidth = 10
        ..color = _mint,
    );

    // Paper plane: knockout outline, light upper wing, shaded lower wing.
    final planeOutline = Path()
      ..moveTo(100, 30)
      ..lineTo(26.4, 54.5)
      ..lineTo(54.8, 63.8)
      ..lineTo(55.8, 93.7)
      ..close();
    _fillAndStroke(canvas, planeOutline, _tile, 6.8);

    final lightWing = Path()
      ..moveTo(100, 30)
      ..lineTo(26.4, 54.5)
      ..lineTo(54.8, 63.8)
      ..close();
    _fillAndStroke(canvas, lightWing, _planeLight, 2.4);

    final shadeWing = Path()
      ..moveTo(100, 30)
      ..lineTo(55.8, 93.7)
      ..lineTo(54.8, 63.8)
      ..close();
    _fillAndStroke(canvas, shadeWing, _planeShade, 2.4);

    // Foreground C cutout: M40.5 32.1A34 34 0 1 0 84 84
    final cutoutArc = Path()
      ..moveTo(40.5, 32.1)
      ..arcToPoint(
        const Offset(84, 84),
        radius: const Radius.circular(34),
        largeArc: true,
        clockwise: false,
      );
    canvas.drawPath(
      cutoutArc,
      arcPaint
        ..strokeWidth = 14.4
        ..color = _tile,
    );

    // Foreground C stroke: M46.7 28.7A34 34 0 1 0 84 84
    final frontArc = Path()
      ..moveTo(46.7, 28.7)
      ..arcToPoint(
        const Offset(84, 84),
        radius: const Radius.circular(34),
        largeArc: true,
        clockwise: false,
      );
    canvas.drawPath(
      frontArc,
      arcPaint
        ..strokeWidth = 10
        ..color = _mint,
    );
  }

  static void _fillAndStroke(
    Canvas canvas,
    Path path,
    Color color,
    double strokeWidth,
  ) {
    canvas.drawPath(path, Paint()..color = color);
    canvas.drawPath(
      path,
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = strokeWidth
        ..strokeCap = StrokeCap.round
        ..strokeJoin = StrokeJoin.round
        ..color = color,
    );
  }

  @override
  bool shouldRepaint(_MarkPainter old) => false;
}

/// Mark plus the name, used in the top bar.
class ColectoBrand extends StatelessWidget {
  const ColectoBrand({super.key, this.markSize = 22});

  final double markSize;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        ColectoMark(size: markSize),
        const SizedBox(width: 8),
        Text(
          'Colecto',
          style: kStyle(
            14.5,
            weight: 660,
            width: 108,
            color: p.ink,
            letterSpacing: -0.2,
          ),
        ),
      ],
    );
  }
}
