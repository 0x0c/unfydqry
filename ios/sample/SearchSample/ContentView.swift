import SwiftUI

@MainActor
final class SearchModel: ObservableObject {
    @Published var query: String = ""
    @Published var status: String = ""
    @Published var results: [Hit] = []

    private let engine: SearchEngine

    init() {
        let url = FileManager.default
            .urls(for: .documentDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("search_index.sqlite")
        do {
            self.engine = try SearchEngine(dbPath: url.path)
        } catch {
            fatalError("open SearchEngine failed: \(error)")
        }
        seedIfNeeded()
    }

    private func seedIfNeeded() {
        // 設計書 §3 の使用イメージそのままに、簡単なシード投入。
        let seed: [(Int64, String)] = [
            (1, "東京タワー"),
            (2, "とうきょうスカイツリー"),
            (3, "ﾄｳｷｮｳ ﾄﾞｰﾑ"),
            (4, "Osaka 城"),
            (5, "がっこう ぐらし"),
            (6, "かっこう の歌"),
            (7, "Ｐｙｔｈｏｎ 入門"),
            (8, "ぱんだ と ﾊﾟﾝﾀﾞ")
        ]
        for (id, text) in seed {
            try? engine.index(id: id, text: text)
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
            results = try engine.search(query: query, limit: 50)
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
                List(model.results, id: \.id) { hit in
                    HStack {
                        Text("id=\(hit.id)")
                        Spacer()
                        Text(String(format: "%.3f", hit.score))
                            .foregroundStyle(.secondary)
                            .monospacedDigit()
                    }
                }
            }
            .padding()
            .navigationTitle("SearchSample")
        }
    }
}
