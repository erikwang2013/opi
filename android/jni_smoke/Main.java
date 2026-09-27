// Host JVM smoke：不依赖 Android，直接 System.load libopi_ffi.so 验证
// JNI_OnLoad + RegisterNatives + 引擎主链路。与 Rust cabi_test 覆盖同一语义。
//
// 用法：
//   javac -d /tmp/smoke-out Main.java
//   java -Dopi.so=/path/to/libopi_ffi.so -cp /tmp/smoke-out io.opi.input.jni.Main [/path/to/luna.opid]
// 成功打印 SMOKE-OK；失败打印 FAIL: <原因> 并退出码 1。
package io.opi.input.jni;

final class OpiEngine {
    static native boolean load(String path);
    static native String inputKey(String ch);
    static native void backspace();
    static native void clear();
    static native String select(int index);
    static native void switchMode(int mode);
    static native void setShift(boolean on);
    static native boolean toggleFullwidth();
    static native boolean fullwidthState();
    static native String toggleSymbol();
    static native boolean chinesePunct();
    static native void setChinesePunct(boolean on);
    static native boolean toggleChinesePunct();
    static native String inputSpace();
    static native String[] candidates(int limit);
    static native String buffer();
    static native int mode();
    static native String[] searchSymbols(String keyword);
    static native String[] emojiSymbols();
    static native String symbolBlocks();
    static native String[] symbolsInBlock(short id);
    static native boolean loadTrad(String path);
    static native boolean learnerEnabled();
    static native void setLearner(boolean enabled);
    static native void clearUserWords();
    static native String exportUserWords();
    static native int importUserWords(String json);
    static native void removeUserWord(String text);
}

public final class Main {
    static int failures = 0;

    static void check(boolean ok, String what) {
        if (!ok) {
            failures++;
            System.out.println("FAIL: " + what);
        }
    }

    public static void main(String[] args) throws Exception {
        String so = System.getProperty("opi.so", "/home/wwwroot/bag/opi/target/debug/libopi_ffi.so");
        System.load(so);
        System.out.println("loaded: " + so);

        // 装载：args[0] 词库路径；缺失走内置回退（也必须成功）。
        String dict = args.length > 0 ? args[0] : null;
        check(OpiEngine.load(dict), "load(" + dict + ") 应为 true");

        // 初始模式 Pinyin(0)
        check(OpiEngine.mode() == 0, "初始 mode 应为 0");

        // 输入 w → buffer=w
        check("".equals(OpiEngine.inputKey("w")), "inputKey(w) 返回空串");
        check("w".equals(OpiEngine.buffer()), "buffer 应为 w");
        check("".equals(OpiEngine.inputKey("o")), "inputKey(o) 返回空串");
        check("wo".equals(OpiEngine.buffer()), "buffer 应为 wo");

        // 候选非空 + select(0) 非空 + buffer 清空
        String[] cands = OpiEngine.candidates(8);
        check(cands != null && cands.length > 0, "candidates(8) 非空");
        String first = OpiEngine.select(0);
        check(first != null && !first.isEmpty(), "select(0) 非空");
        check("".equals(OpiEngine.buffer()), "select 后 buffer 清空");
        // select 越界 → 空串
        check("".equals(OpiEngine.select(999)), "select(999) 返回空串");

        // 单字符外输入返回空串（多字符/空）
        check("".equals(OpiEngine.inputKey("ab")), "inputKey(ab) 返回空串");
        check("".equals(OpiEngine.inputKey("")), "inputKey(空) 返回空串");
        check("".equals(OpiEngine.buffer()), "非法输入后 buffer 仍为空");

        // 模式切换：0→1，越界忽略
        OpiEngine.switchMode(1);
        check(OpiEngine.mode() == 1, "switchMode(1) 后 mode==1");
        OpiEngine.switchMode(9);
        check(OpiEngine.mode() == 1, "越界 mode 忽略");

        // English 模式：输入 abc → space 提交 → buffer 清空
        check("".equals(OpiEngine.inputKey("a")), "inputKey(a)");
        check("".equals(OpiEngine.inputKey("b")), "inputKey(b)");
        check("".equals(OpiEngine.inputKey("c")), "inputKey(c)");
        check("abc".equals(OpiEngine.buffer()), "buffer 应为 abc");
        check("abc".equals(OpiEngine.inputSpace()), "inputSpace 返回 abc");
        check("".equals(OpiEngine.buffer()), "space 后 buffer 清空");

        // Shift：大写锁定 + backspace
        OpiEngine.setShift(true);
        OpiEngine.inputKey("a");
        check("A".equals(OpiEngine.buffer()), "shift 后 buffer 应为 A");
        OpiEngine.setShift(false);
        OpiEngine.backspace();
        check("".equals(OpiEngine.buffer()), "backspace 后 buffer 清空");
        OpiEngine.switchMode(0);

        // Learner：默认开（M1 语义）→ 关闭 → 开启
        check(OpiEngine.learnerEnabled(), "learner 默认开启");
        OpiEngine.setLearner(false);
        check(!OpiEngine.learnerEnabled(), "setLearner(false) 生效");
        OpiEngine.setLearner(true);
        check(OpiEngine.learnerEnabled(), "setLearner(true) 生效");

        // 用户词导出/清空
        String words = OpiEngine.exportUserWords();
        check(words != null && !words.isEmpty() && words.contains("\"version\""), "exportUserWords 含 version");
        OpiEngine.clearUserWords();
        check("{\"version\":1,\"words\":[]}".equals(OpiEngine.exportUserWords()), "清空后导出为空列表");

        // 符号：块 + 块内符号 + 搜索
        String blocks = OpiEngine.symbolBlocks();
        check(blocks != null && blocks.contains("\"id\""), "symbolBlocks 含 id");
        short firstId = 0;
        java.util.regex.Matcher m = java.util.regex.Pattern.compile("\"id\":(\\d+)").matcher(blocks == null ? "" : blocks);
        if (m.find()) {
            firstId = Short.parseShort(m.group(1));
        }
        String[] syms = OpiEngine.symbolsInBlock(firstId);
        check(syms != null && syms.length > 0, "symbolsInBlock(" + firstId + ") 非空");
        String[] hits = OpiEngine.searchSymbols("he");
        boolean hasHeart = false;
        if (hits != null) {
            for (String h : hits) {
                if ("♥".equals(h)) {
                    hasHeart = true;
                }
            }
        }
        check(hasHeart, "searchSymbols(he) 命中 ♥");

        // emojiSymbols：**判据必须是引擎的 emoji 属性，不是码位形状** ——
        // ♥ U+2665 在 BMP 却是真 emoji，🞀 U+1F780 在补充平面却不是。
        // 宿主侧旧启发式「含代理对」在这两条上同时判反（2026-09-28 实测差 429 条：
        // 265 条假 emoji + 164 条漏判，正是本出口存在的理由）。
        String[] emojis = OpiEngine.emojiSymbols();
        java.util.Set<String> emojiSet = new java.util.HashSet<>();
        if (emojis != null) {
            java.util.Collections.addAll(emojiSet, emojis);
        }
        check(emojiSet.contains("♥"), "emojiSymbols 必须含 BMP 真 emoji ♥(U+2665)");
        check(!emojiSet.contains("🞀"), "emojiSymbols 不得含非 emoji 的 🞀(U+1F780)");

        // emoji ⊆ 全部：宿主侧 `emoji = all.filter(isEmoji)` 依赖这一条
        String[] allSyms = OpiEngine.searchSymbols("");
        java.util.Set<String> allSet = new java.util.HashSet<>();
        if (allSyms != null) {
            java.util.Collections.addAll(allSet, allSyms);
        }
        check(allSet.containsAll(emojiSet), "emojiSymbols 必须是 searchSymbols(\"\") 的子集");

        // loadTrad：空路径必须 false（引擎已装载，但坏路径不得动既有词库）
        check(!OpiEngine.loadTrad(""), "loadTrad(空) 应为 false");
        check(OpiEngine.mode() == 0, "loadTrad 失败不得改模式（此时应为 0/Pinyin）");

        // 用户词导入/删除：坏 JSON → 负数且不改动既有用户词
        check(OpiEngine.importUserWords("not json") < 0, "importUserWords(坏 JSON) 应为负数");
        check("{\"version\":1,\"words\":[]}".equals(OpiEngine.exportUserWords()),
                "导入失败后用户词不变");
        OpiEngine.removeUserWord("我"); // 词不存在 → 无操作，不得抛异常
        check("{\"version\":1,\"words\":[]}".equals(OpiEngine.exportUserWords()),
                "删除不存在的词后用户词不变");

        // 标点/全角/符号出口。**这几行是 JNI 侧唯一的函数体取证点**：
        // RegisterNatives 成功只证明「名字+描述符」对得上，不证明函数体对（把
        // `unwrap_or(false)` 写成 `unwrap_or(true)` 照样注册成功）。声明了却不调
        // = 那几条 native 永不被执行过。状态自理，别打乱上面的既有断言。
        check(OpiEngine.chinesePunct(), "chinesePunct 默认开");
        OpiEngine.setChinesePunct(false);
        check(!OpiEngine.chinesePunct(), "setChinesePunct(false) 生效");
        OpiEngine.setChinesePunct(true);
        check(OpiEngine.chinesePunct(), "setChinesePunct(true) 生效");
        // 触发键入口：返回值必须就是新状态，且与读侧一致（翻两次回到原值，
        // 供下面那条「switchMode 不重置本档」的断言用）
        boolean cp = OpiEngine.chinesePunct();
        check(OpiEngine.toggleChinesePunct() == !cp, "toggleChinesePunct 返回切换后的新状态");
        check(OpiEngine.chinesePunct() == !cp, "标点读侧与切换返回值一致");
        check(OpiEngine.toggleChinesePunct() == cp, "再翻一次回到原值");

        boolean fw = OpiEngine.fullwidthState();
        check(OpiEngine.toggleFullwidth() == !fw, "toggleFullwidth 返回切换后的新状态");
        check(OpiEngine.fullwidthState() == !fw, "全角读侧与切换返回值一致");
        OpiEngine.switchMode(1); // English 默认半角
        check(!OpiEngine.fullwidthState(), "switchMode(1) 把全角重置为半角");
        check(OpiEngine.chinesePunct(), "chinesePunct 不随 switchMode 重置（用户偏好）");
        OpiEngine.switchMode(0); // 回 Pinyin（全角默认值）
        check(OpiEngine.fullwidthState(), "switchMode(0) 回拼音的全角默认值");

        // toggleSymbol：空缓冲 → 空串，且切进 Symbol(3)；再切一次回 Pinyin
        check("".equals(OpiEngine.toggleSymbol()), "toggleSymbol 空缓冲返回空串");
        check(OpiEngine.mode() == 3, "toggleSymbol 后 mode==3(Symbol)");
        check("".equals(OpiEngine.toggleSymbol()), "再切一次仍返回空串");
        check(OpiEngine.mode() == 0, "再切一次回 Pinyin");

        if (failures == 0) {
            System.out.println("SMOKE-OK");
        } else {
            System.out.println("FAIL: " + failures + " checks failed");
            System.exit(1);
        }
    }
}
