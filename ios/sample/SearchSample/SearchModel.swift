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
        for record in seed {
            // The engine packs (id, slot) internally; we pass our record id and
            // a slot per field, and get record ids back from searchRecords.
            try? engine.indexRecord(recordId: record.id, fields: [
                FieldValue(slot: RecordSlot.name.rawValue, text: record.name),
                FieldValue(slot: RecordSlot.yomi.rawValue, text: record.yomi)
            ])
            store[record.id] = record
        }
        status = "indexed \(seed.count) records"
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
            // Results reflect the *applied* normalization until a reindex.
            let normalized = normalizeWithOptions(input: query, options: applied)
            status = "hits: \(results.count)  normalized=\u{0022}\(normalized)\u{0022}"
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
