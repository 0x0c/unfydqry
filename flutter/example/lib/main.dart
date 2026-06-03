import 'dart:async' show unawaited;

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

class SearchPage extends StatefulWidget {
  const SearchPage({super.key});

  @override
  State<SearchPage> createState() => _SearchPageState();
}

class _SearchPageState extends State<SearchPage> {
  SearchEngine? _engine;
  final _queryCtrl = TextEditingController();
  String _status = 'initializing…';
  List<RecordHit> _hits = const [];
  late final Map<int, SeedRecord> _byId = {for (final d in _seed) d.id: d};

  @override
  void initState() {
    super.initState();
    _init();
  }

  Future<void> _init() async {
    final dir = await getApplicationDocumentsDirectory();
    final dbPath = '${dir.path}/search_index.sqlite';
    final engine = await SearchEngine.open(dbPath);
    for (final doc in _seed) {
      // Each record is indexed as multiple fields; the engine packs
      // (recordId, slot) internally and returns recordIds from searchRecords.
      await engine.indexRecord(doc.id, [
        FieldValue(slot: _slotName, text: doc.name),
        FieldValue(slot: _slotYomi, text: doc.yomi),
      ]);
    }
    setState(() {
      _engine = engine;
      _status = 'indexed ${_seed.length} records';
    });
  }

  Future<void> _search() async {
    final engine = _engine;
    if (engine == null) return;
    final hits = await engine.searchRecords(
      _queryCtrl.text,
      fieldsPerRecord: _fieldCount,
    );
    setState(() {
      _hits = hits;
      _status = 'hits: ${hits.length}';
    });
  }

  @override
  void dispose() {
    unawaited(_engine?.dispose() ?? Future.value());
    _queryCtrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    // Join record hits back to the "host store" (the seed list here).
    final results = _hits
        .where((h) => _byId.containsKey(h.recordId))
        .map((h) => (record: _byId[h.recordId]!, hit: h))
        .toList();
    return Scaffold(
      appBar: AppBar(title: const Text('SearchSample')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            TextField(
              controller: _queryCtrl,
              decoration: const InputDecoration(
                labelText: '検索 (名前・よみ どちらにもヒット)',
                border: OutlineInputBorder(),
              ),
              onSubmitted: (_) => _search(),
            ),
            const SizedBox(height: 8),
            FilledButton(onPressed: _search, child: const Text('検索')),
            const SizedBox(height: 8),
            Text(_status, style: Theme.of(context).textTheme.bodySmall),
            const SizedBox(height: 8),
            Expanded(
              child: ListView.builder(
                itemCount: results.length,
                itemBuilder: (ctx, i) {
                  final r = results[i];
                  final matched =
                      r.hit.matchedSlots.map(_slotLabel).join(', ');
                  return ListTile(
                    title: Text(r.record.name),
                    subtitle: Text('よみ: ${r.record.yomi}'),
                    trailing: Text('一致: $matched'),
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
