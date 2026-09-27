// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 解析 opi_fcitx5_candidates 的返回值：serde_json 对 Vec<String> 的输出
// （`["好","你"]` —— 非 ASCII 直出 UTF-8，不转义成 \uXXXX，除非原字符是控制
// 字符）。**只认这一个形状**，不做通用 JSON：这里只有一处调用，通用解析器的
// 复杂度换不回任何东西。
//
// 从 opi_fcitx5.cpp 拆出来（那份文件贴着 500 行硬规矩，加模式热键前必须先腾地方）。
// cpp/opi_json_check.cpp 逐字节对拍本解析器与真 serde_json（手工 harness，不进 CI）。
//
// `inline` 而非 `static`：本头被 opi_fcitx5.cpp / opi_json_check.cpp 两个 TU 各自
// 包含，static 会给用不到的 TU 招来 -Wunused-function（三处 g++ 都带 -Werror）。
#ifndef OPI_JSON_H
#define OPI_JSON_H

#include <cstdint>
#include <string>
#include <utility>
#include <vector>

// 读 4 位十六进制。越界或遇非十六进制字符返回 false（调用方负责收摊）。
inline bool parseHex4(const std::string &s, size_t pos, uint32_t &out) {
    if (pos + 4 > s.size()) {
        return false;
    }
    uint32_t v = 0;
    for (size_t i = 0; i < 4; ++i) {
        const char c = s[pos + i];
        v <<= 4;
        if (c >= '0' && c <= '9') {
            v |= static_cast<uint32_t>(c - '0');
        } else if (c >= 'a' && c <= 'f') {
            v |= static_cast<uint32_t>(c - 'a' + 10);
        } else if (c >= 'A' && c <= 'F') {
            v |= static_cast<uint32_t>(c - 'A' + 10);
        } else {
            return false;
        }
    }
    out = v;
    return true;
}

inline void appendUtf8(std::string &out, uint32_t cp) {
    if (cp < 0x80) {
        out.push_back(static_cast<char>(cp));
    } else if (cp < 0x800) {
        out.push_back(static_cast<char>(0xC0 | (cp >> 6)));
        out.push_back(static_cast<char>(0x80 | (cp & 0x3F)));
    } else if (cp < 0x10000) {
        out.push_back(static_cast<char>(0xE0 | (cp >> 12)));
        out.push_back(static_cast<char>(0x80 | ((cp >> 6) & 0x3F)));
        out.push_back(static_cast<char>(0x80 | (cp & 0x3F)));
    } else {
        out.push_back(static_cast<char>(0xF0 | (cp >> 18)));
        out.push_back(static_cast<char>(0x80 | ((cp >> 12) & 0x3F)));
        out.push_back(static_cast<char>(0x80 | ((cp >> 6) & 0x3F)));
        out.push_back(static_cast<char>(0x80 | (cp & 0x3F)));
    }
}

inline std::vector<std::string> parseJsonStringArray(const std::string &json) {
    std::vector<std::string> out;
    size_t i = 0;
    auto skipWs = [&json, &i] {
        while (i < json.size() && (json[i] == ' ' || json[i] == '\t' ||
                                   json[i] == '\n' || json[i] == '\r')) {
            ++i;
        }
    };
    skipWs();
    if (i >= json.size() || json[i] != '[') {
        return out; // 空串（Rust 侧未装载/出错时返回值）也走这里
    }
    ++i;
    while (true) {
        skipWs();
        if (i >= json.size() || json[i] != '"') {
            break;
        }
        ++i;
        std::string s;
        bool closed = false;
        while (i < json.size()) {
            const char c = json[i++];
            if (c == '"') {
                closed = true;
                break;
            }
            if (c != '\\') {
                s.push_back(c);
                continue;
            }
            if (i >= json.size()) {
                break;
            }
            const char esc = json[i++];
            if (esc == 'u') {
                uint32_t cp = 0;
                if (!parseHex4(json, i, cp)) {
                    i = json.size(); // 坏转义：收摊，已解析的部分照常返回
                    break;
                }
                i += 4;
                // 代理对（😀）：低半区必须紧跟，否则按单码点编码。
                if (cp >= 0xD800 && cp <= 0xDBFF && i + 6 <= json.size() &&
                    json[i] == '\\' && json[i + 1] == 'u') {
                    uint32_t lo = 0;
                    if (parseHex4(json, i + 2, lo) && lo >= 0xDC00 &&
                        lo <= 0xDFFF) {
                        cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                        i += 6;
                    }
                }
                appendUtf8(s, cp);
                continue;
            }
            switch (esc) {
            case 'b':
                s.push_back('\b');
                break;
            case 'f':
                s.push_back('\f');
                break;
            case 'n':
                s.push_back('\n');
                break;
            case 'r':
                s.push_back('\r');
                break;
            case 't':
                s.push_back('\t');
                break;
            default:
                s.push_back(esc); // \" \\ \/ 及未知转义：取字面字符
                break;
            }
        }
        if (!closed) {
            break;
        }
        out.push_back(std::move(s));
        skipWs();
        if (i < json.size() && json[i] == ',') {
            ++i;
            continue;
        }
        break;
    }
    return out;
}

#endif // OPI_JSON_H
