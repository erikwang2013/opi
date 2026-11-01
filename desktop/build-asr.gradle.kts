// ---------- opi_asr：本地离线语音识别（2026-11-02）----------
// 四个库必须同目录：主库没有 RPATH（NEEDED `libsherpa-onnx-c-api.so`），AsrFfi 先按
// 绝对路径 preload onnxruntime/cxx/c-api，再载主库（Linux 的 c-api 自带 `$ORIGIN`，
// Windows 靠已加载模块的 basename 解析）。模型**不进包**，首启由设置页下载。
val asrLibNames =
    if (System.getProperty("os.name").lowercase().contains("win")) {
        listOf("opi_asr.dll", "sherpa-onnx-c-api.dll", "sherpa-onnx-cxx-api.dll", "onnxruntime.dll")
    } else {
        listOf("libopi_asr.so", "libsherpa-onnx-c-api.so", "libsherpa-onnx-cxx-api.so", "libonnxruntime.so")
    }
val asrLibOverride = providers.environmentVariable("OPI_ASR_LIB_DIR")
val asrFromCargo = file("../target/release")
val buildOpiAsr = tasks.register<Exec>("buildOpiAsr") {
    onlyIf { !asrLibOverride.isPresent }
    workingDir = projectDir.parentFile
    commandLine("cargo", "build", "--release", "--locked", "-p", "opi-asr")
}

tasks.withType<org.jetbrains.compose.desktop.application.tasks.AbstractJPackageTask>()
    .configureEach {
        dependsOn(buildOpiAsr)
        val dir = asrLibOverride.map { file(it) }.orElse(asrFromCargo)
        if (asrLibOverride.isPresent) {
            asrLibNames.forEach { name ->
                check(File(dir.get(), name).isFile) {
                    "OPI_ASR_LIB_DIR 里缺 $name：${File(dir.get(), name).path}"
                }
            }
        }
        doFirst {
            asrLibNames.forEach { name ->
                check(File(dir.get(), name).isFile) {
                    "缺 $name —— 先 cargo build --release --locked -p opi-asr（或用 OPI_ASR_LIB_DIR）"
                }
            }
        }
        files.from(dir.map { d -> asrLibNames.map { File(d, it) } })
    }

