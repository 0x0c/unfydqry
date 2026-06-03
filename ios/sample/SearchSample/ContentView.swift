import Combine
import UnifiedQuery
import SwiftUI

/// Minimal multi-field record that stands in for the app's "source-of-truth DB".
/// In a real app this would be a SwiftData / Core Data entity with several
/// searchable columns.
struct Record: Identifiable, Hashable {
    let id: Int64
    let name: String
    let yomi: String
}

/// Field slots for the record-layer API. Stable, never renumbered.
let slotName: UInt8 = 0
let slotYomi: UInt8 = 1
let fieldCount: UInt32 = 2

func slotLabel(_ slot: UInt8) -> String {
    switch slot {
    case slotName: return "名前"
    case slotYomi: return "よみ"
    default: return "slot \(slot)"
    }
}

/// A search result row: the matched record plus which of its fields matched.
struct ResultRow: Identifiable, Hashable {
    let record: Record
    let matchedSlots: [UInt8]
    var id: Int64 { record.id }
}

/// UI-facing list of search algorithms, mapped to the FFI enum.
enum StrategyOption: String, CaseIterable, Identifiable {
    case trigramBm25
    case substring
    case prefix
    case suffix
    case allTerms
    case fuzzyTrigram
    case levenshtein
    case damerauLevenshtein

    var id: String { rawValue }

    var label: String {
        switch self {
        case .trigramBm25: return "trigram + bm25"
        case .substring: return "substring"
        case .prefix: return "prefix"
        case .suffix: return "suffix"
        case .allTerms: return "all terms"
        case .fuzzyTrigram: return "fuzzy trigram"
        case .levenshtein: return "levenshtein"
        case .damerauLevenshtein: return "damerau-levenshtein"
        }
    }

    var ffi: SearchStrategy {
        switch self {
        case .trigramBm25: return .trigramBm25
        case .substring: return .substring
        case .prefix: return .prefix
        case .suffix: return .suffix
        case .allTerms: return .allTerms
        case .fuzzyTrigram: return .fuzzyTrigram
        case .levenshtein: return .levenshtein
        case .damerauLevenshtein: return .damerauLevenshtein
        }
    }
}

/// One normalization step toggle, bound to a field of `NormalizeOptions`.
struct OptionToggle: Identifiable {
    let id: String
    let label: String
    let keyPath: WritableKeyPath<NormalizeOptions, Bool>
}

let optionToggles: [OptionToggle] = [
    OptionToggle(id: "lowercase", label: "小文字化", keyPath: \.lowercase),
    OptionToggle(id: "kana_fold", label: "カナ→かな", keyPath: \.kanaFold),
    OptionToggle(id: "fold_diacritics", label: "アクセント除去 (café→cafe)", keyPath: \.foldDiacritics),
    OptionToggle(id: "fold_choonpu", label: "長音畳み込み (サーバー→サーバ)", keyPath: \.foldChoonpu),
    OptionToggle(id: "expand_iteration_marks", label: "繰り返し記号展開 (時々→時時)", keyPath: \.expandIterationMarks),
    OptionToggle(id: "normalize_hyphens", label: "ハイフン統一", keyPath: \.normalizeHyphens),
    OptionToggle(id: "strip_digit_grouping", label: "桁区切り除去 (1,000→1000)", keyPath: \.stripDigitGrouping),
    OptionToggle(id: "collapse_whitespace", label: "空白圧縮", keyPath: \.collapseWhitespace),
]

/// The `loose` preset as composable options (lowercase + kana fold).
private func looseOptions() -> NormalizeOptions {
    NormalizeOptions(
        lowercase: true,
        kanaFold: true,
        foldDiacritics: false,
        foldChoonpu: false,
        expandIterationMarks: false,
        normalizeHyphens: false,
        stripDigitGrouping: false,
        collapseWhitespace: false
    )
}

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
    @Published var options: NormalizeOptions = looseOptions() {
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
    /// The normalization the engine and on-disk index are currently built with.
    private var applied: NormalizeOptions = looseOptions()
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
            self.engine = try SearchModel.makeEngine(
                options: looseOptions(), strategy: .trigramBm25, dbPath: url.path
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

    /// Opens the index for the given composable options. Changing the enabled
    /// steps regenerates the index in place from the retained raw text
    /// (`withOptionsRebuilding`), so the host never re-feeds documents.
    private static func makeEngine(
        options: NormalizeOptions, strategy: SearchStrategy, dbPath: String
    ) throws -> SearchEngine {
        try SearchEngine.withOptionsRebuilding(
            dbPath: dbPath,
            config: EngineOptionsConfig(normalize: options, strategy: strategy)
        )
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
                FieldValue(slot: slotName, text: record.name),
                FieldValue(slot: slotYomi, text: record.yomi)
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
                .map { ResultRow(record: $0, matchedSlots: []) }
            status = "全件表示 (\(results.count))"
            return
        }
        do {
            let hits = try engine.searchRecords(
                query: query, limit: 50, fieldsPerRecord: fieldCount
            )
            results = hits.compactMap { hit in
                store[hit.recordId].map { ResultRow(record: $0, matchedSlots: hit.matchedSlots) }
            }
            // Results reflect the *applied* normalization until a reindex.
            let normalized = normalizeWithOptions(input: query, options: applied)
            status = "hits: \(results.count)  normalized=\u{0022}\(normalized)\u{0022}"
        } catch {
            status = "error: \(error)"
            results = []
        }
    }

    /// UI-test hooks: preselect steps/strategy and/or a query on launch.
    /// SEARCH_OPTIONS is a comma-separated step id list (see `optionToggles`).
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
                self.options = SearchModel.parseOptions(raw)
            }
            if let auto = env["SEARCH_AUTO_QUERY"] {
                self.query = auto
                self.search()
            }
        }
    }

    /// Builds options from a comma-separated list of step ids (see `optionToggles`).
    static func parseOptions(_ raw: String) -> NormalizeOptions {
        let enabled = Set(raw.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) })
        var options = NormalizeOptions(
            lowercase: false, kanaFold: false, foldDiacritics: false, foldChoonpu: false,
            expandIterationMarks: false, normalizeHyphens: false,
            stripDigitGrouping: false, collapseWhitespace: false
        )
        for toggle in optionToggles where enabled.contains(toggle.id) {
            options[keyPath: toggle.keyPath] = true
        }
        return options
    }
}

struct ContentView: View {
    @StateObject private var model = SearchModel()
    // UI-test hook: open the settings sheet on launch when SEARCH_SHOW_SETTINGS is set.
    @State private var showSettings =
        ProcessInfo.processInfo.environment["SEARCH_SHOW_SETTINGS"] != nil

    var body: some View {
        NavigationStack {
            List(model.results) { row in
                VStack(alignment: .leading, spacing: 4) {
                    Text(row.record.name).font(.body)
                    Text("よみ: \(row.record.yomi)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    HStack(spacing: 8) {
                        Text("id=\(row.record.id)")
                            .monospacedDigit()
                        if !row.matchedSlots.isEmpty {
                            Text("一致: \(row.matchedSlots.map(slotLabel).joined(separator: ", "))")
                        }
                    }
                    .font(.caption)
                    .foregroundStyle(.secondary)
                }
            }
            .navigationTitle("SearchSample")
            .searchable(text: $model.query, prompt: "全角/半角/カナ/ひら、なんでも")
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button { showSettings = true } label: {
                        Image(systemName: model.needsReindex
                            ? "gearshape.badge.exclamationmark" : "gearshape")
                    }
                    .accessibilityLabel("設定")
                }
            }
            .safeAreaInset(edge: .top) {
                if model.needsReindex {
                    HStack {
                        Text("正規化設定が変更されました。インデックス再生成が必要です。")
                            .font(.caption)
                        Spacer()
                        Button("再生成") { model.reindex() }
                            .font(.caption.bold())
                    }
                    .padding(.horizontal)
                    .padding(.vertical, 8)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(.yellow.opacity(0.25))
                }
            }
            .safeAreaInset(edge: .bottom) {
                Text(model.status)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal)
                    .padding(.vertical, 6)
                    .background(.bar)
            }
            .sheet(isPresented: $showSettings) {
                SettingsView(model: model)
            }
        }
    }
}

/// Modal settings: normalization step toggles, the search algorithm, and an
/// explicit index-regeneration button.
struct SettingsView: View {
    @ObservedObject var model: SearchModel
    @Environment(\.dismiss) private var dismiss

    private func binding(_ keyPath: WritableKeyPath<NormalizeOptions, Bool>) -> Binding<Bool> {
        Binding(
            get: { model.options[keyPath: keyPath] },
            set: { model.options[keyPath: keyPath] = $0 }
        )
    }

    var body: some View {
        NavigationStack {
            Form {
                Section("正規化ステップ") {
                    ForEach(optionToggles) { toggle in
                        Toggle(toggle.label, isOn: binding(toggle.keyPath))
                            .font(.callout)
                    }
                }
                Section("検索アルゴリズム") {
                    Picker("アルゴリズム", selection: $model.strategy) {
                        ForEach(StrategyOption.allCases) { Text($0.label).tag($0) }
                    }
                }
                Section {
                    if model.needsReindex {
                        Button {
                            model.reindex()
                        } label: {
                            Text("インデックス再生成 (必要)").frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.borderedProminent)
                    } else {
                        Button {
                            model.reindex()
                        } label: {
                            Text("インデックス再生成").frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.bordered)
                    }
                } footer: {
                    Text(model.needsReindex
                        ? "正規化設定が変更されています。再生成すると現在の設定が反映されます。"
                        : "保存済みの生テキストから現在の設定で再生成します。")
                }
            }
            .navigationTitle("設定")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("完了") { dismiss() }
                }
            }
        }
    }
}
