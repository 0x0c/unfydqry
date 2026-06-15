import 'field_value.dart';

/// A whole record — a [recordId] plus its [fields] — for the record-layer batch
/// API ([SearchEngine.indexRecordsBatch]).
///
/// Mirrors the native `RecordIndexItem` UniFFI record: each item is the same
/// `(recordId, fields)` a single [SearchEngine.indexRecord] call would take, but
/// a whole list is indexed in one transaction. Validation is all-or-nothing —
/// if any record id or slot is invalid, nothing is indexed.
class RecordIndexItem {
  /// The host record id, same semantics as [SearchEngine.indexRecord].
  final int recordId;

  /// The fields to store for this record (one [FieldValue] per slot).
  final List<FieldValue> fields;

  const RecordIndexItem({required this.recordId, required this.fields});

  Map<String, dynamic> toMap() => {
        'recordId': recordId,
        'fields': fields.map((f) => f.toMap()).toList(),
      };

  @override
  String toString() => 'RecordIndexItem(recordId: $recordId, fields: $fields)';
}
