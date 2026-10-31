// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.clip

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import xyz.erik.opi.R
import xyz.erik.opi.settings.ImePreferences
import xyz.erik.opi.settings.SettingsActivity

/**
 * 「跨设备剪贴板」的 connectedDevice 前台服务（2026-11-01）。只做两件事：
 * 让进程在后台活着、把 [ClipRuntime] 拉起来/停下来；网络与剪贴板逻辑都不在这里。
 *
 * 平台限制（如实写）：
 * · Android 14+ 必须声明 `foregroundServiceType="connectedDevice"` 与
 *   `FOREGROUND_SERVICE_CONNECTED_DEVICE`；本应用的组播锁权限
 *   `CHANGE_WIFI_MULTICAST_STATE` 满足该类型的资格要求。
 * · Android 13+ 通知需要 `POST_NOTIFICATIONS` 运行时授权；拒绝时服务仍可运行，
 *   只是通知不可见（设置页会提示）。
 * · **只由前台入口启动**（设置页开关）。不在 IME 的后台路径里起前台服务：
 *   Android 12+ 禁止后台启动 FGS。IME 进程内直接 [ClipRuntime.start] 即可。
 * · 通知文案只有状态，绝不含剪贴板内容。
 */
class ClipSyncService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        running = true
        createChannel()
        startInForeground()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            // 「一键停止」也是设置开关的关闭：不清偏好，下次 IME 进程启动会再次拉起。
            ImePreferences.from(this).clipEnabled = false
            ClipRuntime.stop(this)
            stopSelf()
            return START_NOT_STICKY
        }
        ClipRuntime.start(this)
        return START_STICKY
    }

    override fun onDestroy() {
        running = false
        ClipRuntime.stop(this)
        super.onDestroy()
    }

    private fun startInForeground() {
        val stopIntent = PendingIntent.getService(
            this,
            0,
            Intent(this, ClipSyncService::class.java).setAction(ACTION_STOP),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }
        @Suppress("DEPRECATION")
        val stopAction = Notification.Action.Builder(0, "停止", stopIntent).build()
        val notification = builder
            .setSmallIcon(R.drawable.ic_clip_notification)
            .setContentTitle("跨设备剪贴板")
            .setContentText("局域网剪贴板共享运行中")
            .setOngoing(true)
            .setShowWhen(false)
            .setContentIntent(openSettingsIntent())
            .addAction(stopAction)
            .build()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE,
            )
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }

    private fun openSettingsIntent(): PendingIntent = PendingIntent.getActivity(
        this,
        1,
        Intent(this, SettingsActivity::class.java),
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
    )

    private fun createChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        // minSdk 21：用字符串常量取服务（Class 版要 API 23）。
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as? NotificationManager ?: return
        manager.createNotificationChannel(
            NotificationChannel(
                CHANNEL_ID,
                "跨设备剪贴板",
                NotificationManager.IMPORTANCE_LOW,
            ).apply {
                description = "局域网内已配对设备之间的纯文本剪贴板共享"
                setShowBadge(false)
            },
        )
    }

    companion object {
        private const val CHANNEL_ID = "opi_clip"
        private const val NOTIFICATION_ID = 0x0C11
        private const val ACTION_STOP = "xyz.erik.opi.clip.STOP"

        /** 供 IME 判断「用户是否还有前台服务托底」，决定 onDestroy 要不要停内核。 */
        @Volatile
        var running: Boolean = false
            private set

        /** 必须从前台（设置页）调用。 */
        fun start(context: Context) {
            val intent = Intent(context, ClipSyncService::class.java)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, ClipSyncService::class.java))
        }
    }
}
