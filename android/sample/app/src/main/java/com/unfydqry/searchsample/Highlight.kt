package com.unfydqry.searchsample

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle

/// Bridges the engine's `highlight` markers to a styled [AnnotatedString].
///
/// The host asks the engine to wrap each matching region in [OPEN]/[CLOSE]
/// sentinels; the UI turns that marked text into an [AnnotatedString] whose
/// matched spans stand out in the result list.
object Highlight {
    /// Sentinel markers wrapped around matched regions. C0 control characters,
    /// so they never collide with real (normalized) content.
    const val OPEN = "\u0002" // STX
    const val CLOSE = "\u0003" // ETX

    private val matchStyle = SpanStyle(
        background = Color(0x80FFEB3B), // translucent yellow
        fontWeight = FontWeight.Bold,
    )

    /// Parses text wrapped with [OPEN]/[CLOSE] markers into an [AnnotatedString],
    /// emphasizing the matched spans. An optional [prefix] (e.g. "よみ: ") is
    /// prepended unstyled.
    fun annotated(marked: String, prefix: String = ""): AnnotatedString = buildAnnotatedString {
        if (prefix.isNotEmpty()) append(prefix)
        var run = StringBuilder()
        var inMatch = false

        fun flush() {
            if (run.isEmpty()) return
            if (inMatch) withStyle(matchStyle) { append(run.toString()) } else append(run.toString())
            run = StringBuilder()
        }

        marked.forEach { ch ->
            when (ch) {
                OPEN[0] -> { flush(); inMatch = true }
                CLOSE[0] -> { flush(); inMatch = false }
                else -> run.append(ch)
            }
        }
        flush()
    }
}
