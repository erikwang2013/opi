// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// 校验「cmake --install 装到哪」== 「loadDictionary() 去哪找」。
//
// loadDictionary() 走 fcitx::StandardPath::locate(Type::Data, "opi/luna.opid")，
// 落点由 fcitx5 按 XDG 数据目录规则算出来。这件事**只能在装了 fcitx5 的机器上
// 问库**：读 CMake 变量、读文档都推不出来 —— 同一条路径用 Type::Data 和
// Type::PkgData 问，结果只差一个 fcitx5/ 前缀，而装错了**不报错**，只是静默
// 回退内置词库（候选质量骤降，与「本来就没装词库」在用户侧无法区分）。
//
// 三个期望值由**调用方**给：CI 从 `cmake --install` 自己打印的 "Installing:"
// 行里取。于是这里是「安装器说它装到了哪」对「fcitx5 说它去哪找」——
// 两个独立来源，不是拿我自己的期望值自证。
//
// 用法：
//   opi_locate_check <期望词库路径> [期望 addon conf] [期望 inputmethod conf]
// 退出码：0 全相符 / 1 有不相符 / 2 用法错。

#include <fcitx-utils/standardpath.h>

#include <cstdio>
#include <string>

namespace {

int failures = 0;

void check(const char *what, fcitx::StandardPath::Type type,
           const char *relative, const char *expected) {
    const std::string got =
        fcitx::StandardPath::global().locate(type, relative);
    // 空串 = 库在 XDG 数据目录里没找到这个文件。这与「找到了但路径不同」
    // 是两种不同的坏法，都要报出来（前者多半是没装，后者是装错地方）。
    const bool ok = !got.empty() && got == expected;
    printf("%-4s %-10s %-26s -> %s\n", ok ? "ok" : "FAIL", what, relative,
           got.empty() ? "(没找到)" : got.c_str());
    if (!ok) {
        printf("       期望: %s\n", expected);
        ++failures;
    }
}

} // namespace

int main(int argc, char **argv) {
    if (argc < 2) {
        fprintf(stderr,
                "用法: %s <期望词库路径> [期望 addon conf] [期望 inputmethod conf]\n",
                argv[0]);
        return 2;
    }

    // ⚠️ 下面两个相对路径字面量必须与 opi_fcitx5.cpp 的 loadDictionary() 一致。
    // 不一致时本程序会**照样通过**（它没读那份源码），所以 CI 里另有一条 grep
    // 把 opi_fcitx5.cpp 的那一行钉住 —— 改常量的人会同时看到这条红。
    check("词库", fcitx::StandardPath::Type::Data, "opi/luna.opid", argv[1]);
    if (argc > 2) {
        check("addon", fcitx::StandardPath::Type::PkgData,
              "addon/opi_fcitx5.conf", argv[2]);
    }
    if (argc > 3) {
        check("输入法", fcitx::StandardPath::Type::PkgData,
              "inputmethod/opi.conf", argv[3]);
    }

    if (failures != 0) {
        printf("\n%d 项不符\n", failures);
        return 1;
    }
    printf("\n全部相符\n");
    return 0;
}
