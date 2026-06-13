// End-to-end integration tests for the unfydqry Flutter plugin.
//
// Unlike `flutter/test/unfydqry_test.dart` — which mocks the method channel and
// only exercises the Dart layer — these tests run on a real device/emulator and
// drive the *actual* native plugin (`UnfydqryPlugin.kt` / `.swift`) backed by the
// real Rust/UniFFI core. They are the only place that covers the round trip:
//
//   Dart codec  →  platform-channel  →  native glue (arg parsing, wire-name ↔
//   enum mapping, handle lifecycle, error codes)  →  Rust core  →  back.
//
// The search/normalization *algorithm* itself is already proven by the Rust +
// Swift + Kotlin spec-driven suites, so we deliberately do not re-derive
// `spec/*.json` here. What we assert is that the bridge faithfully carries calls
// and results across the FFI on each platform.
//
// Run with:  flutter test integration_test  (on an attached device/emulator)
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:unfydqry/unfydqry.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  // A fresh temp db path per test; cleaned up (including -wal/-shm) afterwards.
  late Directory tmpDir;

  setUp(() async {
    tmpDir = await Directory.systemTemp.createTemp('unfydqry_it_');
  });

  tearDown(() async {
    if (await tmpDir.exists()) {
      await tmpDir.delete(recursive: true);
    }
  });

  String dbPath([String name = 'index.sqlite']) => '${tmpDir.path}/$name';

  group('lifecycle + query round trip', () {
    testWidgets('open in-memory, index, search, remove', (_) async {
      final engine = await SearchEngine.open(':memory:');
      addTearDown(engine.dispose);

      // Spec scenario "index_then_search_returns_the_doc".
      await engine.index(42, '東京タワー');
      final hits = await engine.search('東京');
      expect(hits.map((h) => h.id), [42]);
      expect(hits.single.score.isFinite, isTrue);

      // After remove the doc is no longer findable.
      await engine.remove(42);
      expect(await engine.search('東京'), isEmpty);
    });

    testWidgets('data persists across reopen of the same file', (_) async {
      final path = dbPath();
      final e1 = await SearchEngine.open(path);
      await e1.index(1, 'とうきょうタワー');
      await e1.dispose();

      final e2 = await SearchEngine.open(path);
      addTearDown(e2.dispose);
      // loose default folds katakana→hiragana, so a hiragana query matches.
      final hits = await e2.search('とうきょう');
      expect(hits.map((h) => h.id), [1]);
    });

    testWidgets('search after dispose throws StateError', (_) async {
      final engine = await SearchEngine.open(':memory:');
      await engine.dispose();
      expect(() => engine.search('x'), throwsStateError);
    });
  });

  group('wire-name mapping (Dart ↔ native enum)', () {
    // The single most drift-prone glue: SearchStrategy.wireName must be a value
    // the native side recognizes. An unknown name yields BAD_ARGS → a thrown
    // PlatformException, so a successful open per strategy proves the mapping.
    for (final strategy in SearchStrategy.values) {
      testWidgets('openWithOptions accepts ${strategy.wireName}', (_) async {
        final engine = await SearchEngine.openWithOptions(
          dbPath('${strategy.wireName}.sqlite'),
          strategy: strategy,
        );
        addTearDown(engine.dispose);
        // Exercise the strategy end to end so the native enum is actually used.
        await engine.index(1, 'tokyo tower');
        expect(() => engine.search('tokyo'), returnsNormally);
      });
    }

    testWidgets('normalize loose folds full-width latin to ascii', (_) async {
      // Per spec/normalize.json `fullwidth_alpha_word`: loose runs NFKC +
      // lowercase, so full-width Latin folds all the way to ASCII.
      final out = await SearchEngine.normalize(
        'ＰＹＴＨＯＮ',
        options: const NormalizeOptions.loose(),
      );
      expect(out, 'python');
    });

    testWidgets('reindexStatus maps native enum names back', (_) async {
      final path = dbPath();

      // Fresh path holds no documents → EMPTY.
      expect(
        await SearchEngine.reindexStatus(path,
            options: const NormalizeOptions.loose()),
        ReindexStatus.empty,
      );

      // Stamp the index under loose options.
      final engine = await SearchEngine.openWithOptions(path,
          options: const NormalizeOptions.loose());
      await engine.index(1, 'カタカナ');
      await engine.dispose();

      // Same options → ready as-is.
      expect(
        await SearchEngine.reindexStatus(path,
            options: const NormalizeOptions.loose()),
        ReindexStatus.upToDate,
      );

      // A different normalization profile → needs regenerating.
      expect(
        await SearchEngine.reindexStatus(
          path,
          options: const NormalizeOptions(lowercase: true, kanaFold: true, foldChoonpu: true),
        ),
        ReindexStatus.configChanged,
      );
    });
  });

  group('record-layer API', () {
    testWidgets('indexRecord / searchRecords / removeRecord', (_) async {
      final engine = await SearchEngine.open(dbPath());
      addTearDown(engine.dispose);

      // Same multi-field seed shape used by the sample app.
      await engine.indexRecord(1, const [
        FieldValue(slot: 0, text: '東京タワー'),
        FieldValue(slot: 1, text: 'とうきょうたわー'),
      ]);

      final records = await engine.searchRecords('とうきょう', fieldsPerRecord: 2);
      expect(records.map((r) => r.recordId), [1]);
      // The reading field (slot 1) is the one that matched.
      expect(records.single.matchedSlots, contains(1));

      await engine.removeRecord(1);
      expect(await engine.searchRecords('とうきょう', fieldsPerRecord: 2), isEmpty);
    });

    testWidgets('highlight wraps matched regions in the raw text', (_) async {
      final engine = await SearchEngine.open(dbPath());
      addTearDown(engine.dispose);

      await engine.index(1, '東京タワー');
      final marked =
          await engine.highlight(1, '東京', before: '[', after: ']');
      expect(marked, isNotNull);
      expect(marked, contains('['));
      expect(marked, contains(']'));
    });

    testWidgets('changeFieldBits repacks existing records', (_) async {
      final engine = await SearchEngine.open(dbPath());
      addTearDown(engine.dispose);

      await engine.indexRecord(1, const [FieldValue(slot: 0, text: '大阪城')]);
      final repacked = await engine.changeFieldBits(10);
      expect(repacked, greaterThanOrEqualTo(1));

      // Record is still findable after repacking.
      final records = await engine.searchRecords('大阪', fieldsPerRecord: 1);
      expect(records.map((r) => r.recordId), [1]);
    });
  });
}
