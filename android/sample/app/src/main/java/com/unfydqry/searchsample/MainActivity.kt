package com.unfydqry.searchsample

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import uniffi.unfydqry.EngineConfig
import uniffi.unfydqry.NormalizeProfile
import uniffi.unfydqry.SearchEngine
import uniffi.unfydqry.SearchStrategy
import uniffi.unfydqry.normalizeLoose

/// Minimal record standing in for the app's "source-of-truth DB" (equivalent to a
/// SwiftData / Room entity).
data class Record(val id: Long, val text: String)

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val dbPath = filesDir.resolve("search_index.sqlite").absolutePath
        // Explicitly select the normalize profile + search strategy.
        // This pair reproduces the default behaviour; swap them to change it.
        val engine = SearchEngine.withConfig(
            dbPath,
            EngineConfig(NormalizeProfile.LOOSE, SearchStrategy.TRIGRAM_BM25),
        )
        val store = seed(engine)
        setContent {
            MaterialTheme {
                Surface(modifier = Modifier.fillMaxSize()) {
                    SearchScreen(engine, store)
                }
            }
        }
    }

    // Same seed as the iOS sample, so the same hit IDs can be eyeballed across both OSes.
    // Returns the id → Record store used to re-fetch records.
    private fun seed(engine: SearchEngine): Map<Long, Record> {
        val docs = listOf(
            Record(1L, "東京タワー"),
            Record(2L, "とうきょうスカイツリー"),
            Record(3L, "ﾄｳｷｮｳ ﾄﾞｰﾑ"),
            Record(4L, "Osaka 城"),
            Record(5L, "がっこう ぐらし"),
            Record(6L, "かっこう の歌"),
            Record(7L, "Ｐｙｔｈｏｎ 入門"),
            Record(8L, "ぱんだ と ﾊﾟﾝﾀﾞ")
        )
        docs.forEach { engine.index(it.id, it.text) }
        return docs.associateBy { it.id }
    }
}

@Composable
fun SearchScreen(engine: SearchEngine, store: Map<Long, Record>) {
    var query by remember { mutableStateOf("") }
    var status by remember { mutableStateOf("indexed ${store.size} docs") }
    val results = remember { mutableStateListOf<Record>() }

    Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
        OutlinedTextField(
            value = query,
            onValueChange = { query = it },
            label = { Text("検索クエリ") },
            modifier = Modifier.fillMaxWidth()
        )
        Spacer(Modifier.height(8.dp))
        Button(onClick = {
            val hits = engine.search(query, 50u)
            // Minimal implementation of design doc §1.3 ("return IDs only / re-fetch
            // from the source-of-truth DB").
            val records = hits.mapNotNull { store[it.id] }
            results.clear()
            results.addAll(records)
            status = "hits: ${records.size}  normalized=\"${normalizeLoose(query)}\""
        }) { Text("検索") }
        Spacer(Modifier.height(8.dp))
        Text(status, style = MaterialTheme.typography.bodySmall)
        Spacer(Modifier.height(8.dp))
        LazyColumn(modifier = Modifier.fillMaxSize()) {
            items(results, key = { it.id }) { record ->
                Column(modifier = Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
                    Text(record.text, style = MaterialTheme.typography.bodyLarge)
                    Text(
                        "id=${record.id}",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
        }
    }
}
