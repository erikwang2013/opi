// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import android.app.Activity
import android.graphics.Color
import android.os.Bundle
import android.util.TypedValue
import android.view.ViewGroup
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.LinearLayout
import android.widget.TextView

/**
 * 应用内隐私政策（2026-11-04 用户要求：关于里的隐私政策不要跳浏览器）。
 *
 * 用 WebView 在本 App 内打开站点政策页；页内 http/https 链接继续留在 WebView，
 * 不再交给系统浏览器。IME 是 Service，启动时必须带 NEW_TASK。
 */
class PrivacyActivity : Activity() {
    private lateinit var web: WebView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        web = WebView(this).apply {
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            webViewClient = object : WebViewClient() {
                @Suppress("DEPRECATION")
                override fun shouldOverrideUrlLoading(view: WebView, url: String): Boolean {
                    if (url.startsWith("http://") || url.startsWith("https://")) {
                        view.loadUrl(url)
                    }
                    // 非 http(s)（mailto 等）也不外跳：隐私页只需要留在应用内。
                    return true
                }
            }
            loadUrl(ABOUT_PRIVACY_URL)
        }
        val bar = TextView(this).apply {
            text = "‹  隐私政策"
            setTextColor(Color.WHITE)
            setBackgroundColor(0xFF2E8B80.toInt())
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 17f)
            setPadding(dp(16), dp(12), dp(16), dp(12))
            setOnClickListener { goBackOrFinish() }
        }
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(Color.WHITE)
            addView(
                bar,
                LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                ),
            )
            addView(web, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
        }
        setContentView(root)
    }

    private fun dp(value: Int): Int = (value * resources.displayMetrics.density).toInt()

    private fun goBackOrFinish() {
        if (this::web.isInitialized && web.canGoBack()) web.goBack() else finish()
    }

    @Deprecated("Deprecated in API 33; this Activity still supports the platform back key.")
    override fun onBackPressed() {
        goBackOrFinish()
    }
}
