// ---------- opi_asr：本地离线语音识别（2026-11-02）----------
// 四个库必须同目录：主库没有 RPATH（NEEDED `libsherpa-onnx-c-api.so`），AsrFfi 先按
// 绝对路径 preload onnxruntime/cxx/c-api，再载主库（Linux 的 c-api 自带 `$ORIGIN`，
// Windows 靠已加载模块的 basename 解析）。模型**不进包**，首启由设置页下载。
// 2026-11-01 用户裁决：引擎也只随完整包；普通/基础包在设置页下载组件。
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
val asrLibDir = asrLibOverride.map { file(it) }.orElse(asrFromCargo)

fun checkAsrLibs(dir: File) {
    asrLibNames.forEach { name ->
        check(File(dir, name).isFile) {
            "缺 $name —— 先 cargo build --release --locked -p opi-asr（或用 OPI_ASR_LIB_DIR）"
        }
    }
}

val asrEngineBundled =
    providers.gradleProperty("opiPackageTier").getOrElse("normal") == "full"

if (asrEngineBundled) {
    tasks.withType<org.jetbrains.compose.desktop.application.tasks.AbstractJPackageTask>()
        .configureEach {
            dependsOn(buildOpiAsr)
            if (asrLibOverride.isPresent) {
                checkAsrLibs(asrLibDir.get())
            }
            doFirst { checkAsrLibs(asrLibDir.get()) }
            files.from(asrLibDir.map { d -> asrLibNames.map { File(d, it) } })
        }
}

// 发布用组件：普通包/基础包从站点按需下载（大小 + SHA-256 在 version.json）。
val asrEngineZipName =
    if (System.getProperty("os.name").lowercase().contains("win")) {
        "opi-asr-engine-windows-x64.zip"
    } else {
        "opi-asr-engine-linux-x86_64.zip"
    }
tasks.register<org.gradle.api.tasks.bundling.Zip>("packageAsrEngine") {
    group = "build"
    description = "打包语音引擎组件（供普通包/基础包按需下载）"
    dependsOn(buildOpiAsr)
    from(asrLibDir) { include(asrLibNames) }
    archiveFileName.set(asrEngineZipName)
    destinationDirectory.set(layout.buildDirectory.dir("outputs/asr-engine"))
    isPreserveFileTimestamps = false
    isReproducibleFileOrder = true
    doFirst { checkAsrLibs(asrLibDir.get()) }
}

// 完整包：模型目录由主脚本的 appResourcesRootDir 直接指向（目录必须已存在），
// 这里只校验参数，避免再 Sync 一遍 588MB。
if (providers.gradleProperty("opiPackageTier").getOrElse("normal") == "full") {
    check(layout.projectDirectory.dir("asr-bundle/asr").asFile.isDirectory) {
        "full 档缺 asr-bundle/asr（构建前先把模型放进该目录）"
    }
}
