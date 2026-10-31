// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 语音输入纯逻辑门禁：语言路由、模型就绪判定、清单格式、状态机、事件解析、
//! AsrConfig JSON。全部不碰麦克风/网络/真库。

package xyz.erik.opi.candidate

import java.nio.file.Files
import java.nio.file.Paths
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SpeechTest {

    @Test
    fun `language routing picks whisper only for explicit english`() {
        // 用户口径：zh/auto/短英文默认 SenseVoice；只有明确选 English 才上 Whisper。
        assertEquals(SpeechEngine.SENSEVOICE, SpeechLanguage.AUTO.engine)
        assertEquals(SpeechEngine.SENSEVOICE, SpeechLanguage.ZH.engine)
        assertEquals(SpeechEngine.WHISPER, SpeechLanguage.EN.engine)
        assertEquals("auto", SpeechLanguage.fromValue(null).value)
        assertEquals("auto", SpeechLanguage.fromValue("fr").value)
        assertEquals("en", SpeechLanguage.fromValue("en").value)
        // Whisper 的短句限制必须出现在 UI 文案里（15 秒），不能只靠文档。
        assertTrue(SpeechLanguage.EN.label, SpeechLanguage.EN.label.contains("高精度"))
        assertTrue(SpeechEngine.WHISPER.detail, SpeechEngine.WHISPER.detail.contains("15 秒"))
    }

    @Test
    fun `model sizes and hashes are pinned per engine`() {
        // 模型不进包，下载校验全靠这组常量；写错 = 用户下完 240 MB 才发现 SHA 不对。
        for (spec in SpeechEngine.entries.flatMap { it.requiredFiles() }) {
            assertTrue("${spec.name} 大小必须为正", spec.size > 0)
            assertEquals("${spec.name} SHA-256 长度", 64, spec.sha256.length)
            assertTrue(
                "${spec.name} SHA-256 含非十六进制字符",
                spec.sha256.all { it in "0123456789abcdef" },
            )
        }
        assertTrue(SpeechEngine.SENSEVOICE.requiredFiles().contains(VAD_FILE))
        assertTrue(SpeechEngine.WHISPER.requiredFiles().contains(VAD_FILE))
        assertEquals(SpeechEngine.SENSEVOICE.totalBytes(), SENSEVOICE_MODEL.files.sumOf { it.size } + VAD_FILE.size)
        assertEquals(SpeechEngine.WHISPER.totalBytes(), WHISPER_MODEL.files.sumOf { it.size } + VAD_FILE.size)
        // 两个引擎的模型文件不能同名（同一下载根下会互相覆盖）。
        val names = (SENSEVOICE_MODEL.files + WHISPER_MODEL.files).map { it.name }
        assertEquals(names.size, names.toSet().size)
    }

    @Test
    fun `model is not ready without manifest and exact-size files`() {
        val root = Files.createTempDirectory("opi-speech-model").toFile()
        try {
            val rootPath = root.toPath()
            for (engine in SpeechEngine.entries) {
                assertFalse("$engine 空目录不该就绪", modelReady(engine, rootPath))
            }
            // 只有清单、没有模型文件 ⇒ 不就绪（不是「有 manifest 就信」）。
            val dir = rootPath.resolve(SENSEVOICE_MODEL.dirName)
            Files.createDirectories(dir)
            Files.writeString(dir.resolve("manifest.json"), manifestJson(SENSEVOICE_MODEL.files))
            assertFalse(modelReady(SpeechEngine.SENSEVOICE, rootPath))
            // 文件名字对、大小不对（0 字节）⇒ 不就绪（modelReady 不哈希，Rust 会再验）。
            for (spec in SENSEVOICE_MODEL.files) {
                Files.write(dir.resolve(spec.name), ByteArray(1))
            }
            Files.createDirectories(rootPath.resolve("vad"))
            Files.write(vadPath(rootPath), ByteArray(1))
            assertFalse(modelReady(SpeechEngine.SENSEVOICE, rootPath))
        } finally {
            root.deleteRecursively()
        }
    }

    @Test
    fun `manifest json matches the rust schema`() {
        val obj = parseLine(manifestJson(SENSEVOICE_MODEL.files))
            ?: error("manifest 不是 JSON：${manifestJson(SENSEVOICE_MODEL.files)}")
        assertEquals(1L, (obj["version"] as JVal.JNum).v)
        val files = (obj["files"] as JVal.JArr).v.map { (it as JVal.JObj).v }
        assertEquals(SENSEVOICE_MODEL.files.size, files.size)
        val model = files[0]
        assertEquals("model.int8.onnx", (model["path"] as JVal.JStr).v)
        assertEquals(239_233_841L, (model["size"] as JVal.JNum).v)
        assertEquals(64, (model["sha256"] as JVal.JStr).v.length)
        assertEquals("model", (model["role"] as JVal.JStr).v)
        // VAD 直接按路径打开，不应混进模型清单。
        assertTrue(files.none { (it["path"] as JVal.JStr).v == VAD_FILE.name })
    }

    @Test
    fun `speech model root is private and platform specific`() {
        assertEquals(
            Paths.get("/xdg", "opi", "models"),
            speechModelRoot(windows = false, localAppData = null, xdgDataHome = "/xdg", home = "/home/u"),
        )
        assertEquals(
            Paths.get("/home/u", ".local", "share", "opi", "models"),
            speechModelRoot(windows = false, localAppData = null, xdgDataHome = "", home = "/home/u"),
        )
        val win = speechModelRoot(
            windows = true, localAppData = "C:\\Users\\u\\AppData\\Local",
            xdgDataHome = null, home = null,
        )
        assertEquals("models", win.fileName.toString())
        assertEquals("opi", win.parent.fileName.toString())
    }

    @Test
    fun `state machine only accepts transitions from the expected phase`() {
        assertEquals(SpeechPhase.PREPARING, nextSpeechPhase(SpeechPhase.IDLE, SpeechTransition.START))
        assertEquals(SpeechPhase.PREPARING, nextSpeechPhase(SpeechPhase.ERROR, SpeechTransition.START))
        assertEquals(SpeechPhase.RECORDING, nextSpeechPhase(SpeechPhase.PREPARING, SpeechTransition.READY))
        assertEquals(SpeechPhase.RECOGNIZING, nextSpeechPhase(SpeechPhase.RECORDING, SpeechTransition.STOP))
        assertEquals(SpeechPhase.DONE, nextSpeechPhase(SpeechPhase.RECOGNIZING, SpeechTransition.FINAL))
        assertEquals(SpeechPhase.ERROR, nextSpeechPhase(SpeechPhase.DONE, SpeechTransition.FAIL))
        assertEquals(SpeechPhase.IDLE, nextSpeechPhase(SpeechPhase.ERROR, SpeechTransition.CANCEL))
        assertEquals(SpeechPhase.IDLE, nextSpeechPhase(SpeechPhase.DONE, SpeechTransition.DISMISS))
        // 迟到/错位的转移不改档：取消后的 final 不能把浮层重新点亮成「完成」。
        assertEquals(SpeechPhase.IDLE, nextSpeechPhase(SpeechPhase.IDLE, SpeechTransition.READY))
        assertEquals(SpeechPhase.IDLE, nextSpeechPhase(SpeechPhase.IDLE, SpeechTransition.FINAL))
        assertEquals(SpeechPhase.PREPARING, nextSpeechPhase(SpeechPhase.PREPARING, SpeechTransition.STOP))
        assertEquals(SpeechPhase.RECORDING, nextSpeechPhase(SpeechPhase.RECORDING, SpeechTransition.READY))
    }

    @Test
    fun `event parsing handles level floats and text fallbacks`() {
        assertEquals("level", eventType("""{"type":"level","rms_db":-30.0}"""))
        assertEquals("final", eventType("""{"type" : "final", "text":"你好"}"""))
        assertNull(eventType("""{"text":"no type"}"""))
        assertNull(eventType("not json"))
        // rms_db -60..0 → 0..1，超界夹住。
        assertEquals(0.5f, eventLevel("""{"type":"level","rms_db":-30}""") ?: -1f, 0.0001f)
        assertEquals(0f, eventLevel("""{"type":"level","rms_db":-60}""") ?: -1f, 0.0001f)
        assertEquals(1f, eventLevel("""{"type":"level","rms_db":0}""") ?: -1f, 0.0001f)
        assertEquals(0f, eventLevel("""{"type":"level","rms_db":-99}""") ?: -1f, 0.0001f)
        assertNull(eventLevel("""{"type":"level"}"""))
        // 正文走 parseLine（转义的中文/引号都还原），error 兼容 message 字段。
        assertEquals("你好", eventText("""{"type":"final","text":"你好"}"""))
        assertEquals("a\"b", eventText("""{"type":"final","text":"a\"b"}"""))
        assertEquals("模型文件缺失", eventText("""{"type":"error","message":"模型文件缺失"}"""))
        assertNull(eventText("not json"))
        assertNull(eventText("""{"type":"final"}"""))
    }

    @Test
    fun `asr config json carries absolute model paths and language`() {
        val root = Paths.get("/tmp/模型")
        val cfg = parseLine(asrConfigJson(root, SpeechLanguage.EN)) ?: error("AsrConfig 不是 JSON")
        assertEquals("/tmp/模型", (cfg["models_dir"] as JVal.JStr).v)
        assertEquals("sensevoice", (cfg["sense_voice_dir"] as JVal.JStr).v)
        assertEquals("whisper-small.en", (cfg["whisper_dir"] as JVal.JStr).v)
        assertEquals("vad/silero_vad.onnx", (cfg["vad_model"] as JVal.JStr).v)
        assertEquals("en", (cfg["language"] as JVal.JStr).v)
        assertEquals(2L, (cfg["num_threads"] as JVal.JNum).v)
        assertEquals(JVal.JBool(true), cfg["verify_models"])
        assertEquals("zh", (parseLine(asrConfigJson(root, SpeechLanguage.ZH))!!["language"] as JVal.JStr).v)
        assertEquals("auto", (parseLine(asrConfigJson(root, SpeechLanguage.AUTO))!!["language"] as JVal.JStr).v)
    }
}
