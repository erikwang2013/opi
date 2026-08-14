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
    static native String inputSpace();
    static native String[] candidates(int limit);
    static native String buffer();
    static native int mode();
    static native String[] searchSymbols(String keyword);
    static native String symbolBlocks();
    static native String[] symbolsInBlock(short id);
    static native boolean learnerEnabled();
    static native void setLearner(boolean enabled);
    static native void clearUserWords();
    static native String exportUserWords();
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

        if (failures == 0) {
            System.out.println("SMOKE-OK");
        } else {
            System.out.println("FAIL: " + failures + " checks failed");
            System.exit(1);
        }
    }
}
