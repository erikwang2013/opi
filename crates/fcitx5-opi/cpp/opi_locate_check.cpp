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
// 期望值由**调用方**给：CI 从 `cmake --install` 自己打印的 "Installing:"
// 行里取。于是这里是「安装器说它装到了哪」对「fcitx5 说它去哪找」——
// 两个独立来源，不是拿我自己的期望值自证。
//
// 图标那一条是**例外**（它没有期望值参数）：期望值就是装好的那份
// opi.conf 里的 `Icon=` —— 见下面 checkIcon() 的说明。
//
// 用法：
//   opi_locate_check <期望词库路径> [期望 addon conf] [期望 inputmethod conf]
// 退出码：0 全相符 / 1 有不相符 / 2 用法错。

#include <fcitx-utils/standardpath.h>

#include <cstdio>
#include <fstream>
#include <string>

namespace {

int failures = 0;

// StandardPath 自 5.1.21 起弃用，但**不能迁移**（老版本没有 StandardPaths）。
// 为什么 / 边界在哪 / 何时删这三件事只写在一处：cpp/CMakeLists.txt 的
// 「为什么两个 .cpp 里有 -Wdeprecated-declarations」段 —— 别在这里复制。
// 本文件通篇就是在问 StandardPath，故抑制恰好包住这一个函数（全文件唯一的
// StandardPath 调用点）；调用方不再各自包一遍。
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
std::string locate(fcitx::StandardPath::Type type, const std::string &relative) {
    return fcitx::StandardPath::global().locate(type, relative);
}
#pragma GCC diagnostic pop

void check(const char *what, fcitx::StandardPath::Type type,
           const char *relative, const char *expected) {
    const std::string got = locate(type, relative);
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

// 图标单独一条：`Icon=` 是**主题图标名**、不是路径，由 fcitx5 的
// IconTheme::findIcon 在 XDG 图标目录里按名字解析（hicolor 是 XDG 规定的公共
// 回退主题，任何主题的继承链里都有它 —— 实测本机 nirvana 的链是
// nirvana→Papirus→bloom→breeze→hicolor→Papirus，它的 index.theme 里
// scalable/apps 是列出的 649 个目录之一）。装错的地方**不报错** —— 用户只是
// 看不到图，回退成文字标签，与「本来就没配图标」在用户侧不可区分。
//
// 所以这里不问「图标文件装了没有」（那只证明拷贝成功），而是**从装好的那份
// opi.conf 里读 Icon=**，再拿这个值去问 fcitx5 的查找器。名字改了、装到别的
// 子目录、或 conf 与文件不同名，三种坏法都会在这里红。副作用是期望值仍然
// 只有一个来源：随包发出去的那两个文件自己。
int checkIcon(const char *confPath) {
    std::ifstream in(confPath);
    if (!in) {
        printf("FAIL 图标       %s -> (读不出这份 conf)\n", confPath);
        return ++failures;
    }
    std::string icon;
    for (std::string line; std::getline(in, line);) {
        // 行首锚定：被注释掉的 `#Icon=` 不算。
        if (line.rfind("Icon=", 0) == 0) {
            icon = line.substr(5);
            break;
        }
    }
    if (icon.empty()) {
        printf("FAIL 图标       %s -> (conf 里没有 Icon=)\n", confPath);
        return ++failures;
    }
    // 扩展名与 IconTheme::findIcon 的缺省表同序（.svg/.png/.xpm）。
    for (const char *ext : {".svg", ".png", ".xpm"}) {
        const std::string rel =
            "icons/hicolor/scalable/apps/" + icon + ext;
        const std::string got = locate(fcitx::StandardPath::Type::Data, rel);
        if (!got.empty()) {
            printf("ok   图标       %-26s -> %s\n", rel.c_str(), got.c_str());
            return failures;
        }
    }
    printf("FAIL 图标       Icon=%s -> (XDG 图标目录里没有它)\n", icon.c_str());
    printf("       期望: <前缀>/share/icons/hicolor/scalable/apps/%s.{svg,png,xpm}\n",
           icon.c_str());
    printf("       成因二选一：cpp/CMakeLists.txt 没装这个文件，"
           "或装出来的名字与 opi.conf 的 Icon= 不一致\n");
    return ++failures;
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
        // 期望值取自刚校验过落点的那份 conf 本身（见 checkIcon 的说明），
        // 故复用 argv[3]，不新增第四个参数 —— CI 现有的三参调用已经覆盖它。
        checkIcon(argv[3]);
    }

    if (failures != 0) {
        printf("\n%d 项不符\n", failures);
        return 1;
    }
    printf("\n全部相符\n");
    return 0;
}
