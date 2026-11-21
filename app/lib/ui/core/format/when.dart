/// Mail date labels, ported from Colecto's `lib/when.ts`: a short list label,
/// a relative group heading, and a full reader timestamp.
library;

class When {
  const When({required this.label, required this.group, required this.full});

  /// `11:17`, `Tue`, `Nov 6`, or `Nov 6, 2024`.
  final String label;

  /// `Today`, `Yesterday`, `Previous 7 days`, `October`, `October 2024`.
  final String group;

  /// `Thursday, November 20, 2026 at 08:35`.
  final String full;
}

const _monthsShort = [
  'Jan',
  'Feb',
  'Mar',
  'Apr',
  'May',
  'Jun',
  'Jul',
  'Aug',
  'Sep',
  'Oct',
  'Nov',
  'Dec',
];
const _monthsLong = [
  'January',
  'February',
  'March',
  'April',
  'May',
  'June',
  'July',
  'August',
  'September',
  'October',
  'November',
  'December',
];
const _daysShort = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
const _daysLong = [
  'Monday',
  'Tuesday',
  'Wednesday',
  'Thursday',
  'Friday',
  'Saturday',
  'Sunday',
];

String _two(int n) => n.toString().padLeft(2, '0');

String clockTime(DateTime d) => '${_two(d.hour)}:${_two(d.minute)}';

When when(String raw, {DateTime? now}) {
  final text = raw.trim();
  final current = now ?? DateTime.now();
  if (text.toLowerCase() == 'today') {
    return const When(label: 'Today', group: 'Today', full: 'Today');
  }
  final parsed = DateTime.tryParse(text)?.toLocal();
  if (parsed == null) {
    return When(label: text, group: 'Earlier', full: text);
  }
  final today = DateTime(current.year, current.month, current.day);
  final day = DateTime(parsed.year, parsed.month, parsed.day);
  final days = today.difference(day).inHours ~/ 24;
  final sameYear = parsed.year == current.year;
  final monthDay = '${_monthsShort[parsed.month - 1]} ${parsed.day}';

  final label = days <= 0
      ? clockTime(parsed)
      : days < 7
      ? _daysShort[parsed.weekday - 1]
      : sameYear
      ? monthDay
      : '$monthDay, ${parsed.year}';
  final group = days <= 0
      ? 'Today'
      : days == 1
      ? 'Yesterday'
      : days < 7
      ? 'Previous 7 days'
      : sameYear
      ? _monthsLong[parsed.month - 1]
      : '${_monthsLong[parsed.month - 1]} ${parsed.year}';
  final full =
      '${_daysLong[parsed.weekday - 1]}, ${_monthsLong[parsed.month - 1]} '
      '${parsed.day}, ${parsed.year} at ${clockTime(parsed)}';
  return When(label: label, group: group, full: full);
}

/// A compact reader timestamp: `Today 08:35`, `Yesterday 17:02`,
/// `Tue 09:14`, `Nov 6, 09:14`.
String readerTime(String raw, {DateTime? now}) {
  final parsed = DateTime.tryParse(raw.trim())?.toLocal();
  if (parsed == null) return raw;
  final current = now ?? DateTime.now();
  final today = DateTime(current.year, current.month, current.day);
  final days =
      today
          .difference(DateTime(parsed.year, parsed.month, parsed.day))
          .inHours ~/
      24;
  final time = clockTime(parsed);
  if (days <= 0) return 'Today $time';
  if (days == 1) return 'Yesterday $time';
  if (days < 7) return '${_daysShort[parsed.weekday - 1]} $time';
  final md = '${_monthsShort[parsed.month - 1]} ${parsed.day}';
  return parsed.year == current.year
      ? '$md, $time'
      : '$md, ${parsed.year}, $time';
}

/// `Synced just now`, `Synced 4 min ago`, `Synced 2 h ago`.
String syncedAgo(DateTime at, {DateTime? now}) {
  final diff = (now ?? DateTime.now()).difference(at);
  if (diff.inSeconds < 45) return 'Synced just now';
  if (diff.inMinutes < 60) return 'Synced ${diff.inMinutes} min ago';
  if (diff.inHours < 24) return 'Synced ${diff.inHours} h ago';
  return 'Synced ${diff.inDays} d ago';
}

/// The first grapheme of a name, upper-cased (`initial()` in Colecto).
String initialOf(String name) {
  final trimmed = name.trim();
  if (trimmed.isEmpty) return '?';
  final first = String.fromCharCodes([trimmed.runes.first]);
  return first.toUpperCase();
}

/// The time-of-day greeting for the empty reader.
String greeting({DateTime? now}) {
  final hour = (now ?? DateTime.now()).hour;
  if (hour < 5) return 'Up late?';
  if (hour < 12) return 'Good morning.';
  if (hour < 18) return 'Good afternoon.';
  return 'Good evening.';
}
