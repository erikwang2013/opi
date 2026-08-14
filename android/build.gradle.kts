// 根构建脚本：插件版本声明（M4 flutter 工程验证过的组合：AGP 9.0.1 + Kotlin 2.3.20）。
// org.jetbrains.kotlin.plugin.compose 版本随 Kotlin（2.3.20）。

tasks.register<Delete>("clean") {
    delete(rootProject.layout.buildDirectory)
}
