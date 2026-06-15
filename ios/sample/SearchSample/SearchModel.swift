import Combine
import Foundation
import UnifiedQuery

@MainActor
final class SearchModel: ObservableObject {
    /// Bound to the standard search bar (`.searchable`); changes drive an
    /// incremental, debounced search.
    @Published var query: String = ""
    @Published var status: String = ""
    @Published var results: [ResultRow] = []

    /// The *pending* normalization the toggles reflect. Changing it does NOT
    /// rebuild the index — instead we detect whether a regeneration is needed
    /// (`needsReindex`) and let the user apply it with the reindex button.
    @Published var options: NormalizeOptions = .loose {
        didSet { if options != oldValue { refreshStatus() } }
    }
    /// Strategy only affects the query algorithm, never the stored norms, so a
    /// change applies immediately (no reindex).
    @Published var strategy: StrategyOption = .trigramBm25 {
        didSet { if strategy != oldValue { applyStrategy() } }
    }
    /// True when the pending `options` differ from what the index was built with
    /// (detected via `reindexStatus`). Surfaced in the UI to prompt a reindex.
    @Published var needsReindex: Bool = false
    /// Whether the `extraSeed` batch is currently indexed. Drives the
    /// enabled/disabled state of the bulk add/remove buttons.
    @Published var extraAdded: Bool = false

    private var engine: SearchEngine
    /// The engine packs `(record_id, slot)` into the document id it stores (and
    /// highlights) under. The sample opens with the default config, so the
    /// number of low bits reserved for the slot is the library default (8); the
    /// packed id is `record_id << fieldBits | slot`.
    private static let fieldBits: Int64 = 8
    /// The normalization the engine and on-disk index are currently built with.
    private var applied: NormalizeOptions = .loose
    private let dbPath: String
    /// The engine returns only IDs and scores, so the host side maps id → Record.
    private var store: [Int64: Record] = [:]
    private var cancellables = Set<AnyCancellable>()

    init() {
        let url = FileManager.default
            .urls(for: .documentDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("search_index.sqlite")
        self.dbPath = url.path
        do {
            // Regenerates the index in place from the retained raw text if the
            // stored normalization differs, so the host never re-feeds documents.
            self.engine = try SearchEngine.withOptionsRebuilding(
                dbPath: url.path,
                config: EngineOptionsConfig(normalize: .loose, strategy: .trigramBm25)
            )
        } catch {
            fatalError("open SearchEngine failed: \(error)")
        }
        seed()
        // Incremental search: debounce keystrokes so a search runs shortly after
        // typing settles rather than on every character.
        $query
            .debounce(for: .milliseconds(150), scheduler: RunLoop.main)
            .removeDuplicates()
            .sink { [weak self] _ in self?.search() }
            .store(in: &cancellables)
        search() // show all docs immediately for the initial empty query
        applyEnvHooks()
    }

    /// Detects whether the pending `options` would require regenerating the
    /// index (the stored documents were normalized under a different profile).
    private func refreshStatus() {
        let status = (try? reindexStatusWithOptions(dbPath: dbPath, options: options)) ?? .upToDate
        needsReindex = (status == .configChanged)
    }

    /// Applies a strategy change immediately by reopening with the *applied*
    /// normalization (strategy is not part of the index fingerprint, so this
    /// never needs a reindex).
    private func applyStrategy() {
        do {
            engine = try SearchEngine.withOptions(
                dbPath: dbPath,
                config: EngineOptionsConfig(normalize: applied, strategy: strategy.ffi)
            )
            search()
        } catch {
            status = "strategy error: \(error)"
        }
    }

    private func seed() {
        // Multi-field records (name + reading). The same seed is used across the
        // iOS, Android, and Flutter samples so hits can be compared by id.
        let seed: [Record] = [
            Record(id: 1, name: "東京タワー", yomi: "とうきょうたわー"),
            Record(id: 2, name: "スカイツリー", yomi: "すかいつりー"),
            Record(id: 3, name: "大阪城", yomi: "おおさかじょう"),
            Record(id: 4, name: "名古屋テレビ塔", yomi: "なごやてれびとう"),
            Record(id: 5, name: "札幌時計台", yomi: "さっぽろとけいだい"),
            Record(id: 6, name: "コーヒーサーバー", yomi: "こーひーさーばー"),
            Record(id: 7, name: "データベース", yomi: "でーたべーす"),
            Record(id: 8, name: "プリンター", yomi: "ぷりんたー")
        ]
        // Seed every record in a single transaction with the batch API
        // (`indexRecordsBatch`) instead of one `indexRecord` call per record.
        // Each `RecordIndexItem` carries the host record id plus a `FieldValue`
        // per slot — the engine packs (id, slot) internally and returns record
        // ids from searchRecords. Validation is all-or-nothing: if any record is
        // invalid, nothing is indexed.
        let items = seed.map { record in
            RecordIndexItem(recordId: record.id, fields: [
                FieldValue(slot: RecordSlot.name.rawValue, text: record.name),
                FieldValue(slot: RecordSlot.yomi.rawValue, text: record.yomi)
            ])
        }
        let indexed = (try? engine.indexRecordsBatch(records: items)) ?? 0
        for record in seed { store[record.id] = record }
        status = "indexed \(indexed) records"
    }

    /// A second set of records the bulk-add button indexes — and the bulk-remove
    /// button removes — in one transaction, to demonstrate the batch APIs. Uses
    /// ids distinct from `seed()`.
    private static let extraSeed: [Record] = [
        Record(id: 101, name: "横浜ランドマークタワー", yomi: "よこはまらんどまーくたわー"),
        Record(id: 102, name: "通天閣", yomi: "つうてんかく"),
        Record(id: 103, name: "金閣寺", yomi: "きんかくじ"),
        Record(id: 104, name: "厳島神社", yomi: "いつくしまじんじゃ"),
        Record(id: 105, name: "首里城", yomi: "しゅりじょう")
    ]

    /// Adds `extraSeed` in a single transaction via the record-layer batch API
    /// (`indexRecordsBatch`), which returns the number of records indexed.
    func addExtraBatch() {
        do {
            let items = Self.extraSeed.map { record in
                RecordIndexItem(recordId: record.id, fields: [
                    FieldValue(slot: RecordSlot.name.rawValue, text: record.name),
                    FieldValue(slot: RecordSlot.yomi.rawValue, text: record.yomi)
                ])
            }
            let indexed = try engine.indexRecordsBatch(records: items)
            for record in Self.extraSeed { store[record.id] = record }
            extraAdded = true
            status = "一括追加: \(indexed) 件"
            search()
        } catch {
            status = "batch add error: \(error)"
        }
    }

    /// Removes `extraSeed` in a single transaction via `removeBatch`. That API
    /// works at the packed document-id layer, so each record is expanded into
    /// its `(recordId, slot)` ids (`recordId << fieldBits | slot`); the call
    /// returns the number of document ids processed.
    func removeExtraBatch() {
        do {
            let ids = Self.extraSeed.flatMap { record in
                [RecordSlot.name.rawValue, RecordSlot.yomi.rawValue].map { slot in
                    (record.id << Self.fieldBits) | Int64(slot)
                }
            }
            let removed = try engine.removeBatch(ids: ids)
            for record in Self.extraSeed { store.removeValue(forKey: record.id) }
            extraAdded = false
            status = "一括削除: \(removed) ドキュメント"
            search()
        } catch {
            status = "batch remove error: \(error)"
        }
    }

    /// Applies the pending `options` by regenerating the index in place from the
    /// retained raw text (`withOptionsRebuilding`), then clears `needsReindex`.
    func reindex() {
        do {
            engine = try SearchEngine.withOptionsRebuilding(
                dbPath: dbPath,
                config: EngineOptionsConfig(normalize: options, strategy: strategy.ffi)
            )
            applied = options
            needsReindex = false
            status = "インデックスを再生成しました"
            search()
        } catch {
            status = "reindex error: \(error)"
        }
    }

    func search() {
        guard !query.isEmpty else {
            // Empty query → show every indexed record (sorted by id for stability).
            results = store.values
                .sorted { $0.id < $1.id }
                .map { ResultRow(record: $0, matchedSlots: [], highlights: [:]) }
            status = "全件表示 (\(results.count))"
            return
        }
        do {
            let hits = try engine.searchRecords(
                query: query, limit: 50, fieldsPerRecord: RecordSlot.fieldCount
            )
            results = hits.compactMap { hit in
                guard let record = store[hit.recordId] else { return nil }
                // The FFI returns matched slots as a byte buffer (Data); expose them
                // as [UInt8] so the UI can map each slot to a label.
                let slots = [UInt8](hit.matchedSlots)
                return ResultRow(
                    record: record,
                    matchedSlots: slots,
                    highlights: highlights(recordId: hit.recordId, slots: slots)
                )
            }
            // Total matching documents, unbounded by the result limit ("About N
            // results" UI). This counts at the document (field/slot) layer, so it
            // can exceed the record-row count when a record matches in several
            // fields (e.g. both name and yomi).
            let total = (try? engine.matchCount(query: query)) ?? 0
            // Results reflect the *applied* normalization until a reindex.
            let normalized = normalizeWithOptions(input: query, options: applied)
            status = "hits: \(results.count)  全マッチ文書: \(total)  normalized=\u{0022}\(normalized)\u{0022}"
        } catch {
            status = "error: \(error)"
            results = []
        }
    }

    /// Asks the engine to highlight the current `query` within each matched
    /// field of `recordId`, keyed by slot. Slots whose normalized field does not
    /// actually contain a marked match are dropped, so the UI falls back to the
    /// raw text for them rather than showing a marker-free normalized string.
    private func highlights(recordId: Int64, slots: [UInt8]) -> [UInt8: String] {
        var result: [UInt8: String] = [:]
        for slot in slots {
            let id = (recordId << Self.fieldBits) | Int64(slot)
            let marked = (try? engine.highlight(
                query: query, id: id, before: Highlight.open, after: Highlight.close
            )) ?? nil
            if let marked, marked.contains(Highlight.open) {
                result[slot] = marked
            }
        }
        return result
    }

    /// UI-test hooks: preselect steps/strategy and/or a query on launch.
    /// SEARCH_OPTIONS is a comma-separated step id list (see `OptionToggle.all`).
    private func applyEnvHooks() {
        let env = ProcessInfo.processInfo.environment
        guard env["SEARCH_AUTO_QUERY"] != nil || env["SEARCH_OPTIONS"] != nil
            || env["SEARCH_STRATEGY"] != nil else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { [weak self] in
            guard let self else { return }
            if let s = env["SEARCH_STRATEGY"].flatMap(StrategyOption.init(rawValue:)) {
                self.strategy = s
            }
            if let raw = env["SEARCH_OPTIONS"] {
                // Sets the pending options only; whether this needs a reindex is
                // detected and surfaced (banner), matching a real toggle change.
                self.options = NormalizeOptions(stepIds: raw)
            }
            if let auto = env["SEARCH_AUTO_QUERY"] {
                self.query = auto
                self.search()
            }
        }
    }
}
