package com.unimose.searchsample

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
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
import uniffi.search_core.Hit
import uniffi.search_core.SearchEngine
import uniffi.search_core.normalizeLoose

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val dbPath = filesDir.resolve("search_index.sqlite").absolutePath
        val engine = SearchEngine(dbPath)
        seed(engine)
        setContent {
            MaterialTheme {
                Surface(modifier = Modifier.fillMaxSize()) {
                    SearchScreen(engine)
                }
            }
        }
    }

    private fun seed(engine: SearchEngine) {
        // iOS サンプルと同じシード(両OSで同じヒットIDが返ることを目で確認するため)
        val docs = listOf(
            1L to "東京タワー",
            2L to "とうきょうスカイツリー",
            3L to "ﾄｳｷｮｳ ﾄﾞｰﾑ",
            4L to "Osaka 城",
            5L to "がっこう ぐらし",
            6L to "かっこう の歌",
            7L to "Ｐｙｔｈｏｎ 入門",
            8L to "ぱんだ と ﾊﾟﾝﾀﾞ"
        )
        docs.forEach { (id, text) -> engine.index(id, text) }
    }
}

@Composable
fun SearchScreen(engine: SearchEngine) {
    var query by remember { mutableStateOf("") }
    var status by remember { mutableStateOf("indexed 8 docs") }
    val results = remember { mutableStateListOf<Hit>() }

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
            results.clear()
            results.addAll(hits)
            status = "hits: ${hits.size}  normalized=\"${normalizeLoose(query)}\""
        }) { Text("検索") }
        Spacer(Modifier.height(8.dp))
        Text(status, style = MaterialTheme.typography.bodySmall)
        Spacer(Modifier.height(8.dp))
        LazyColumn(modifier = Modifier.fillMaxSize()) {
            items(results, key = { it.id }) { hit ->
                Row(modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                    horizontalArrangement = Arrangement.SpaceBetween) {
                    Text("id=${hit.id}")
                    Text("%.3f".format(hit.score))
                }
            }
        }
    }
}
