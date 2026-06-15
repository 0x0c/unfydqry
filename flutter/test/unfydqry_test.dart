import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:unfydqry/unfydqry.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late List<MethodCall> log;

  setUp(() {
    log = [];
    int nextHandle = 0;

    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
      const MethodChannel('unfydqry/search'),
      (call) async {
        log.add(call);
        switch (call.method) {
          case 'open':
          case 'openWithOptions':
          case 'openWithOptionsRebuilding':
            return nextHandle++;
          case 'index':
          case 'remove':
          case 'dispose':
            return null;
          case 'indexBatch':
            return (call.arguments['items'] as List).length;
          case 'removeBatch':
            return (call.arguments['ids'] as List).length;
          case 'indexRecordsBatch':
            return (call.arguments['records'] as List).length;
          case 'search':
            return [
              {'id': 1, 'score': -1.521},
              {'id': 7, 'score': -2.103},
            ];
          case 'normalizeWithOptions':
            return 'ｐｙｔｈｏｎ';
          case 'reindexStatusWithOptions':
            return 'CONFIG_CHANGED';
          default:
            return null;
        }
      },
    );
  });

  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
            const MethodChannel('unfydqry/search'), null);
  });

  group('SearchEngine.open', () {
    test('sends dbPath argument', () async {
      await SearchEngine.open('/data/search.sqlite');
      expect(log.last.method, 'open');
      expect(log.last.arguments['dbPath'], '/data/search.sqlite');
    });

    test('returns an engine instance', () async {
      final engine = await SearchEngine.open('/tmp/db.sqlite');
      expect(engine, isNotNull);
      await engine.dispose();
    });
  });

  group('SearchEngine.index', () {
    test('sends handle, id, and text', () async {
      final engine = await SearchEngine.open('/tmp/db.sqlite');
      await engine.index(42, 'Ｐｙｔｈｏｮ 入門');

      final call = log.last;
      expect(call.method, 'index');
      expect(call.arguments['id'], 42);
      expect(call.arguments['text'], 'Ｐｙｔｈｏｮ 入門');
      await engine.dispose();
    });
  });

  group('SearchEngine.remove', () {
    test('sends handle and id', () async {
      final engine = await SearchEngine.open('/tmp/db.sqlite');
      await engine.remove(7);

      final call = log.last;
      expect(call.method, 'remove');
      expect(call.arguments['id'], 7);
      await engine.dispose();
    });
  });

  group('SearchEngine.indexBatch', () {
    test('forwards items as id/text maps and returns the count', () async {
      final engine = await SearchEngine.open('/tmp/db.sqlite');
      final n = await engine.indexBatch(const [
        IndexItem(id: 1, text: 'a'),
        IndexItem(id: 2, text: 'b'),
      ]);

      final call = log.last;
      expect(call.method, 'indexBatch');
      expect(call.arguments['items'], [
        {'id': 1, 'text': 'a'},
        {'id': 2, 'text': 'b'},
      ]);
      expect(n, 2);
      await engine.dispose();
    });
  });

  group('SearchEngine.removeBatch', () {
    test('forwards the id list and returns the count', () async {
      final engine = await SearchEngine.open('/tmp/db.sqlite');
      final n = await engine.removeBatch([3, 4, 5]);

      final call = log.last;
      expect(call.method, 'removeBatch');
      expect(call.arguments['ids'], [3, 4, 5]);
      expect(n, 3);
      await engine.dispose();
    });
  });

  group('SearchEngine.indexRecordsBatch', () {
    test('forwards records as recordId/fields maps and returns the count', () async {
      final engine = await SearchEngine.open('/tmp/db.sqlite');
      final n = await engine.indexRecordsBatch(const [
        RecordIndexItem(recordId: 1, fields: [
          FieldValue(slot: 0, text: '東京タワー'),
          FieldValue(slot: 1, text: 'とうきょうたわー'),
        ]),
      ]);

      final call = log.last;
      expect(call.method, 'indexRecordsBatch');
      expect(call.arguments['records'], [
        {
          'recordId': 1,
          'fields': [
            {'slot': 0, 'text': '東京タワー'},
            {'slot': 1, 'text': 'とうきょうたわー'},
          ],
        },
      ]);
      expect(n, 1);
      await engine.dispose();
    });
  });

  group('SearchEngine.search', () {
    test('returns Hit list from native response', () async {
      final engine = await SearchEngine.open('/tmp/db.sqlite');
      final hits = await engine.search('python');

      expect(hits, hasLength(2));
      expect(hits[0].id, 1);
      expect(hits[0].score, closeTo(-1.521, 1e-6));
      expect(hits[1].id, 7);
      await engine.dispose();
    });

    test('forwards query and default limit', () async {
      final engine = await SearchEngine.open('/tmp/db.sqlite');
      await engine.search('tokyo');

      final call = log.last;
      expect(call.arguments['query'], 'tokyo');
      expect(call.arguments['limit'], 50);
      await engine.dispose();
    });

    test('forwards custom limit', () async {
      final engine = await SearchEngine.open('/tmp/db.sqlite');
      await engine.search('kana', limit: 10);

      expect(log.last.arguments['limit'], 10);
      await engine.dispose();
    });

    test('returns empty list when native returns null', () async {
      // Override handler to simulate no results.
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(
        const MethodChannel('unfydqry/search'),
        (call) async => call.method == 'open' ? 0 : null,
      );

      final engine = await SearchEngine.open('/tmp/db.sqlite');
      final hits = await engine.search('nothing');
      expect(hits, isEmpty);
    });
  });

  group('SearchEngine.dispose', () {
    test('sends handle on dispose', () async {
      final engine = await SearchEngine.open('/tmp/db.sqlite');
      await engine.dispose();
      expect(log.last.method, 'dispose');
    });

    test('is idempotent — second dispose is a no-op', () async {
      final engine = await SearchEngine.open('/tmp/db.sqlite');
      await engine.dispose();
      log.clear();
      await engine.dispose();
      expect(log, isEmpty);
    });

    test('methods after dispose throw StateError', () async {
      final engine = await SearchEngine.open('/tmp/db.sqlite');
      await engine.dispose();
      expect(() => engine.index(1, 't'), throwsStateError);
      expect(() => engine.remove(1), throwsStateError);
      expect(() => engine.search('q'), throwsStateError);
      expect(() => engine.indexBatch(const []), throwsStateError);
      expect(() => engine.removeBatch(const []), throwsStateError);
      expect(() => engine.indexRecordsBatch(const []), throwsStateError);
    });
  });

  group('SearchEngine.search malformed payload', () {
    test('wraps a bad hit shape as SearchException', () async {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(
        const MethodChannel('unfydqry/search'),
        (call) async => call.method == 'open'
            ? 0
            : [
                {'id': 'not-a-number'},
              ],
      );

      final engine = await SearchEngine.open('/tmp/db.sqlite');
      await expectLater(
        engine.search('q'),
        throwsA(isA<SearchException>()),
      );
    });
  });

  group('Multiple engines', () {
    test('each engine gets a unique handle', () async {
      final a = await SearchEngine.open('/tmp/a.sqlite');
      final b = await SearchEngine.open('/tmp/b.sqlite');

      await a.index(1, 'first engine');
      final aHandle = log.firstWhere((c) => c.method == 'index').arguments['handle'];

      await b.index(2, 'second engine');
      final bHandle = log.lastWhere((c) => c.method == 'index').arguments['handle'];

      expect(aHandle, isNot(equals(bHandle)));

      await a.dispose();
      await b.dispose();
    });
  });

  group('NormalizeOptions', () {
    test('loose preset folds case and kana only', () {
      const o = NormalizeOptions.loose();
      expect(o.lowercase, isTrue);
      expect(o.kanaFold, isTrue);
      expect(o.foldDiacritics, isFalse);
      expect(o.collapseWhitespace, isFalse);
    });

    test('copyWith replaces only the given fields', () {
      const o = NormalizeOptions.loose();
      final n = o.copyWith(foldChoonpu: true);
      expect(n.foldChoonpu, isTrue);
      expect(n.lowercase, isTrue); // unchanged
      expect(o.foldChoonpu, isFalse); // original untouched
    });

    test('toMap carries every flag under its wire key', () {
      final map = const NormalizeOptions.loose().toMap();
      expect(map['lowercase'], isTrue);
      expect(map['kanaFold'], isTrue);
      expect(map.keys, containsAll(<String>[
        'lowercase',
        'kanaFold',
        'foldDiacritics',
        'foldChoonpu',
        'expandIterationMarks',
        'normalizeHyphens',
        'stripDigitGrouping',
        'collapseWhitespace',
      ]));
    });
  });

  group('SearchEngine.openWithOptions', () {
    test('forwards dbPath, options map, and strategy wire name', () async {
      final engine = await SearchEngine.openWithOptions(
        '/tmp/db.sqlite',
        options: const NormalizeOptions.loose(),
        strategy: SearchStrategy.substring,
      );
      final call = log.last;
      expect(call.method, 'openWithOptions');
      expect(call.arguments['dbPath'], '/tmp/db.sqlite');
      expect(call.arguments['strategy'], 'SUBSTRING');
      expect(call.arguments['options']['kanaFold'], isTrue);
      await engine.dispose();
    });

    test('defaults to loose options and trigramBm25', () async {
      final engine = await SearchEngine.openWithOptions('/tmp/db.sqlite');
      expect(log.last.arguments['strategy'], 'TRIGRAM_BM25');
      expect(log.last.arguments['options']['lowercase'], isTrue);
      await engine.dispose();
    });
  });

  group('SearchEngine.openWithOptionsRebuilding', () {
    test('uses the openWithOptionsRebuilding channel method', () async {
      final engine = await SearchEngine.openWithOptionsRebuilding(
        '/tmp/db.sqlite',
        strategy: SearchStrategy.fuzzyTrigram,
      );
      expect(log.last.method, 'openWithOptionsRebuilding');
      expect(log.last.arguments['strategy'], 'FUZZY_TRIGRAM');
      await engine.dispose();
    });
  });

  group('SearchEngine.normalize', () {
    test('forwards input and options, returns native string', () async {
      final out = await SearchEngine.normalize(
        'ＰＹＴＨＯＮ',
        options: const NormalizeOptions.loose(),
      );
      final call = log.last;
      expect(call.method, 'normalizeWithOptions');
      expect(call.arguments['input'], 'ＰＹＴＨＯＮ');
      expect(call.arguments['options']['lowercase'], isTrue);
      expect(out, 'ｐｙｔｈｏｎ');
    });
  });

  group('SearchEngine.reindexStatus', () {
    test('maps the wire name back to a ReindexStatus', () async {
      final status = await SearchEngine.reindexStatus('/tmp/db.sqlite');
      expect(log.last.method, 'reindexStatusWithOptions');
      expect(status, ReindexStatus.configChanged);
    });
  });

  group('SearchStrategy', () {
    test('round-trips through its wire name', () {
      for (final s in SearchStrategy.values) {
        expect(SearchStrategy.fromWire(s.wireName), s);
      }
    });

    test('falls back to trigramBm25 for unknown wire names', () {
      expect(SearchStrategy.fromWire('NOPE'), SearchStrategy.trigramBm25);
    });
  });
}
