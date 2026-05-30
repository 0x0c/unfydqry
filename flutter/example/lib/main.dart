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

/// Same seed data as the iOS and Android samples so results can be compared
/// visually across platforms.
const _seed = [
  (id: 1, text: '東京タワー'),
  (id: 2, text: 'とうきょうスカイツリー'),
  (id: 3, text: 'ﾄｳｷｮｳ ﾄﾞｰﾑ'),
  (id: 4, text: 'Osaka 城'),
  (id: 5, text: 'がっこう ぐらし'),
  (id: 6, text: 'かっこう の歌'),
  (id: 7, text: 'Ｐｙｔｈｏｎ 入門'),
  (id: 8, text: 'ぱんだ と ﾊﾟﾝﾀﾞ'),
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
  List<Hit> _hits = const [];
  late final Map<int, String> _byId = {for (final d in _seed) d.id: d.text};

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
      await engine.index(doc.id, doc.text);
    }
    setState(() {
      _engine = engine;
      _status = 'indexed ${_seed.length} docs';
    });
  }

  Future<void> _search() async {
    final engine = _engine;
    if (engine == null) return;
    final hits = await engine.search(_queryCtrl.text);
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
    // Join engine hits back to the "host store" (the seed list here).
    final results = _hits
        .where((h) => _byId.containsKey(h.id))
        .map((h) => (id: h.id, text: _byId[h.id]!))
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
                labelText: '検索クエリ (全角/半角/カナ/ひら、なんでも)',
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
                  return ListTile(
                    title: Text(r.text),
                    subtitle: Text('id=${r.id}'),
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
