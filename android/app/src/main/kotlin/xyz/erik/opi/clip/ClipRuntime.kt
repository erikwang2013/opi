// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.clip

import android.content.ClipboardManager
import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Environment
import android.os.Handler
import android.os.Looper
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.File
import java.net.Inet4Address

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
    private const val STAGING_DIR = "transfer"
    /** 入站暂存：内核先写这里，完成后再导入公共 Download/opi（见 [importIncomingToPublicDownloads]）。 */
    private const val INCOMING_DIR = "transfer-incoming"
    private const val MAX_TRANSFERS = 20

    val peers = mutableStateListOf<ClipPeer>()
    /** 传输面板的收发记录（只保留最近 20 条；终态不自动消失，便于看「打开文件夹」）。 */
    val transfers = mutableStateListOf<ClipTransfer>()
    var running by mutableStateOf(false)
        private set
    var status by mutableStateOf<ClipStatus?>(null)
        private set
    /** 传输面板里的一次性反馈（复制成功/失败原因），不记录内容。 */
    var transferMessage by mutableStateOf("")
        internal set

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
    private var appContext: Context? = null
    private var bridge: ClipBridge? = null
    private var adapter: ClipboardAdapter? = null
    private var multicastLock: WifiManager.MulticastLock? = null
    /** 发送侧暂存目录：transferId → cacheDir/transfer/<uuid>；终态事件后删除。主线程访问。 */
    private val staged = mutableMapOf<String, File>()

    @Synchronized
    fun start(context: Context) {
        if (bridge != null) return
        val app = context.applicationContext
        val clipboard = app.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
        if (clipboard == null) {
            lastError = "系统剪贴板不可用"
            return
        }
        // 只对已配对设备自动发送：未配对设备既不产生会话，内核 publish_text 也只发给
        // trusted 会话；这里再挡一层，避免「只发现到未配对设备」时白跑一次发布。
        val clipAdapter = ClipboardAdapter(clipboard) { text ->
            if (hasTrustedTarget(peers)) bridge?.publishLocalText(text)
        }
        lateinit var created: ClipBridge
        created = ClipBridge(
            api = OpiClipNative,
            worker = HandlerWorker("opi-clip"),
            callback = MainCallback({ created }, main),
        )
        adapter = clipAdapter
        bridge = created
        appContext = app
        multicastLock = acquireMulticastLock(app)
        pruneStaleStaging(File(app.cacheDir, STAGING_DIR))
        pruneStaleStaging(File(app.getExternalFilesDir(null) ?: app.filesDir, INCOMING_DIR))
        migrateLegacyDownloads(app)
        lastError = ""
        lastSource = ""
        lastPairing = ""
        val lan = localAddress(app)
        created.start(
            clipConfigJson(
                dataDir = File(app.filesDir, "clip").absolutePath,
                deviceName = deviceName(),
                discoveryPort = DISCOVERY_PORT,
                advertiseIp = lan?.ip,
                advertisePrefix = lan?.prefix,
            ),
        )
        ClipRouting.bind { send, receive -> created.setTextRoutes(send, receive) }
        clipAdapter.start()
    }

    @Synchronized
    fun stop(context: Context) {
        ClipRouting.bind(null)
        adapter?.stop()
        adapter = null
        bridge?.stop()
        bridge = null
        appContext = null
        multicastLock?.let { lock -> runCatching { if (lock.isHeld) lock.release() } }
        multicastLock = null
        peers.clear()
        transfers.clear()
        staged.values.forEach { runCatching { it.deleteRecursively() } }
        staged.clear()
        running = false
        status = null
        pairingPrompt = null
        lastSource = ""
        lastError = ""
        transferMessage = ""
    }

    fun pairStart(instanceId: String) {
        lastError = ""
        bridge?.pairStart(instanceId)
    }

    /** 手动连接：向对方 IP 的发现端口发探测；对端会单播回复并出现在 peers。 */
    fun probe(ip: String) {
        lastError = ""
        bridge?.probe(ip.trim())
    }

    /** 用户在 SAS 对话框点了允许/拒绝（或关掉对话框 = 拒绝）。 */
    fun pairConfirm(instanceId: String, accept: Boolean) {
        pairingPrompt = null
        bridge?.pairConfirm(instanceId, accept)
    }

    fun forgetPeer(instanceId: String) {
        bridge?.forgetPeer(instanceId)
    }

    /** 手动发送当前剪贴板（后台读剪贴板可能被系统拒绝，这里由设置页前台触发）。 */
    fun publishCurrentClipboard(context: Context): String {
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
            ?: return "系统剪贴板不可用"
        val text = try {
            clipboard.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.text?.toString()
        } catch (_: SecurityException) {
            null
        }
        if (text.isNullOrEmpty()) return "剪贴板里没有文本"
        if (text.length > 256 * 1024) return "文本超过 256 KiB 上限"
        bridge?.publishLocalText(text) ?: return "剪贴板内核未启动"
        return "已发送 ${text.length} 字（内容不显示、不记录）"
    }

    /** 传输面板「文本」：与系统剪贴板同一条 publish_text（内核只发给 trusted 会话）。 */
    fun publishText(text: String): String {
        if (text.isEmpty()) return "文本为空"
        if (text.length > 256 * 1024) return "文本超过 256 KiB 上限"
        bridge?.publishLocalText(text) ?: return "传输内核未启动"
        return "已发送 ${text.length} 字（内容不显示、不记录）"
    }

    /** 发送侧 staging 目录（cacheDir/transfer/<uuid>），调用方负责复制完再交给内核。 */
    fun newStageDir(context: Context): File {
        val root = File(context.cacheDir, STAGING_DIR)
        if (!root.isDirectory && !root.mkdirs()) {
            throw IllegalStateException("无法创建传输缓存目录")
        }
        return File(root, java.util.UUID.randomUUID().toString()).apply { mkdirs() }
    }

    /** 文件已复制进 [stage]：发送清单；失败或终态事件后删除 staging。 */
    fun sendStagedFiles(peerId: String, filesJson: String, stage: File) {
        transferMessage = ""
        val current = bridge
        if (current == null) {
            stage.deleteRecursively()
            transferMessage = "传输内核未启动"
            return
        }
        current.sendFiles(peerId, filesJson) { id ->
            main.post {
                if (id == null) {
                    stage.deleteRecursively()
                    transferMessage = "发送失败"
                } else {
                    staged[id] = stage
                }
            }
        }
    }

    /**
     * 2026-11-04：旧版把手收文件放在应用专属 `Download/opi`。升级后一次性搬到公共
     * Download/opi 并删掉旧目录；部分失败时保留旧目录（文件不丢）。
     */
    private fun migrateLegacyDownloads(context: Context) {
        val legacy = context.getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS)
            ?.resolve("opi") ?: return
        if (!legacy.isDirectory) return
        Thread({
            android.os.Process.setThreadPriority(android.os.Process.THREAD_PRIORITY_BACKGROUND)
            runCatching { importIncomingToPublicDownloads(context, legacy) }
        }, "opi-transfer-migrate").start()
    }

    /**
     * 入站暂存目录（应用专属外部存储，免权限）。内核把文件写完后，
     * [importIncomingToPublicDownloads] 再搬进公共 Download/opi —— Android 10+ 不能直接给内核
     * 一个可写的公共路径，必须走 MediaStore，所以采用「暂存 → 导入」。
     */
    fun incomingSaveDir(context: Context): String? {
        val base = context.getExternalFilesDir(null) ?: context.filesDir
        val dir = File(base, "$INCOMING_DIR/${java.util.UUID.randomUUID()}")
        if (!dir.isDirectory && !dir.mkdirs()) return null
        return dir.absolutePath
    }

    fun respondTransfer(context: Context, transferId: String, accept: Boolean) {
        transferMessage = ""
        val current = bridge
        if (current == null) {
            transferMessage = "传输内核未启动"
            return
        }
        val dir = if (accept) incomingSaveDir(context) else ""
        if (dir == null) {
            transferMessage = "无法创建接收目录"
            return
        }
        current.respondTransfer(transferId, accept, dir)
        // 内核的接收答复没有同步返回值；先乐观切状态，后续 progress/终态事件会覆盖。
        val index = transfers.indexOfFirst { it.id == transferId }
        if (index >= 0 && transfers[index].phase == TransferPhase.OFFERED) {
            transfers[index] = transfers[index].copy(
                phase = if (accept) TransferPhase.ACTIVE else TransferPhase.CANCELED,
                savedDir = if (accept) dir else null,
            )
        }
    }

    fun cancelTransfer(transferId: String) {
        bridge?.cancelTransfer(transferId)
    }

    private fun pruneStaleStaging(root: File) {
        val cutoff = System.currentTimeMillis() - 24L * 60 * 60 * 1000
        root.listFiles()?.forEach { dir ->
            if (dir.lastModified() < cutoff) runCatching { dir.deleteRecursively() }
        }
    }

    private fun deviceName(): String {
        val raw = "${Build.MANUFACTURER} ${Build.MODEL}".trim()
        return raw.ifEmpty { "Android" }.take(MAX_DEVICE_NAME)
    }

    /**
     * 本机 Wi-Fi/以太网 IPv4。默认出口在「Wi-Fi 无外网 + 蜂窝/VPN 在线」时可能是
     * 蜂窝或 VPN，拿它推 /24 会广播到错误网段；这里优先 Wi-Fi 接口的站点本地地址。
     */
    internal data class LanAddr(val ip: String, val prefix: Int)

    /** 显示用：只取地址。 */
    internal fun localIpv4(context: Context): String? = localAddress(context)?.ip

    internal fun localAddress(context: Context): LanAddr? {
        val cm = context.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager
            ?: return null
        fun links(network: android.net.Network): List<Pair<Inet4Address, Int>> =
            cm.getLinkProperties(network)?.linkAddresses.orEmpty().mapNotNull { link ->
                (link.address as? Inet4Address)
                    ?.takeIf { !it.isLoopbackAddress && !it.isLinkLocalAddress }
                    ?.let { it to link.prefixLength }
            }

        // VPN/蜂窝不参与局域网广播：开着 VPN 时「默认网络」往往不是 Wi-Fi。
        val lanNets = cm.allNetworks.filter { network ->
            val caps = cm.getNetworkCapabilities(network) ?: return@filter false
            !caps.hasTransport(NetworkCapabilities.TRANSPORT_VPN)
        }
        val wifi = lanNets.firstOrNull { network ->
            cm.getNetworkCapabilities(network)
                ?.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) == true
        }
        val ethernet = lanNets.firstOrNull { network ->
            cm.getNetworkCapabilities(network)
                ?.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) == true
        }
        val preferred = (wifi ?: ethernet)?.let { network ->
            links(network).firstOrNull { p -> p.first.isSiteLocalAddress }
        }
        val anyLocal = lanNets.asSequence()
            .flatMap { links(it).asSequence() }
            .firstOrNull { p -> p.first.isSiteLocalAddress }
        return (preferred ?: anyLocal)?.let { LanAddr(it.first.hostAddress ?: return null, it.second) }
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

        override fun onTransfer(event: ClipEvent) = onMain {
            when (event) {
                is ClipEvent.TransferOffer -> upsert(
                    ClipTransfer(
                        id = event.transferId,
                        instanceId = event.instanceId,
                        peerName = peerName(event.instanceId),
                        outgoing = event.outgoing,
                        files = event.files,
                        totalBytes = event.totalBytes,
                        fileCount = event.files.size,
                        phase = if (event.outgoing) TransferPhase.ACTIVE else TransferPhase.OFFERED,
                    ),
                )
                is ClipEvent.TransferProgress -> update(event.transferId) {
                    it.copy(
                        transferredBytes = event.transferredBytes,
                        totalBytes = event.totalBytes,
                        fileIndex = event.fileIndex,
                        fileCount = event.fileCount,
                        phase = TransferPhase.ACTIVE,
                    )
                }
                is ClipEvent.TransferCompleted -> {
                    cleanupStage(event.transferId)
                    update(event.transferId) {
                        it.copy(
                            phase = TransferPhase.COMPLETED,
                            transferredBytes = event.totalBytes,
                            totalBytes = event.totalBytes,
                            savedDir = event.savedDir,
                            files = event.files,
                        )
                    }
                    // 入站：内核写的是应用专属暂存目录，后台导入公共 Download 后再更新落点。
                    if (!event.outgoing) {
                        val ctx = ClipRuntime.appContext
                        val source = event.savedDir?.let(::File)
                        if (ctx != null && source != null) {
                            Thread({
                                android.os.Process.setThreadPriority(
                                    android.os.Process.THREAD_PRIORITY_BACKGROUND,
                                )
                                val public = runCatching {
                                    importIncomingToPublicDownloads(ctx, source)
                                }.getOrNull()
                                if (public != null) {
                                    main.post {
                                        update(event.transferId) { it.copy(savedDir = public) }
                                    }
                                }
                            }, "opi-transfer-import").start()
                        }
                    }
                }
                is ClipEvent.TransferFailed -> {
                    cleanupStage(event.transferId)
                    update(event.transferId) {
                        it.copy(phase = TransferPhase.FAILED, reason = event.reason)
                    }
                }
                is ClipEvent.TransferCanceled -> {
                    cleanupStage(event.transferId)
                    update(event.transferId) { it.copy(phase = TransferPhase.CANCELED) }
                }
                else -> Unit
            }
        }

        override fun onError(message: String) = onMain {
            ClipRuntime.lastError = message
        }

        private fun upsert(transfer: ClipTransfer) {
            val index = ClipRuntime.transfers.indexOfFirst { it.id == transfer.id }
            if (index >= 0) {
                ClipRuntime.transfers[index] = transfer
                return
            }
            if (ClipRuntime.transfers.size >= MAX_TRANSFERS) {
                val terminal = ClipRuntime.transfers.indexOfFirst {
                    it.phase == TransferPhase.COMPLETED ||
                        it.phase == TransferPhase.FAILED ||
                        it.phase == TransferPhase.CANCELED
                }
                ClipRuntime.transfers.removeAt(if (terminal >= 0) terminal else 0)
            }
            ClipRuntime.transfers.add(transfer)
        }

        private fun update(transferId: String, block: (ClipTransfer) -> ClipTransfer) {
            val index = ClipRuntime.transfers.indexOfFirst { it.id == transferId }
            if (index >= 0) ClipRuntime.transfers[index] = block(ClipRuntime.transfers[index])
        }

        private fun cleanupStage(transferId: String) {
            ClipRuntime.staged.remove(transferId)?.let { dir ->
                runCatching { dir.deleteRecursively() }
            }
        }

        private fun peerName(instanceId: String): String =
            ClipRuntime.peers.firstOrNull { it.instanceId == instanceId }
                ?.name?.takeIf { it.isNotEmpty() }
                ?: instanceId.take(8)
    }
}
