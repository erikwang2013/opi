// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 应用内隐私政策（2026-11-04 用户要求：关于里的隐私政策不跳系统浏览器）。
//! 用 JEditorPane 在模态对话框里加载站点政策页；页内链接继续在同一个面板里打开。

package xyz.erik.opi.candidate

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.Text
import androidx.compose.material.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import androidx.compose.ui.Modifier
import androidx.compose.ui.awt.SwingPanel
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.DialogWindow
import androidx.compose.ui.window.rememberDialogState
import xyz.erik.opi.pet.OpiPetWindowIcon
import java.awt.Color as AwtColor
import java.net.URI
import javax.swing.BorderFactory
import javax.swing.JEditorPane
import javax.swing.JScrollPane
import javax.swing.SwingUtilities
import javax.swing.event.HyperlinkEvent
import javax.swing.text.html.HTMLDocument
import javax.swing.text.html.HTMLEditorKit

internal const val PRIVACY_URL = "https://opi.erik.xyz/privacy/index.html"

@Composable
internal fun PrivacyWindow(onClose: () -> Unit) {
    DialogWindow(
        onCloseRequest = onClose,
        title = "隐私政策",
        state = rememberDialogState(width = 760.dp, height = 600.dp),
        icon = OpiPetWindowIcon,
        resizable = true,
    ) {
        val pane = remember {
            JEditorPane().apply {
                isEditable = false
                contentType = "text/html"
                background = AwtColor.WHITE
                putClientProperty(JEditorPane.HONOR_DISPLAY_PROPERTIES, true)
                addHyperlinkListener { event ->
                    if (event.eventType == HyperlinkEvent.EventType.ACTIVATED) {
                        event.url?.let { runCatching { setPage(it) } }
                    }
                }
                (editorKit as? HTMLEditorKit)?.styleSheet?.apply {
                    addRule("body { font-family: sans-serif; font-size: 12pt; margin: 14px; }")
                    addRule("h1, h2, h3 { margin-top: 14px; margin-bottom: 6px; }")
                    addRule("p, li { margin-top: 5px; margin-bottom: 5px; }")
                }
            }
        }
        // 页面很长（隐私政策全文）；没有滚动容器时 JEditorPane 会直接被裁掉。
        val scroll = remember {
            JScrollPane(pane).apply {
                verticalScrollBarPolicy = JScrollPane.VERTICAL_SCROLLBAR_AS_NEEDED
                horizontalScrollBarPolicy = JScrollPane.HORIZONTAL_SCROLLBAR_NEVER
                border = BorderFactory.createEmptyBorder()
                viewport.background = AwtColor.WHITE
            }
        }
        LaunchedEffect(Unit) {
            val html = withContext(Dispatchers.IO) { httpGet(PRIVACY_URL, 15_000) }
            SwingUtilities.invokeLater {
                if (html == null) {
                    pane.text = "无法加载隐私政策：$PRIVACY_URL"
                    return@invokeLater
                }
                // 只取正文，避开站点导航/页脚；JEditorPane 不认现代 CSS，
                // 用本地样式重排，页面比直接 setPage 干净，也有滚动条。
                val body = Regex("(?is)<main\\b[^>]*>(.*?)</main>")
                    .find(html)?.groupValues?.get(1) ?: html
                pane.text = """
                    <html><head><style>
                    body { font-family: sans-serif; font-size: 12pt; margin: 16px; color: #222; }
                    h1 { font-size: 21pt; margin-bottom: 10px; }
                    h2 { font-size: 15pt; margin-top: 18px; margin-bottom: 6px; }
                    p, li { font-size: 12pt; margin-top: 6px; margin-bottom: 6px; }
                    a { color: #0b6e63; }
                    code { font-family: monospace; }
                    </style></head><body>$body</body></html>
                """.trimIndent()
                (pane.document as? HTMLDocument)?.base =
                    runCatching { URI(PRIVACY_URL).toURL() }.getOrNull()
                pane.caretPosition = 0
            }
        }
        Column(Modifier.fillMaxSize()) {
            SwingPanel(
                background = Color.White,
                factory = { scroll },
                modifier = Modifier.weight(1f).fillMaxWidth(),
            )
            Row(
                Modifier.fillMaxWidth().padding(8.dp),
                horizontalArrangement = Arrangement.End,
            ) {
                TextButton(onClick = onClose) { Text("关闭") }
            }
        }
    }
}
