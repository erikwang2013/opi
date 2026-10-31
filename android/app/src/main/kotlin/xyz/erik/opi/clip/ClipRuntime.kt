// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.clip

import android.content.ClipboardManager
import android.content.Context
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.File

/**
 * 进程内唯一实例：把 [ClipBridge]（内核线程）与 [ClipboardAdapter]（系统剪贴板）
 * 接起来，并把状态投影成 Compose 可观察量（设置页直接读）。
 *
 * 生命周期由调用方决定，本对象不认识开关偏好：
 * · `OpiImeService.onCreate` → 偏好开着就 start（进程内，无需前台服务）；
 * · `ClipSyncService`（connectedDevice 前台服务）→ start / stop，让进程在后台存活；
 * · 设置页开关 → start + 起前台服务；关闭 → stop + 停服务。
 *
 * 安全边界：远端文本只写系统剪贴板，**不自动粘贴**、不自动打开链接；本类与
 * [ClipBridge]/[ClipboardAdapter] 都不写剪贴板内容的日志。
 */
internal object ClipRuntime {
    private const val DISCOVERY_PORT = 45888
    private const val MAX_DEVICE_NAME = 48

    val peers = mutableStateListOf<ClipPeer>()
    var running by mutableStateOf(false)
        private set
    var status by mutableStateOf<ClipStatus?>(null)
        private set

    /** 待用户核对的 SAS；绝不自动接受。 */
    var pairingPrompt by mutableStateOf<ClipPairingPrompt?>(null)
        private set

    /** 上次远端内容的来源设备名（只记来源，不记内容）。 */
    var lastSource by mutableStateOf("")
        private set
    var lastPairing by mutableStateOf("")
        private set
    var lastError by mutableStateOf("")
        private set

    private val main = Handler(Looper.getMainLooper())
    private var bridge: ClipBridge? = null
    private var adapter: ClipboardAdapter? = null
    private var multicastLock: WifiManager.MulticastLock? = null

    @Synchronized
    fun start(context: Context) {
        if (bridge != null) return
        val app = context.applicationContext
        val clipboard = app.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
        if (clipboard == null) {
            lastError = "系统剪贴板不可用"
            return
        }
        val clipAdapter = ClipboardAdapter(clipboard) { text -> bridge?.publishLocalText(text) }
        lateinit var created: ClipBridge
        created = ClipBridge(
            api = OpiClipNative,
            worker = HandlerWorker("opi-clip"),
            callback = MainCallback({ created }, main),
        )
        adapter = clipAdapter
        bridge = created
        multicastLock = acquireMulticastLock(app)
        lastError = ""
        lastSource = ""
        lastPairing = ""
        created.start(
            clipConfigJson(
                dataDir = File(app.filesDir, "clip").absolutePath,
                deviceName = deviceName(),
                discoveryPort = DISCOVERY_PORT,
            ),
        )
        clipAdapter.start()
    }

    @Synchronized
    fun stop(context: Context) {
        adapter?.stop()
        adapter = null
        bridge?.stop()
        bridge = null
        multicastLock?.let { lock -> runCatching { if (lock.isHeld) lock.release() } }
        multicastLock = null
        peers.clear()
        running = false
        status = null
        pairingPrompt = null
        lastSource = ""
        lastError = ""
    }

    fun pairStart(instanceId: String) {
        lastError = ""
        bridge?.pairStart(instanceId)
    }

    /** 用户在 SAS 对话框点了允许/拒绝（或关掉对话框 = 拒绝）。 */
    fun pairConfirm(instanceId: String, accept: Boolean) {
        pairingPrompt = null
        bridge?.pairConfirm(instanceId, accept)
    }

    fun forgetPeer(instanceId: String) {
        bridge?.forgetPeer(instanceId)
    }

    private fun deviceName(): String {
        val raw = "${Build.MANUFACTURER} ${Build.MODEL}".trim()
        return raw.ifEmpty { "Android" }.take(MAX_DEVICE_NAME)
    }

    /** 组播锁：Android 不持锁时通常收不到 UDP 组播（发现报文）。权限是普通权限。 */
    private fun acquireMulticastLock(context: Context): WifiManager.MulticastLock? = runCatching {
        (context.getSystemService(Context.WIFI_SERVICE) as? WifiManager)
            ?.createMulticastLock("opi-clip")
            ?.apply {
                setReferenceCounted(false)
                acquire()
            }
    }.getOrNull()

    /**
     * worker 线程回调 → 主线程。`owner` 校验挡住 stop 之后迟到的状态（否则
     * `running` 会被旧的 status 重新点亮）。
     */
    private class MainCallback(
        private val owner: () -> ClipBridge,
        private val main: Handler,
    ) : ClipBridge.Callback {
        private fun onMain(block: () -> Unit) {
            main.post {
                if (ClipRuntime.bridge === owner()) block()
            }
        }

        override fun onStatus(status: ClipStatus) = onMain {
            ClipRuntime.status = status
            ClipRuntime.running = status.running
        }

        override fun onPeers(peers: List<ClipPeer>) = onMain {
            if (ClipRuntime.peers == peers) return@onMain
            ClipRuntime.peers.clear()
            ClipRuntime.peers.addAll(peers)
        }

        override fun onPairingPrompt(prompt: ClipPairingPrompt) = onMain {
            ClipRuntime.pairingPrompt = prompt
        }

        override fun onPairingFinished(instanceId: String, trusted: Boolean, reason: String?) = onMain {
            if (ClipRuntime.pairingPrompt?.instanceId == instanceId) ClipRuntime.pairingPrompt = null
            val name = peerName(instanceId)
            ClipRuntime.lastPairing = if (trusted) {
                "已与 $name 配对"
            } else {
                "与 $name 的配对未完成" + (reason?.takeIf { it.isNotEmpty() }?.let { "：$it" } ?: "")
            }
        }

        override fun onRemoteText(instanceId: String, text: String) = onMain {
            // 来源提示只显示设备名；内容只写剪贴板（不写日志、不粘贴）。
            ClipRuntime.lastSource = peerName(instanceId)
            ClipRuntime.adapter?.applyRemote(text)
        }

        override fun onError(message: String) = onMain {
            ClipRuntime.lastError = message
        }

        private fun peerName(instanceId: String): String =
            ClipRuntime.peers.firstOrNull { it.instanceId == instanceId }
                ?.name?.takeIf { it.isNotEmpty() }
                ?: instanceId.take(8)
    }
}
