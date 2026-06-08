import 'dart:async' show Timer, unawaited;

import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'package:unfydqry/unfydqry.dart';

void main() => runApp(const SearchSampleApp());

class SearchSampleApp extends StatelessWidget {
  const SearchSampleApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'unfydqry Flutter Sample',
      theme: ThemeData(colorSchemeSeed: Colors.deepPurple, useMaterial3: true),
      home: const SearchPage(),
    );
  }
}

/// Field slots for the record-layer API. Stable, never renumbered.
const _slotName = 0;
const _slotYomi = 1;
const _fieldCount = 2;

String _slotLabel(int slot) => switch (slot) {
      _slotName => '名前',
      _slotYomi => 'よみ',
      _ => 'slot $slot',
    };

/// A multi-field record: a name and its reading (yomi). The same seed is used
/// across the iOS, Android, and Flutter samples so results can be compared.
typedef SeedRecord = ({int id, String name, String yomi});

const _seed = <SeedRecord>[
  (id: 1, name: '東京タワー', yomi: 'とうきょうたわー'),
  (id: 2, name: 'スカイツリー', yomi: 'すかいつりー'),
  (id: 3, name: '大阪城', yomi: 'おおさかじょう'),
  (id: 4, name: '名古屋テレビ塔', yomi: 'なごやてれびとう'),
  (id: 5, name: '札幌時計台', yomi: 'さっぽろとけいだい'),
  (id: 6, name: 'コーヒーサーバー', yomi: 'こーひーさーばー'),
  (id: 7, name: 'データベース', yomi: 'でーたべーす'),
  (id: 8, name: 'プリンター', yomi: 'ぷりんたー'),
];

/// One normalization step toggle, bound to a field of [NormalizeOptions].
typedef StepToggle = ({
  String label,
  bool Function(NormalizeOptions) get,
  NormalizeOptions Function(NormalizeOptions, bool) set,
});

// Top-level getters/setters for the step toggles (const tear-offs can't close
// over instance state, so each step binds to a plain top-level function).
bool _getLowercase(NormalizeOptions o) => o.lowercase;
NormalizeOptions _setLowercase(NormalizeOptions o, bool v) => o.copyWith(lowercase: v);
bool _getKanaFold(NormalizeOptions o) => o.kanaFold;
NormalizeOptions _setKanaFold(NormalizeOptions o, bool v) => o.copyWith(kanaFold: v);
bool _getFoldDiacritics(NormalizeOptions o) => o.foldDiacritics;
NormalizeOptions _setFoldDiacritics(NormalizeOptions o, bool v) => o.copyWith(foldDiacritics: v);
bool _getFoldChoonpu(NormalizeOptions o) => o.foldChoonpu;
NormalizeOptions _setFoldChoonpu(NormalizeOptions o, bool v) => o.copyWith(foldChoonpu: v);
bool _getExpandIterationMarks(NormalizeOptions o) => o.expandIterationMarks;
NormalizeOptions _setExpandIterationMarks(NormalizeOptions o, bool v) => o.copyWith(expandIterationMarks: v);
bool _getNormalizeHyphens(NormalizeOptions o) => o.normalizeHyphens;
NormalizeOptions _setNormalizeHyphens(NormalizeOptions o, bool v) => o.copyWith(normalizeHyphens: v);
bool _getStripDigitGrouping(NormalizeOptions o) => o.stripDigitGrouping;
NormalizeOptions _setStripDigitGrouping(NormalizeOptions o, bool v) => o.copyWith(stripDigitGrouping: v);
bool _getCollapseWhitespace(NormalizeOptions o) => o.collapseWhitespace;
NormalizeOptions _setCollapseWhitespace(NormalizeOptions o, bool v) => o.copyWith(collapseWhitespace: v);

const _stepToggles = <StepToggle>[
  (label: '小文字化', get: _getLowercase, set: _setLowercase),
  (label: 'カナ→かな', get: _getKanaFold, set: _setKanaFold),
  (label: 'アクセント除去 (café→cafe)', get: _getFoldDiacritics, set: _setFoldDiacritics),
  (label: '長音畳み込み (サーバー→サーバ)', get: _getFoldChoonpu, set: _setFoldChoonpu),
  (label: '繰り返し記号展開 (時々→時時)', get: _getExpandIterationMarks, set: _setExpandIterationMarks),
  (label: 'ハイフン統一', get: _getNormalizeHyphens, set: _setNormalizeHyphens),
  (label: '桁区切り除去 (1,000→1000)', get: _getStripDigitGrouping, set: _setStripDigitGrouping),
  (label: '空白圧縮', get: _getCollapseWhitespace, set: _setCollapseWhitespace),
];

/// A search result row: the record plus which of its fields matched.
///
/// [highlights] holds the marked normalized text per matched slot, as produced
/// by the engine's `highlight` (matches wrapped in [_hlOpen]/[_hlClose]).
/// A slot is present only when the query actually matched its field.
typedef ResultRow = ({
  SeedRecord record,
  List<int> matchedSlots,
  Map<int, String> highlights,
});

/// The engine packs (recordId, slot) into the document id it highlights under.
/// The sample opens with the default config, so fieldBits is the library
/// default (8); the packed id is `recordId << fieldBits | slot`.
const _fieldBits = 8;

/// Sentinel markers wrapped around matched regions by the engine's `highlight`,
/// chosen from C0 control characters so they never collide with real content.
const _hlOpen = '\u0002'; // STX
const _hlClose = '\u0003'; // ETX

const _matchStyle = TextStyle(
  backgroundColor: Color(0x80FFEB3B), // translucent yellow
  fontWeight: FontWeight.bold,
);

/// Builds a [TextSpan] tree from text marked with [_hlOpen]/[_hlClose],
/// emphasizing the matched spans. An optional [prefix] (e.g. "よみ: ") is
/// prepended unstyled, and [base] applies to the whole span.
TextSpan _highlightedSpan(String marked, {String prefix = '', TextStyle? base}) {
  final children = <TextSpan>[];
  if (prefix.isNotEmpty) children.add(TextSpan(text: prefix));
  final buf = StringBuffer();
  var inMatch = false;
  void flush() {
    if (buf.isEmpty) return;
    children.add(TextSpan(text: buf.toString(), style: inMatch ? _matchStyle : null));
    buf.clear();
  }

  for (final rune in marked.runes) {
    switch (rune) {
      case 0x02:
        flush();
        inMatch = true;
      case 0x03:
        flush();
        inMatch = false;
      default:
        buf.writeCharCode(rune);
    }
  }
  flush();
  return TextSpan(style: base, children: children);
}

class SearchPage extends StatefulWidget {
  const SearchPage({super.key});

  @override
  State<SearchPage> createState() => _SearchPageState();
}

class _SearchPageState extends State<SearchPage> {
  SearchEngine? _engine;
  String _dbPath = '';
  final _queryCtrl = TextEditingController();
  Timer? _debounce;

  // `_options` is the pending selection the toggles reflect; `_applied` is what
  // the engine/index are built with. Changing options only flags whether a
  // reindex is needed (detected via reindexStatus) — it does not rebuild.
  NormalizeOptions _options = const NormalizeOptions.loose();
  NormalizeOptions _applied = const NormalizeOptions.loose();
  SearchStrategy _strategy = SearchStrategy.trigramBm25;
  bool _needsReindex = false;

  String _status = 'initializing…';
  List<ResultRow> _results = const [];
  late final Map<int, SeedRecord> _byId = {for (final d in _seed) d.id: d};

  // All records as rows, used when the query is empty.
  List<ResultRow> get _allRows =>
      (_seed.toList()..sort((a, b) => a.id.compareTo(b.id)))
          .map((r) => (
                record: r,
                matchedSlots: const <int>[],
                highlights: const <int, String>{},
              ))
          .toList();

  @override
  void initState() {
    super.initState();
    _init();
  }

  Future<void> _init() async {
    final dir = await getApplicationDocumentsDirectory();
    _dbPath = '${dir.path}/search_index.sqlite';
    // Open with the applied options/strategy, regenerating the stored documents
    // in place if a previous run used a different normalization profile.
    final engine = await SearchEngine.openWithOptionsRebuilding(
      _dbPath,
      options: _applied,
      strategy: _strategy,
    );
    for (final doc in _seed) {
      // Each record is indexed as multiple fields; the engine packs
      // (recordId, slot) internally and returns recordIds from searchRecords.
      await engine.indexRecord(doc.id, [
        FieldValue(slot: _slotName, text: doc.name),
        FieldValue(slot: _slotYomi, text: doc.yomi),
      ]);
    }
    if (!mounted) {
      await engine.dispose();
      return;
    }
    setState(() {
      _engine = engine;
      _results = _allRows;
      _status = '全件表示 (${_results.length})';
    });
  }

  Future<void> _runSearch() async {
    final engine = _engine;
    if (engine == null) return;
    final query = _queryCtrl.text;
    if (query.trim().isEmpty) {
      setState(() {
        _results = _allRows;
        _status = '全件表示 (${_results.length})';
      });
      return;
    }
    // Record-layer search: hits collapse to one row per record, with the
    // matched field slots. The host re-fetches records by id from the seed.
    final hits = await engine.searchRecords(query, fieldsPerRecord: _fieldCount);
    final rows = <ResultRow>[];
    for (final h in hits) {
      final record = _byId[h.recordId];
      if (record == null) continue;
      rows.add((
        record: record,
        matchedSlots: h.matchedSlots,
        highlights: await _highlightsFor(engine, query, h.recordId, h.matchedSlots),
      ));
    }
    // Results reflect the *applied* normalization until a reindex.
    final normalized = await SearchEngine.normalize(query, options: _applied);
    if (!mounted) return;
    setState(() {
      _results = rows;
      _status = 'hits: ${rows.length}  normalized="$normalized"';
    });
  }

  /// Asks the engine to highlight [query] within each matched field of
  /// [recordId], keyed by slot. Slots whose normalized field does not actually
  /// contain a marked match are dropped, so the UI falls back to the raw text.
  Future<Map<int, String>> _highlightsFor(
    SearchEngine engine,
    String query,
    int recordId,
    List<int> matchedSlots,
  ) async {
    final out = <int, String>{};
    for (final slot in matchedSlots) {
      final id = (recordId << _fieldBits) | slot;
      final marked = await engine.highlight(id, query, before: _hlOpen, after: _hlClose);
      if (marked != null && marked.contains(_hlOpen)) {
        out[slot] = marked;
      }
    }
    return out;
  }

  /// Strategy isn't part of the index fingerprint, so apply it immediately by
  /// reopening with the applied options and the new strategy (no reindex).
  Future<void> _applyStrategy(SearchStrategy newStrategy) async {
    final old = _engine;
    final engine = await SearchEngine.openWithOptions(
      _dbPath,
      options: _applied,
      strategy: newStrategy,
    );
    await old?.dispose();
    if (!mounted) {
      await engine.dispose();
      return;
    }
    setState(() {
      _engine = engine;
      _strategy = newStrategy;
    });
    await _runSearch();
  }

  /// Apply the pending options by regenerating the index in place, then clear the flag.
  Future<void> _doReindex() async {
    final old = _engine;
    final engine = await SearchEngine.openWithOptionsRebuilding(
      _dbPath,
      options: _options,
      strategy: _strategy,
    );
    await old?.dispose();
    if (!mounted) {
      await engine.dispose();
      return;
    }
    setState(() {
      _engine = engine;
      _applied = _options;
      _needsReindex = false;
      _status = 'インデックスを再生成しました';
    });
    await _runSearch();
  }

  void _onQueryChanged(String _) {
    setState(() {}); // refresh the clear-button visibility
    // Incremental search: debounce keystrokes so a search runs shortly after
    // typing settles rather than on every character.
    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 150), _runSearch);
  }

  Future<void> _openSettings() async {
    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (ctx) => _SettingsSheet(
        initialOptions: _options,
        initialStrategy: _strategy,
        initialNeedsReindex: _needsReindex,
        // Pure check; also lets the main-screen banner stay in sync via onOptionsChanged.
        checkReindex: (opts) => SearchEngine.reindexStatus(_dbPath, options: opts),
        onOptionsChanged: (opts, needsReindex) {
          if (!mounted) return;
          setState(() {
            _options = opts;
            _needsReindex = needsReindex;
          });
        },
        onStrategy: _applyStrategy,
        onReindex: _doReindex,
      ),
    );
  }

  @override
  void dispose() {
    _debounce?.cancel();
    unawaited(_engine?.dispose() ?? Future.value());
    _queryCtrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      appBar: AppBar(
        title: const Text('SearchSample'),
        actions: [
          TextButton(onPressed: _openSettings, child: const Text('設定')),
        ],
      ),
      body: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (_needsReindex)
              Container(
                width: double.infinity,
                color: theme.colorScheme.tertiaryContainer,
                padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                child: Row(
                  children: [
                    Expanded(
                      child: Text(
                        '正規化設定が変更されました。再生成が必要です。',
                        style: theme.textTheme.bodySmall,
                      ),
                    ),
                    TextButton(onPressed: _doReindex, child: const Text('再生成')),
                  ],
                ),
              ),
            const SizedBox(height: 8),
            TextField(
              controller: _queryCtrl,
              decoration: InputDecoration(
                labelText: '検索 (全角/半角/カナ/ひら、なんでも)',
                border: const OutlineInputBorder(),
                suffixIcon: _queryCtrl.text.isEmpty
                    ? null
                    : IconButton(
                        icon: const Icon(Icons.clear),
                        onPressed: () {
                          _queryCtrl.clear();
                          _runSearch();
                        },
                      ),
              ),
              onChanged: _onQueryChanged,
              onSubmitted: (_) => _runSearch(),
            ),
            const SizedBox(height: 4),
            Text(_status, style: theme.textTheme.bodySmall),
            const SizedBox(height: 8),
            Expanded(
              child: ListView.builder(
                itemCount: _results.length,
                itemBuilder: (ctx, i) {
                  final r = _results[i];
                  final matched = r.matchedSlots.isEmpty
                      ? ''
                      : '  一致: ${r.matchedSlots.map(_slotLabel).join(', ')}';
                  final nameHl = r.highlights[_slotName];
                  final yomiHl = r.highlights[_slotYomi];
                  final yomiStyle = theme.textTheme.bodySmall?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  );
                  return Padding(
                    padding: const EdgeInsets.symmetric(vertical: 6),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        // Matched fields show the engine's highlighted (normalized)
                        // text; unmatched fields fall back to the raw record text.
                        if (nameHl != null)
                          Text.rich(_highlightedSpan(nameHl, base: theme.textTheme.bodyLarge))
                        else
                          Text(r.record.name, style: theme.textTheme.bodyLarge),
                        if (yomiHl != null)
                          Text.rich(_highlightedSpan(yomiHl, prefix: 'よみ: ', base: yomiStyle))
                        else
                          Text('よみ: ${r.record.yomi}', style: yomiStyle),
                        Text(
                          'id=${r.record.id}$matched',
                          style: theme.textTheme.bodySmall?.copyWith(
                            color: theme.colorScheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ),
                  );
                },
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// The settings bottom sheet: normalization step toggles + search strategy
/// selection + reindex action. Mirrors the iOS/Android sample settings UI.
///
/// Owns its own editing state so the switches/dropdown update while the sheet
/// is open (the parent's `setState` cannot rebuild a `showModalBottomSheet`
/// subtree). Changes are mirrored back to the parent via the callbacks so the
/// main-screen reindex banner stays in sync.
class _SettingsSheet extends StatefulWidget {
  const _SettingsSheet({
    required this.initialOptions,
    required this.initialStrategy,
    required this.initialNeedsReindex,
    required this.checkReindex,
    required this.onOptionsChanged,
    required this.onStrategy,
    required this.onReindex,
  });

  final NormalizeOptions initialOptions;
  final SearchStrategy initialStrategy;
  final bool initialNeedsReindex;
  final Future<ReindexStatus> Function(NormalizeOptions) checkReindex;
  final void Function(NormalizeOptions options, bool needsReindex) onOptionsChanged;
  final ValueChanged<SearchStrategy> onStrategy;
  final VoidCallback onReindex;

  @override
  State<_SettingsSheet> createState() => _SettingsSheetState();
}

class _SettingsSheetState extends State<_SettingsSheet> {
  late NormalizeOptions _options = widget.initialOptions;
  late SearchStrategy _strategy = widget.initialStrategy;
  late bool _needsReindex = widget.initialNeedsReindex;

  Future<void> _toggle(NormalizeOptions newOptions) async {
    setState(() => _options = newOptions);
    final status = await widget.checkReindex(newOptions);
    final needsReindex = status == ReindexStatus.configChanged;
    if (!mounted) return;
    setState(() => _needsReindex = needsReindex);
    widget.onOptionsChanged(newOptions, needsReindex);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return SafeArea(
      child: SingleChildScrollView(
        padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('正規化ステップ', style: theme.textTheme.titleSmall),
            const SizedBox(height: 8),
            for (final step in _stepToggles)
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                dense: true,
                title: Text(step.label, style: theme.textTheme.bodyMedium),
                value: step.get(_options),
                onChanged: (v) => _toggle(step.set(_options, v)),
              ),
            const SizedBox(height: 8),
            const Divider(),
            const SizedBox(height: 8),
            Text('検索アルゴリズム', style: theme.textTheme.titleSmall),
            const SizedBox(height: 4),
            DropdownButton<SearchStrategy>(
              value: _strategy,
              isExpanded: true,
              onChanged: (s) {
                if (s == null) return;
                setState(() => _strategy = s);
                widget.onStrategy(s);
              },
              items: [
                for (final s in SearchStrategy.values)
                  DropdownMenuItem(value: s, child: Text(s.label)),
              ],
            ),
            const SizedBox(height: 16),
            if (_needsReindex)
              FilledButton(
                onPressed: () {
                  Navigator.of(context).pop();
                  widget.onReindex();
                },
                child: const Text('インデックス再生成 (必要)'),
              )
            else
              OutlinedButton(
                onPressed: () {
                  Navigator.of(context).pop();
                  widget.onReindex();
                },
                child: const Text('インデックス再生成'),
              ),
            const SizedBox(height: 4),
            Text(
              _needsReindex
                  ? '正規化設定が変更されています。再生成すると現在の設定が反映されます。'
                  : '保存済みの生テキストから現在の設定で再生成します。',
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
