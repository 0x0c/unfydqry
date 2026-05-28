import UnifiedQuery
import SwiftUI

/// Minimal record that stands in for the app's "source-of-truth DB".
/// In a real app this would be a SwiftData / Core Data entity.
struct Record: Identifiable, Hashable {
    let id: Int64
    let text: String
}

@MainActor
final class SearchModel: ObservableObject {
    @Published var query: String = ""
    @Published var status: String = ""
    @Published var results: [Record] = []

    private let engine: SearchEngine
    /// The engine returns only IDs and scores, so the host side maps id → Record.
    /// A miniature implementation of design doc §1.3 ("return IDs only / re-fetch from
    /// the source-of-truth DB").
    private var store: [Int64: Record] = [:]

    init() {
        let url = FileManager.default
            .urls(for: .documentDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("search_index.sqlite")
        do {
            // Explicitly select the normalize profile + search strategy.
            // This pair reproduces the default behaviour; swap them to change it.
            self.engine = try SearchEngine.withConfig(
                dbPath: url.path,
                config: EngineConfig(normalize: .loose, strategy: .trigramBm25)
            )
        } catch {
            fatalError("open SearchEngine failed: \(error)")
        }
        seedIfNeeded()
    }

    private func seedIfNeeded() {
        let seed: [Record] = [
            Record(id: 1, text: "東京タワー"),
            Record(id: 2, text: "とうきょうスカイツリー"),
            Record(id: 3, text: "ﾄｳｷｮｳ ﾄﾞｰﾑ"),
            Record(id: 4, text: "Osaka 城"),
            Record(id: 5, text: "がっこう ぐらし"),
            Record(id: 6, text: "かっこう の歌"),
            Record(id: 7, text: "Ｐｙｔｈｏｎ 入門"),
            Record(id: 8, text: "ぱんだ と ﾊﾟﾝﾀﾞ")
        ]
        for record in seed {
            try? engine.index(id: record.id, text: record.text)
            store[record.id] = record
        }
        status = "indexed \(seed.count) docs"
        if let auto = ProcessInfo.processInfo.environment["SEARCH_AUTO_QUERY"] {
            query = auto
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { [weak self] in
                self?.search()
            }
        }
    }

    func search() {
        do {
            let hits = try engine.search(query: query, limit: 50)
            // Re-fetch records from the host store by ID (skip any that were dropped).
            results = hits.compactMap { store[$0.id] }
            status = "hits: \(results.count)  normalized=\u{0022}\(normalizeLoose(input: query))\u{0022}"
        } catch {
            status = "error: \(error)"
            results = []
        }
    }
}

struct ContentView: View {
    @StateObject private var model = SearchModel()

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 12) {
                TextField("検索クエリ(全角/半角/カナ/ひら、なんでも)", text: $model.query)
                    .textFieldStyle(.roundedBorder)
                    .onSubmit { model.search() }
                Button("検索") { model.search() }
                    .buttonStyle(.borderedProminent)
                Text(model.status)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                List(model.results) { record in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(record.text)
                            .font(.body)
                        Text("id=\(record.id)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .monospacedDigit()
                    }
                    .padding(.vertical, 2)
                }
            }
            .padding()
            .navigationTitle("SearchSample")
        }
    }
}
