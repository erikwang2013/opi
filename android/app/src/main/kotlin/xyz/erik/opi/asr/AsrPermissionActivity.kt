// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.asr

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.widget.Toast

/**
 * 透明权限 Activity：IME 是 Service，不能直接弹运行时权限。
 * 授权后不自动开录（见 [AsrRuntime.onPermissionResult]），结果回运行时后即关。
 */
class AsrPermissionActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M ||
            checkSelfPermission(Manifest.permission.RECORD_AUDIO) ==
            PackageManager.PERMISSION_GRANTED
        ) {
            finishWith(true)
            return
        }
        requestPermissions(arrayOf(Manifest.permission.RECORD_AUDIO), REQUEST)
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray,
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode != REQUEST) return
        finishWith(grantResults.firstOrNull() == PackageManager.PERMISSION_GRANTED)
    }

    private fun finishWith(granted: Boolean) {
        AsrRuntime.onPermissionResult(granted)
        Toast.makeText(
            this,
            if (granted) "麦克风权限已授予，请再次长按空格" else "未授予麦克风权限，无法语音输入",
            Toast.LENGTH_SHORT,
        ).show()
        finish()
    }

    companion object {
        private const val REQUEST = 41

        fun start(context: Context) {
            context.startActivity(
                Intent(context, AsrPermissionActivity::class.java)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
        }
    }
}
