package com.unfydqry.searchsample

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import uniffi.unfydqry.EngineOptionsConfig
import uniffi.unfydqry.FieldValue
import uniffi.unfydqry.NormalizeOptions
import uniffi.unfydqry.ReindexStatus
import uniffi.unfydqry.SearchEngine
import uniffi.unfydqry.SearchStrategy
import uniffi.unfydqry.normalizeWithOptions
import uniffi.unfydqry.reindexStatusWithOptions

/// Minimal multi-field record standing in for the app's "source-of-truth DB"
/// (equivalent to a SwiftData / Room entity with several searchable columns).
data class Record(val id: Long, val name: String, val yomi: String)

// Field slots for the record-layer API. Stable, never renumbered.
private const val SLOT_NAME = 0
private const val SLOT_YOMI = 1
private const val FIELD_COUNT: UInt = 2u

private fun slotLabel(slot: UByte): String = when (slot.toInt()) {
    SLOT_NAME -> "名前"
    SLOT_YOMI -> "よみ"
    else -> "slot $slot"
}

/// A search result row: the record plus which of its fields matched.
data class ResultRow(val record: Record, val matchedSlots: List<UByte>)

/// The `loose` preset as composable options (lowercase + kana fold).
private fun looseOptions() = NormalizeOptions(lowercase = true, kanaFold = true)

/// One normalization step toggle, bound to a field of [NormalizeOptions].
private data class StepToggle(
    val label: String,
    val get: (NormalizeOptions) -> Boolean,
    val set: (NormalizeOptions, Boolean) -> NormalizeOptions,
)

private val stepToggles = listOf(
    StepToggle("小文字化", { it.lowercase }, { o, v -> o.copy(lowercase = v) }),
    StepToggle("カナ→かな", { it.kanaFold }, { o, v -> o.copy(kanaFold = v) }),
    StepToggle("アクセント除去 (café→cafe)", { it.foldDiacritics }, { o, v -> o.copy(foldDiacritics = v) }),
    StepToggle("長音畳み込み (サーバー→サーバ)", { it.foldChoonpu }, { o, v -> o.copy(foldChoonpu = v) }),
    StepToggle("繰り返し記号展開 (時々→時時)", { it.expandIterationMarks }, { o, v -> o.copy(expandIterationMarks = v) }),
    StepToggle("ハイフン統一", { it.normalizeHyphens }, { o, v -> o.copy(normalizeHyphens = v) }),
    StepToggle("桁区切り除去 (1,000→1000)", { it.stripDigitGrouping }, { o, v -> o.copy(stripDigitGrouping = v) }),
    StepToggle("空白圧縮", { it.collapseWhitespace }, { o, v -> o.copy(collapseWhitespace = v) }),
)

private fun strategyLabel(s: SearchStrategy): String = when (s) {
    SearchStrategy.TRIGRAM_BM25 -> "trigram + bm25"
    SearchStrategy.SUBSTRING -> "substring"
    SearchStrategy.PREFIX -> "prefix"
    SearchStrategy.SUFFIX -> "suffix"
    SearchStrategy.ALL_TERMS -> "all terms"
    SearchStrategy.FUZZY_TRIGRAM -> "fuzzy trigram"
    SearchStrategy.LEVENSHTEIN -> "levenshtein"
    SearchStrategy.DAMERAU_LEVENSHTEIN -> "damerau-levenshtein"
}

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val dbPath = filesDir.resolve("search_index.sqlite").absolutePath
        val engine = SearchEngine.withOptionsRebuilding(
            dbPath,
            EngineOptionsConfig(looseOptions(), SearchStrategy.TRIGRAM_BM25),
        )
        val store = seed(engine)
        setContent {
            MaterialTheme {
                Surface(modifier = Modifier.fillMaxSize()) {
                    SearchScreen(engine, store, dbPath)
                }
            }
        }
    }

    // Same seed as the iOS sample, so the same hit IDs can be eyeballed across both OSes.
    // Returns the id → Record store used to re-fetch records.
    private fun seed(engine: SearchEngine): Map<Long, Record> {
        // Multi-field records (name + reading). The same seed is used across the
        // iOS, Android, and Flutter samples so hits can be compared by id.
        val docs = listOf(
            Record(1L, "東京タワー", "とうきょうたわー"),
            Record(2L, "スカイツリー", "すかいつりー"),
            Record(3L, "大阪城", "おおさかじょう"),
            Record(4L, "名古屋テレビ塔", "なごやてれびとう"),
            Record(5L, "札幌時計台", "さっぽろとけいだい"),
            Record(6L, "コーヒーサーバー", "こーひーさーばー"),
            Record(7L, "データベース", "でーたべーす"),
            Record(8L, "プリンター", "ぷりんたー"),
        )
        docs.forEach { r ->
            // The engine packs (id, slot) internally; we pass our record id and a
            // slot per field, and get record ids back from searchRecords.
            engine.indexRecord(
                recordId = r.id,
                fields = listOf(
                    FieldValue(slot = SLOT_NAME.toUByte(), text = r.name),
                    FieldValue(slot = SLOT_YOMI.toUByte(), text = r.yomi),
                ),
            )
        }
        return docs.associateBy { it.id }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SearchScreen(initialEngine: SearchEngine, store: Map<Long, Record>, dbPath: String) {
    var engine by remember { mutableStateOf(initialEngine) }
    var query by remember { mutableStateOf("") }
    // `options` is the pending selection the toggles reflect; `applied` is what
    // the engine/index are built with. Changing options only flags whether a
    // reindex is needed (detected via reindexStatus) — it does not rebuild.
    var options by remember { mutableStateOf(looseOptions()) }
    var applied by remember { mutableStateOf(looseOptions()) }
    var strategy by remember { mutableStateOf(SearchStrategy.TRIGRAM_BM25) }
    var needsReindex by remember { mutableStateOf(false) }
    var status by remember { mutableStateOf("") }
    var showSettings by remember { mutableStateOf(false) }
    val allDocs = remember(store) {
        store.values.sortedBy { it.id }.map { ResultRow(it, emptyList()) }
    }
    // Prefilled so the initial empty query shows every record without a flash.
    val results = remember { mutableStateListOf<ResultRow>().apply { addAll(allDocs) } }

    fun runSearch() {
        if (query.isBlank()) {
            results.clear()
            results.addAll(allDocs)
            status = "全件表示 (${results.size})"
            return
        }
        // Record-layer search: hits collapse to one row per record, with the
        // matched field slots. The host re-fetches records by id from `store`.
        val hits = engine.searchRecords(query, 50u, FIELD_COUNT)
        val rows = hits.mapNotNull { h -> store[h.recordId]?.let { ResultRow(it, h.matchedSlots) } }
        results.clear()
        results.addAll(rows)
        // Results reflect the *applied* normalization until a reindex.
        status = "hits: ${rows.size}  normalized=\"${normalizeWithOptions(query, applied)}\""
    }

    // Toggling a step only detects whether a reindex is needed; it does not rebuild.
    fun setOptions(newOptions: NormalizeOptions) {
        options = newOptions
        needsReindex = reindexStatusWithOptions(dbPath, newOptions) == ReindexStatus.CONFIG_CHANGED
    }

    // Strategy isn't part of the index fingerprint, so apply it immediately by
    // reopening with the applied options and the new strategy (no reindex).
    fun applyStrategy(newStrategy: SearchStrategy) {
        strategy = newStrategy
        val old = engine
        engine = SearchEngine.withOptions(dbPath, EngineOptionsConfig(applied, newStrategy))
        old.close()
        runSearch()
    }

    // Apply the pending options by regenerating the index in place, then clear the flag.
    fun doReindex() {
        val old = engine
        engine = SearchEngine.withOptionsRebuilding(dbPath, EngineOptionsConfig(options, strategy))
        old.close()
        applied = options
        needsReindex = false
        status = "インデックスを再生成しました"
        runSearch()
    }

    // Incremental search: debounce keystrokes so a search runs shortly after typing
    // settles rather than on every character.
    LaunchedEffect(query) {
        delay(150)
        runSearch()
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("SearchSample") },
                actions = { TextButton(onClick = { showSettings = true }) { Text("設定") } },
            )
        },
    ) { padding ->
        Column(modifier = Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp)) {
            if (needsReindex) {
                Surface(
                    color = MaterialTheme.colorScheme.tertiaryContainer,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Row(
                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            "正規化設定が変更されました。再生成が必要です。",
                            style = MaterialTheme.typography.bodySmall,
                            modifier = Modifier.weight(1f),
                        )
                        TextButton(onClick = { doReindex() }) { Text("再生成") }
                    }
                }
                Spacer(Modifier.height(8.dp))
            }
            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                label = { Text("検索 (全角/半角/カナ/ひら、なんでも)") },
                singleLine = true,
                trailingIcon = {
                    if (query.isNotEmpty()) {
                        TextButton(onClick = { query = "" }) { Text("✕") }
                    }
                },
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(Modifier.height(4.dp))
            Text(status, style = MaterialTheme.typography.bodySmall)
            Spacer(Modifier.height(8.dp))
            LazyColumn(modifier = Modifier.fillMaxSize()) {
                items(results, key = { it.record.id }) { row ->
                    Column(modifier = Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
                        Text(row.record.name, style = MaterialTheme.typography.bodyLarge)
                        Text(
                            "よみ: ${row.record.yomi}",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        val matched = if (row.matchedSlots.isEmpty()) {
                            ""
                        } else {
                            "  一致: ${row.matchedSlots.joinToString(", ") { slotLabel(it) }}"
                        }
                        Text(
                            "id=${row.record.id}$matched",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
        }
    }

    if (showSettings) {
        val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
        ModalBottomSheet(onDismissRequest = { showSettings = false }, sheetState = sheetState) {
            SettingsSheet(
                options = options,
                strategy = strategy,
                needsReindex = needsReindex,
                onToggle = { newOptions -> setOptions(newOptions) },
                onStrategy = { newStrategy -> applyStrategy(newStrategy) },
                onReindex = { doReindex() },
            )
        }
    }
}

@Composable
private fun SettingsSheet(
    options: NormalizeOptions,
    strategy: SearchStrategy,
    needsReindex: Boolean,
    onToggle: (NormalizeOptions) -> Unit,
    onStrategy: (SearchStrategy) -> Unit,
    onReindex: () -> Unit,
) {
    Column(modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp).padding(bottom = 24.dp)) {
        Text("正規化ステップ", style = MaterialTheme.typography.titleSmall)
        Spacer(Modifier.height(8.dp))
        stepToggles.forEach { step ->
            Row(
                modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Switch(
                    checked = step.get(options),
                    onCheckedChange = { v -> onToggle(step.set(options, v)) },
                )
                Spacer(Modifier.width(12.dp))
                Text(step.label, style = MaterialTheme.typography.bodyMedium)
            }
        }

        Spacer(Modifier.height(8.dp))
        HorizontalDivider()
        Spacer(Modifier.height(8.dp))

        Text("検索アルゴリズム", style = MaterialTheme.typography.titleSmall)
        Spacer(Modifier.height(4.dp))
        var expanded by remember { mutableStateOf(false) }
        Button(onClick = { expanded = true }) { Text(strategyLabel(strategy)) }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            SearchStrategy.values().forEach { s ->
                DropdownMenuItem(
                    text = { Text(strategyLabel(s)) },
                    onClick = { expanded = false; onStrategy(s) },
                )
            }
        }

        Spacer(Modifier.height(16.dp))
        if (needsReindex) {
            Button(onClick = onReindex, modifier = Modifier.fillMaxWidth()) {
                Text("インデックス再生成 (必要)")
            }
        } else {
            OutlinedButton(onClick = onReindex, modifier = Modifier.fillMaxWidth()) {
                Text("インデックス再生成")
            }
        }
        Text(
            if (needsReindex) "正規化設定が変更されています。再生成すると現在の設定が反映されます。"
            else "保存済みの生テキストから現在の設定で再生成します。",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 4.dp),
        )
    }
}
