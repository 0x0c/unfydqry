import 'package:flutter/services.dart';

import 'field_value.dart';
import 'hit.dart';
import 'normalize_options.dart';
import 'record_hit.dart';
import 'reindex_status.dart';
import 'search_exception.dart';
import 'search_strategy.dart';

/// Thin wrapper around the platform's SearchEngine.
///
/// The actual search logic runs in Rust via UniFFI; this class only
/// forwards calls over the method channel.
///
/// ```dart
/// final engine = await SearchEngine.open(dbPath);
/// await engine.index(1, 'Ｐｙｔｈｏｮ 入門');
/// final hits = await engine.search('python');
/// await engine.dispose();
/// ```
class SearchEngine {
  static const _channel = MethodChannel('unfydqry/search');

  int _handle;

  SearchEngine._(this._handle);

  bool get _disposed => _handle < 0;

  void _checkAlive() {
    if (_disposed) throw StateError('SearchEngine used after dispose()');
  }

  /// Opens (or creates) the SQLite FTS index at [dbPath].
  static Future<SearchEngine> open(String dbPath) async {
    final handle = await _channel.invokeMethod<int>('open', {'dbPath': dbPath});
    if (handle == null) throw const SearchException('open returned null handle');
    return SearchEngine._(handle);
  }

  /// Opens the index at [dbPath] applying [options] and [strategy].
  ///
  /// Throws if the stored index was built with different normalization
  /// [options] (a config mismatch). Use [openWithOptionsRebuilding] to adopt
  /// new options by regenerating the stored documents in place.
  static Future<SearchEngine> openWithOptions(
    String dbPath, {
    NormalizeOptions options = const NormalizeOptions.loose(),
    SearchStrategy strategy = SearchStrategy.trigramBm25,
  }) async {
    final handle = await _channel.invokeMethod<int>('openWithOptions', {
      'dbPath': dbPath,
      'options': options.toMap(),
      'strategy': strategy.wireName,
    });
    if (handle == null) throw const SearchException('open returned null handle');
    return SearchEngine._(handle);
  }

  /// Opens the index at [dbPath] with [options]/[strategy], regenerating the
  /// stored documents in place if they were normalized under different options.
  static Future<SearchEngine> openWithOptionsRebuilding(
    String dbPath, {
    NormalizeOptions options = const NormalizeOptions.loose(),
    SearchStrategy strategy = SearchStrategy.trigramBm25,
  }) async {
    final handle =
        await _channel.invokeMethod<int>('openWithOptionsRebuilding', {
      'dbPath': dbPath,
      'options': options.toMap(),
      'strategy': strategy.wireName,
    });
    if (handle == null) throw const SearchException('open returned null handle');
    return SearchEngine._(handle);
  }

  /// Normalizes [input] with [options], returning the transformed text the
  /// engine would index and query against. Pure function — no open engine
  /// needed; useful for previewing the effect of a normalization profile.
  static Future<String> normalize(
    String input, {
    NormalizeOptions options = const NormalizeOptions.loose(),
  }) async {
    final out = await _channel.invokeMethod<String>('normalizeWithOptions', {
      'input': input,
      'options': options.toMap(),
    });
    return out ?? '';
  }

  /// Reports whether the index stored at [dbPath] is consistent with [options],
  /// i.e. whether a reindex is needed before querying with those options.
  static Future<ReindexStatus> reindexStatus(
    String dbPath, {
    NormalizeOptions options = const NormalizeOptions.loose(),
  }) async {
    final wire =
        await _channel.invokeMethod<String>('reindexStatusWithOptions', {
      'dbPath': dbPath,
      'options': options.toMap(),
    });
    return ReindexStatus.fromWire(wire);
  }

  /// Indexes or re-indexes [text] under [id].
  Future<void> index(int id, String text) {
    _checkAlive();
    return _channel.invokeMethod<void>(
      'index',
      {'handle': _handle, 'id': id, 'text': text},
    );
  }

  /// Removes the entry with [id] from the index.
  Future<void> remove(int id) {
    _checkAlive();
    return _channel.invokeMethod<void>(
      'remove',
      {'handle': _handle, 'id': id},
    );
  }

  /// Searches for [query], returning at most [limit] results ordered by relevance.
  Future<List<Hit>> search(String query, {int limit = 50}) async {
    _checkAlive();
    final raw = await _channel.invokeMethod<List<dynamic>>(
      'search',
      {'handle': _handle, 'query': query, 'limit': limit},
    );
    try {
      return (raw ?? [])
          .cast<Map<dynamic, dynamic>>()
          .map((m) => Hit(
                id: (m['id'] as num).toInt(),
                score: (m['score'] as num).toDouble(),
              ))
          .toList();
    } catch (e) {
      throw SearchException('malformed hit payload: $e');
    }
  }

  /// Adds, or replaces, the whole record [recordId] made of multiple [fields].
  ///
  /// Each field is stored under a stable id packing `(recordId, slot)`; fields
  /// empty once normalized are dropped. Re-calling with an existing [recordId]
  /// fully replaces its previous fields.
  Future<void> indexRecord(int recordId, List<FieldValue> fields) {
    _checkAlive();
    return _channel.invokeMethod<void>('indexRecord', {
      'handle': _handle,
      'recordId': recordId,
      'fields': fields.map((f) => f.toMap()).toList(),
    });
  }

  /// Removes every field of [recordId] from the index.
  Future<void> removeRecord(int recordId) {
    _checkAlive();
    return _channel.invokeMethod<void>(
      'removeRecord',
      {'handle': _handle, 'recordId': recordId},
    );
  }

  /// Searches across record fields, returning at most [limit] records ranked by
  /// their best matching field.
  ///
  /// [fieldsPerRecord] is the host's field count, used as an over-fetch hint so
  /// collapsing field hits to records still yields roughly [limit] records.
  Future<List<RecordHit>> searchRecords(
    String query, {
    int limit = 50,
    required int fieldsPerRecord,
  }) async {
    _checkAlive();
    final raw = await _channel.invokeMethod<List<dynamic>>('searchRecords', {
      'handle': _handle,
      'query': query,
      'limit': limit,
      'fieldsPerRecord': fieldsPerRecord,
    });
    try {
      return (raw ?? [])
          .cast<Map<dynamic, dynamic>>()
          .map((m) => RecordHit(
                recordId: (m['recordId'] as num).toInt(),
                score: (m['score'] as num).toDouble(),
                matchedSlots: (m['matchedSlots'] as List<dynamic>)
                    .map((s) => (s as num).toInt())
                    .toList(),
              ))
          .toList();
    } catch (e) {
      throw SearchException('malformed record hit payload: $e');
    }
  }

  /// Returns the normalized text of the document [id] with regions matching
  /// [query] wrapped in [before]/[after] markers.
  ///
  /// Returns null when the document does not exist or [query] is empty once
  /// normalized; returns the plain normalized text when the query does not
  /// match. The returned text is the *normalized* form, not the raw text.
  ///
  /// Records are stored under a packed id; to highlight a record field, pass
  /// `recordId << fieldBits | slot` (the host's `fieldBits`, default 8).
  Future<String?> highlight(
    int id,
    String query, {
    required String before,
    required String after,
  }) {
    _checkAlive();
    return _channel.invokeMethod<String>('highlight', {
      'handle': _handle,
      'id': id,
      'query': query,
      'before': before,
      'after': after,
    });
  }

  /// Re-packs the index from its current `fieldBits` to [newFieldBits],
  /// rebuilding the id encoding in place. Returns the number of documents
  /// repacked. Throws if a stored slot or record id would not fit.
  Future<int> changeFieldBits(int newFieldBits) async {
    _checkAlive();
    final n = await _channel.invokeMethod<int>(
      'changeFieldBits',
      {'handle': _handle, 'newFieldBits': newFieldBits},
    );
    return n ?? 0;
  }

  /// Releases native resources. The engine must not be used after this.
  ///
  /// Idempotent: calling it more than once is a no-op.
  Future<void> dispose() async {
    if (_disposed) return;
    final handle = _handle;
    _handle = -1;
    await _channel.invokeMethod<void>('dispose', {'handle': handle});
  }
}
