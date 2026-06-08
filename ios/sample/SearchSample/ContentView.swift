import SwiftUI
import UnifiedQuery

struct ContentView: View {
    @StateObject private var model = SearchModel()
    // UI-test hook: open the settings sheet on launch when SEARCH_SHOW_SETTINGS is set.
    @State private var showSettings =
        ProcessInfo.processInfo.environment["SEARCH_SHOW_SETTINGS"] != nil

    var body: some View {
        NavigationStack {
            List(model.results) { row in
                VStack(alignment: .leading, spacing: 4) {
                    // Matched fields show the engine's highlighted (normalized)
                    // text; unmatched fields fall back to the raw record text.
                    if let marked = row.highlights[RecordSlot.name.rawValue] {
                        Text(Highlight.attributed(marked)).font(.body)
                    } else {
                        Text(row.record.name).font(.body)
                    }
                    if let marked = row.highlights[RecordSlot.yomi.rawValue] {
                        (Text("よみ: ") + Text(Highlight.attributed(marked)))
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    } else {
                        Text("よみ: \(row.record.yomi)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    HStack(spacing: 8) {
                        Text("id=\(row.record.id)")
                            .monospacedDigit()
                        if !row.matchedSlots.isEmpty {
                            Text("一致: \(row.matchedSlots.map(RecordSlot.label(for:)).joined(separator: ", "))")
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
                    ForEach(OptionToggle.all) { toggle in
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
