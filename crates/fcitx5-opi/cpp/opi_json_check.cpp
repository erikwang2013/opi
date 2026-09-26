// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// opi_fcitx5.cpp 里 parseJsonStringArray 的对照检查 —— **手工 harness，不进 CI**。
// 跑法见同目录 run-harness.sh 或 README「验证 harness」一节。
//
// 为什么要有它：胶水不引 JSON 库，自己写了个窄解析器只认 opi_fcitx5_candidates
// 产出的 `Vec<String>`。窄解析器最容易出的错是「转义 / 代理对 / 非 ASCII 处理
// 与真 serde_json 不一致」—— 那种错在 C++ 侧看不出，只会让候选框里出现乱码。
// 所以这里喂**真 serde_json 的输出**，逐字节比对解析结果。
//
// 向量文件 opi_json_vec.txt 两行：
//   第 1 行 = serde_json::to_string(&Vec<String>) 的真实输出
//   第 2 行 = 期望字节的 hex（每串 UTF-8 字节后跟一个 "00" 当分隔符）
// 重新生成的办法：把下面这段塞进任意带 serde_json 的 cargo 工程跑一遍，
// 输出重定向进 opi_json_vec.txt（本 harness 不含生成器，避免为此养一个 crate）:
//
//   let v: Vec<String> = vec!["你".into(), "a\"b".into(), "x\\y".into(),
//       "tab\there".into(), "nl\nhere".into(), "ctrl\u{1}char".into(),
//       "😀emoji".into(), "".into(), "斜杠/与退格\u{8}".into()];
//   println!("{}", serde_json::to_string(&v).unwrap());
//   let mut j: Vec<u8> = Vec::new();
//   for s in &v { j.extend_from_slice(s.as_bytes()); j.push(0); }
//   println!("{}", j.iter().map(|b| format!("{:02x}", b)).collect::<String>());
//
// 用法：opi_json_check [向量文件]   （缺省用同目录的 opi_json_vec.txt）

#include "opi_fcitx5.cpp"

#include <cstdio>
#include <fstream>
#include <string>
#include <vector>

namespace {

std::string hexOf(const std::vector<std::string> &v) {
    std::string out;
    char buf[3];
    for (const auto &s : v) {
        for (unsigned char b : s) {
            std::snprintf(buf, sizeof(buf), "%02x", b);
            out += buf;
        }
        out += "00"; // 分隔符，与生成端一致
    }
    return out;
}

// 向量文件默认就在本源码旁边（__FILE__ 是编译时用的路径，脚本按该路径编译）。
std::string defaultVectorPath() {
    const std::string self = __FILE__;
    const auto slash = self.find_last_of('/');
    return (slash == std::string::npos ? std::string(".") : self.substr(0, slash)) +
           "/opi_json_vec.txt";
}

} // namespace

int main(int argc, char **argv) {
    const std::string vecPath = argc > 1 ? argv[1] : defaultVectorPath();
    std::ifstream in(vecPath);
    if (!in) {
        std::fprintf(stderr, "打不开向量文件: %s\n用法: %s [向量文件]\n",
                     vecPath.c_str(), argv[0]);
        return 2;
    }
    std::string json, want;
    std::getline(in, json);
    std::getline(in, want);
    if (json.empty() || want.empty()) {
        std::fprintf(stderr, "向量文件 %s 需要两行（JSON + 期望 hex）\n",
                     vecPath.c_str());
        return 2;
    }

    int fail = 0;
    const auto got = hexOf(parseJsonStringArray(json));
    std::printf("输入 JSON: %s\n", json.c_str());
    std::printf("期望字节: %s\n", want.c_str());
    std::printf("解析得到: %s\n", got.c_str());
    if (got != want) {
        std::printf("  [FAIL] serde_json 真输出 解析不符\n");
        ++fail;
    } else {
        std::printf("  [PASS] serde_json 真输出 解析逐字节一致（%zu 条）\n",
                    parseJsonStringArray(json).size());
    }

    // serde_json 默认不转义非 BMP，故代理对分支它不会产出 —— 手工构造一个。
    // ⚠️ 必须是**字面量的反斜杠+u 转义序列**（6 个字符的 \ud83d），不能写成真的
    // emoji —— 写真的就绕过了代理对分支，这条测试就变成假绿（吃过一次）。
    // 用拼接而不是把转义写进一个原始字符串：那样写极易被编辑器/工具链悄悄还原
    // 成真 emoji，且看不出区别。
    const std::string bs = "\\"; // 一个反斜杠字符
    const std::string surrogate =
        "[\"" + bs + "ud83d" + bs + "ude00\",\"你\",\"a\\/b\"]";
    const std::string surrogateWant = "f09f988000" "e4bda000" "612f6200"; // 😀/你/a/b
    const auto got2 = hexOf(parseJsonStringArray(surrogate));
    std::printf("代理对 JSON: %s\n", surrogate.c_str());
    // 防假绿的关键一步：**真 emoji 与字面转义解码后字节完全相同**，所以光比对
    // 输出根本区分不出二者 —— 必须直接查输入里有没有反斜杠。少了这一条，谁把源
    // 里的转义改回真 emoji，测试照样 PASS，而代理对分支其实一次都没跑过。
    if (surrogate.find(bs + "u") == std::string::npos) {
        std::printf("  [FAIL] 输入里没有字面反斜杠-u 转义 → 代理对分支未被覆盖（假绿）\n");
        ++fail;
    } else {
        std::printf("  [PASS] 输入确实是字面反斜杠-u 转义（代理对分支被覆盖）\n");
    }
    std::printf("期望字节: %s\n解析得到: %s\n", surrogateWant.c_str(),
                got2.c_str());
    if (got2 != surrogateWant) {
        std::printf("  [FAIL] \\uXXXX 代理对 / 转义斜杠 解析不符\n");
        ++fail;
    } else {
        std::printf("  [PASS] \\uXXXX 代理对与 \\/ 转义解析正确\n");
    }

    // 坏输入不崩：空串、截断。
    for (const std::string &bad : {std::string(), std::string("[\"abc"),
                                   std::string("garbage"), std::string("[\"a\\")}) {
        const auto r = parseJsonStringArray(bad);
        std::printf("  坏输入 %-12s -> %zu 条（不崩）\n",
                    bad.empty() ? "\"\"" : bad.c_str(), r.size());
    }

    std::printf("%s\n", fail == 0 ? "ALL PASS" : "HAS FAILURES");
    return fail == 0 ? 0 : 1;
}
