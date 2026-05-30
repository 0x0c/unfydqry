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
            return nextHandle++;
          case 'index':
          case 'remove':
          case 'dispose':
            return null;
          case 'search':
            return [
              {'id': 1, 'score': -1.521},
              {'id': 7, 'score': -2.103},
            ];
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
}
