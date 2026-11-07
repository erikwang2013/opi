// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 设置窗里需要输入中文的文本框。Compose Desktop 的 BasicTextField 在本机
//! fcitx5/XIM 下收不到组字（只落裸字母），但 Swing 的 JTextField 正常走 XIM。
//! 这里用 SwingPanel 包一个无边框 JTextField，外观由 Compose 的边框/提示负责。
//!
//! 只用于可聚焦窗口（设置窗）的少量文本输入；符号搜索框仍用 Compose BasicTextField
//!（它搜的是拼音/英文原文，不需要组字），候选窗等自绘界面不走这里。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.awt.SwingPanel
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.awt.Font
import java.awt.Window
import java.awt.event.MouseAdapter
import java.awt.event.MouseEvent
import java.awt.event.FocusAdapter
import java.awt.event.FocusEvent
import java.awt.event.WindowAdapter
import java.awt.event.WindowEvent
import javax.swing.JTextField
import javax.swing.SwingUtilities
import javax.swing.border.EmptyBorder
import javax.swing.event.DocumentEvent
import javax.swing.event.DocumentListener

/**
 * 无边框 Swing 单行输入（IME 走 AWT 的原生焦点与 XIM，Compose BasicTextField 收不到组字）。
 * 外观由调用方画；这里只负责文本同步。
 */
@Composable
internal fun ImeSwingPanel(
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    height: Dp = 34.dp,
    fontSize: Int = 14,
    foreground: Color = Color(0x22, 0x27, 0x2E),
    caretColor: Color = Color(0x2E, 0x8B, 0x80),
    background: Color = Color.White,
    enabled: Boolean = true,
    onFocusChanged: ((Boolean) -> Unit)? = null,
) {
    val suppress = remember { booleanArrayOf(false) }
    val field = remember(enabled, fontSize, foreground, caretColor) {
        JTextField().apply {
            isOpaque = false
            isEnabled = enabled
            border = EmptyBorder(0, 0, 0, 0)
            font = Font("SansSerif", Font.PLAIN, fontSize)
            this.foreground = java.awt.Color(foreground.toArgb(), true)
            this.caretColor = java.awt.Color(caretColor.toArgb(), true)
        }
    }
    // 监听器只注册一次；最新的 lambda 经 State 读取，避免 Effect 反复摘挂。
    val change by rememberUpdatedState(onValueChange)
    DisposableEffect(field) {
        val listener = object : DocumentListener {
            private fun changed() {
                if (!suppress[0]) change(field.text)
            }
            override fun insertUpdate(e: DocumentEvent) = changed()
            override fun removeUpdate(e: DocumentEvent) = changed()
            override fun changedUpdate(e: DocumentEvent) = changed()
        }
        val focusListener = object : FocusAdapter() {
            override fun focusGained(e: FocusEvent) {
                onFocusChanged?.invoke(true)
            }
            override fun focusLost(e: FocusEvent) {
                onFocusChanged?.invoke(false)
            }
        }
        // 符号窗默认 focusable=false：点击先让窗可聚焦，等窗口真拿到焦点后再把
        // 焦点交给输入框（探针实证：提前 requestFocusInWindow 会失败）。
        val mouseListener = object : MouseAdapter() {
            override fun mousePressed(e: MouseEvent) {
                onFocusChanged?.invoke(true)
                val w: Window = SwingUtilities.getWindowAncestor(field) ?: return
                if (!w.focusableWindowState) {
                    XFocus.remember()
                    w.addWindowFocusListener(object : WindowAdapter() {
                        override fun windowGainedFocus(ev: WindowEvent) {
                            w.removeWindowFocusListener(this)
                            field.requestFocusInWindow()
                        }
                    })
                    w.setFocusableWindowState(true)
                    w.toFront()
                    w.requestFocus()
                }
            }
        }
        field.document.addDocumentListener(listener)
        field.addFocusListener(focusListener)
        field.addMouseListener(mouseListener)
        onDispose {
            field.document.removeDocumentListener(listener)
            field.removeFocusListener(focusListener)
            field.removeMouseListener(mouseListener)
        }
    }
    SwingPanel(
        background = background,
        factory = { field },
        modifier = modifier.height(height),
        update = { f ->
            val bg = java.awt.Color(background.toArgb(), true)
            if (f.background != bg) {
                f.background = bg
                f.isOpaque = true
                (f.parent as? javax.swing.JComponent)?.let {
                    it.background = bg
                    it.isOpaque = true
                }
            }
            if (f.text != value) {
                suppress[0] = true
                f.text = value
                suppress[0] = false
            }
        },
    )
}

@Composable
internal fun ImeTextField(
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    label: String? = null,
    hint: String = "",
    enabled: Boolean = true,
) {
    Column(modifier) {
        if (label != null) {
            Text(label, fontSize = 11.sp, color = Muted)
        }
        Box(
            modifier = Modifier.fillMaxWidth().height(36.dp)
                .background(Paper)
                .border(1.dp, Hairline)
                .padding(horizontal = 8.dp),
            contentAlignment = Alignment.CenterStart,
        ) {
            if (value.isEmpty() && hint.isNotEmpty()) {
                Text(hint, fontSize = 12.sp, color = Muted)
            }
            ImeSwingPanel(
                value = value,
                onValueChange = onValueChange,
                modifier = Modifier.fillMaxWidth(),
                enabled = enabled,
            )
        }
    }
}
