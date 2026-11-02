// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.clip

import android.app.Activity
import android.app.AlertDialog
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import android.widget.EditText
import android.widget.Toast
import java.io.File

/**
 * 透明传输助手 Activity（2026-11-02）：IME 是 Service，拿不到 `onActivityResult`，
 * 所以文件/媒体/文件夹选择与文本输入都放这里，结果直接交给 [ClipRuntime]。
 *
 * 文件先复制到 `cacheDir/transfer/<uuid>`（保留显示名与 MIME），再调 `clipSendFiles`；
 * 终态事件由 [ClipRuntime] 删除暂存目录。数量/大小/路径上限在复制前先判一次，
 * 复制过程中再按实际字节数兜底 —— 元数据缺失或撒谎时不至于写爆磁盘。
 */
class TransferActivity : Activity() {

    private var mode = MODE_FILES
    private var peerId = ""

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        mode = intent.getStringExtra(EXTRA_MODE) ?: MODE_FILES
        peerId = intent.getStringExtra(EXTRA_PEER).orEmpty()
        when (mode) {
            MODE_TEXT -> showTextDialog()
            MODE_MEDIA -> launchPicker(
                Intent(Intent.ACTION_GET_CONTENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE)
                    type = "*/*"
                    putExtra(Intent.EXTRA_MIME_TYPES, arrayOf("image/*", "video/*"))
                    putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
                },
            )
            MODE_FOLDER -> launchPicker(
                Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
                    addFlags(
                        Intent.FLAG_GRANT_READ_URI_PERMISSION or
                            Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION,
                    )
                },
            )
            else -> launchPicker(
                Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE)
                    type = "*/*"
                    putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
                },
            )
        }
    }

    private fun launchPicker(intent: Intent) {
        try {
            startActivityForResult(intent, REQUEST_PICK)
        } catch (t: Throwable) {
            toastAndFinish("无法打开系统选择器：${t.message ?: "未知错误"}")
        }
    }

    @Deprecated("IME 宿主用 transparent Activity 承接 SAF 结果，无需 Activity Result API")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode != REQUEST_PICK) return
        if (resultCode != RESULT_OK || data == null) {
            finish()
            return
        }
        val uris = ArrayList<Uri>()
        data.clipData?.let { clip ->
            for (i in 0 until clip.itemCount) clip.getItemAt(i)?.uri?.let { uris.add(it) }
        }
        if (uris.isEmpty()) data.data?.let { uris.add(it) }
        if (uris.isEmpty()) {
            finish()
            return
        }
        Thread({ process(uris) }, "opi-transfer-copy").start()
    }

    private fun process(uris: List<Uri>) {
        try {
            if (peerId.isEmpty()) throw IllegalStateException("没有已配对且已连接的设备")
            if (mode == MODE_FOLDER) sendFolder(uris.first()) else sendFiles(uris)
        } catch (t: Throwable) {
            runOnUiThread {
                Toast.makeText(
                    this,
                    "无法发送：${t.message ?: t.javaClass.simpleName}",
                    Toast.LENGTH_LONG,
                ).show()
                finish()
            }
        }
    }

    // ---------- 文件/媒体：逐个复制到 staging ----------

    private class Picked(
        val uri: Uri,
        val name: String,
        val mime: String,
        val size: Long,
    )

    private fun sendFiles(uris: List<Uri>) {
        val used = HashSet<String>()
        val picked = uris.map { uri ->
            val name = uniqueTransferName(safeTransferName(displayNameOf(uri)), used)
            val mime = contentResolver.getType(uri)?.takeIf { it.isNotEmpty() }
                ?: mimeForName(name)
            Picked(uri, name, mime, sizeOf(uri))
        }
        val entries = picked.map { TransferEntry(it.name, it.size, it.mime, isDir = false) }
        transferLimitError(entries)?.let { throw IllegalStateException(it) }

        val stage = ClipRuntime.newStageDir(this)
        try {
            val specs = ArrayList<TransferSpec>(picked.size)
            var total = 0L
            for (file in picked) {
                val dest = File(stage, file.name)
                val written = copyTo(dest, file.uri, total)
                total += written
                specs += TransferSpec(dest.absolutePath, file.name, file.mime)
            }
            send(transferFilesJson(specs), stage, "正在发送 ${specs.size} 项…")
        } catch (t: Throwable) {
            stage.deleteRecursively()
            throw t
        }
    }

    // ---------- 文件夹：递归复制整棵树，保留相对路径 ----------

    private fun sendFolder(treeUri: Uri) {
        val rootName = safeTransferName(treeRootName(treeUri))
        val stage = ClipRuntime.newStageDir(this)
        val dest = File(stage, rootName)
        try {
            if (!dest.mkdirs()) throw IllegalStateException("无法创建传输缓存目录")
            val entries = ArrayList<TransferEntry>()
            entries += TransferEntry(rootName, 0, "", isDir = true)
            copyTree(treeUri, DocumentsContract.getTreeDocumentId(treeUri), rootName, dest, entries)
            transferLimitError(entries)?.let { throw IllegalStateException(it) }
            val json = transferFilesJson(listOf(TransferSpec(dest.absolutePath)))
            send(json, stage, "正在发送文件夹…")
        } catch (t: Throwable) {
            stage.deleteRecursively()
            throw t
        }
    }

    private fun copyTree(
        treeUri: Uri,
        parentDocumentId: String,
        prefix: String,
        destDir: File,
        entries: MutableList<TransferEntry>,
    ) {
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, parentDocumentId)
        val projection = arrayOf(
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE,
        )
        val cursor = contentResolver.query(children, projection, null, null, null)
            ?: throw IllegalStateException("无法读取文件夹")
        cursor.use {
            val used = HashSet<String>()
            while (it.moveToNext()) {
                val docId = it.getString(0) ?: continue
                val rawName = it.getString(1) ?: "文件"
                val name = uniqueTransferName(safeTransferName(rawName), used)
                val mime = it.getString(2) ?: mimeForName(name)
                val relative = "$prefix/$name"
                if (mime == DocumentsContract.Document.MIME_TYPE_DIR) {
                    entries += TransferEntry(relative, 0, mime, isDir = true)
                    limitOrThrow(entries)
                    val childDir = File(destDir, name)
                    if (!childDir.mkdirs()) {
                        throw IllegalStateException("无法创建目录：$relative")
                    }
                    copyTree(treeUri, docId, relative, childDir, entries)
                } else {
                    val dest = File(destDir, name)
                    val prior = entries.asSequence().filter { !it.isDir }.sumOf { it.size }
                    val written = copyTo(
                        dest,
                        DocumentsContract.buildDocumentUriUsingTree(treeUri, docId),
                        prior,
                    )
                    entries += TransferEntry(relative, written, mime, isDir = false)
                    limitOrThrow(entries)
                }
            }
        }
    }

    private fun limitOrThrow(entries: List<TransferEntry>) {
        transferLimitError(entries)?.let { throw IllegalStateException(it) }
    }

    // ---------- 复制与 SAF 元数据 ----------

    /** 复制一个 URI；[priorTotal] 是已复制字节，用于 8 GiB 总上限的即时兜底。 */
    private fun copyTo(dest: File, uri: Uri, priorTotal: Long): Long {
        val input = contentResolver.openInputStream(uri)
            ?: throw IllegalStateException("无法读取：${displayNameOf(uri)}")
        var written = 0L
        var total = priorTotal
        input.use { source ->
            dest.outputStream().use { sink ->
                val buffer = ByteArray(64 * 1024)
                while (true) {
                    val read = source.read(buffer)
                    if (read < 0) break
                    written += read
                    if (written > MAX_TRANSFER_FILE_BYTES) {
                        throw IllegalStateException("单个文件超过 2 GiB")
                    }
                    total += read
                    if (total > MAX_TRANSFER_TOTAL_BYTES) {
                        throw IllegalStateException("总大小超过 8 GiB，请分批发送")
                    }
                    sink.write(buffer, 0, read)
                }
            }
        }
        return written
    }

    private fun displayNameOf(uri: Uri): String {
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
            ?.use { cursor ->
                if (cursor.moveToFirst()) return cursor.getString(0) ?: "文件"
            }
        return uri.lastPathSegment ?: "文件"
    }

    private fun sizeOf(uri: Uri): Long {
        contentResolver.query(uri, arrayOf(OpenableColumns.SIZE), null, null, null)
            ?.use { cursor ->
                if (cursor.moveToFirst() && !cursor.isNull(0)) return cursor.getLong(0)
            }
        return 0L
    }

    private fun treeRootName(treeUri: Uri): String {
        val docUri = DocumentsContract.buildDocumentUriUsingTree(
            treeUri,
            DocumentsContract.getTreeDocumentId(treeUri),
        )
        contentResolver.query(
            docUri,
            arrayOf(DocumentsContract.Document.COLUMN_DISPLAY_NAME),
            null,
            null,
            null,
        )?.use { cursor ->
            if (cursor.moveToFirst()) return cursor.getString(0) ?: "文件夹"
        }
        return "文件夹"
    }

    private fun send(filesJson: String, stage: File, message: String) {
        runOnUiThread {
            ClipRuntime.sendStagedFiles(peerId, filesJson, stage)
            Toast.makeText(this, message, Toast.LENGTH_SHORT).show()
            finish()
        }
    }

    // ---------- 文本输入（IME 面板里无法稳定拉起自己的键盘） ----------

    private fun showTextDialog() {
        val edit = EditText(this).apply {
            hint = "要发送的文本"
            minLines = 3
            maxLines = 8
            setSingleLine(false)
        }
        val dialog = AlertDialog.Builder(this, android.R.style.Theme_Material_Light_Dialog_Alert)
            .setTitle("发送文本")
            .setMessage("发送给所有已配对且在线的设备")
            .setView(edit)
            .setPositiveButton("发送", null)
            .setNegativeButton("取消") { _, _ -> finish() }
            .setOnCancelListener { finish() }
            .create()
        dialog.setOnShowListener {
            dialog.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener {
                val text = edit.text?.toString().orEmpty()
                if (text.isEmpty()) {
                    edit.error = "文本为空"
                    return@setOnClickListener
                }
                if (!hasTrustedTarget(ClipRuntime.peers)) {
                    Toast.makeText(this, "没有已配对设备", Toast.LENGTH_SHORT).show()
                    return@setOnClickListener
                }
                Toast.makeText(this, ClipRuntime.publishText(text), Toast.LENGTH_SHORT).show()
                dialog.dismiss()
                finish()
            }
        }
        dialog.show()
    }

    private fun toastAndFinish(message: String) {
        Toast.makeText(this, message, Toast.LENGTH_LONG).show()
        finish()
    }

    companion object {
        const val MODE_MEDIA = "media"
        const val MODE_FILES = "files"
        const val MODE_FOLDER = "folder"
        const val MODE_TEXT = "text"

        private const val EXTRA_MODE = "xyz.erik.opi.transfer.MODE"
        private const val EXTRA_PEER = "xyz.erik.opi.transfer.PEER"
        private const val REQUEST_PICK = 73

        fun start(context: Context, mode: String, peerId: String = "") {
            context.startActivity(
                Intent(context, TransferActivity::class.java)
                    .putExtra(EXTRA_MODE, mode)
                    .putExtra(EXTRA_PEER, peerId)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
        }
    }
}
