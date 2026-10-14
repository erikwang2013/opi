// Host JVM smoke：不依赖 Android，直接 System.load libopi_ffi.so 验证
// JNI_OnLoad + RegisterNatives + 引擎主链路。与 Rust cabi_test 覆盖同一语义。
//
// 用法：
//   javac -d /tmp/smoke-out Main.java
//   java -Dopi.so=/path/to/libopi_ffi.so -cp /tmp/smoke-out xyz.erik.opi.jni.Main [/path/to/luna.opid]
// 成功打印 SMOKE-OK；失败打印 FAIL: <原因> 并退出码 1。
package xyz.erik.opi.jni;

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
    static native String[] candidateReadings();
    static native void pinReading(String reading);
    static native String buffer();
    static native String displayBuffer();
    static native int mode();
    static native String[] searchSymbols(String keyword);
    static native String[] emojiSymbols();
    static native String emojiGroups();
    static native String symbolBlocks();
    static native String[] symbolsInBlock(short id);
    static native boolean loadTrad(String path);
    static native boolean learnerEnabled();
    static native void setLearner(boolean enabled);
    static native boolean fuzzyEnabled();
    static native void setFuzzy(boolean on);
    static native int fuzzyPairs();
    static native void setFuzzyPairs(int mask);
    static native boolean correctionEnabled();
    static native void setCorrection(boolean on);
    static native boolean jianpinEnabled();
    static native void setJianpin(boolean on);
    static native int scheme();
    static native void setScheme(int scheme);
    static native int shuangpinLayout();
    static native void setShuangpinLayout(int index);
    static native void switchLanguage(int mode, int scheme);
    static native boolean loadStroke(String path);
    static native void clearUserWords();
    static native String exportUserWords();
    static native int importUserWords(String json);
    static native void removeUserWord(String text);
    static native boolean loadWubi(String path);
    static native int wubiStyle();
    static native void setWubiStyle(int style);
    static native int wubiMixedEntry();
    static native void setWubiMixedEntry(int entry);
    static native int wubiCandidateLimit();
    static native void setWubiCandidateLimit(int n);
    static native int wubiReturnStyle();
    static native int enterAction();
    static native void setWubiReturnStyle(int style);
    static native boolean wubiAutoCommit();
    static native void setWubiAutoCommit(boolean on);
    static native String wubiNextKeys();
    static native int importDict(String path);
    static native String dictList();
    static native boolean setDictEnabled(String name, boolean on);
    static native boolean removeDict(String name);
}

public final class Main {
    static int failures = 0;

    static void check(boolean ok, String what) {
        if (!ok) {
            failures++;
            System.out.println("FAIL: " + what);
        }
    }

    /** 候选数组里有没有这个字符串（candidates 可能返回 null）。 */
    static boolean has(String[] arr, String s) {
        if (arr == null) return false;
        for (String x : arr) {
            if (s.equals(x)) return true;
        }
        return false;
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

        // 「属性设置」的两个新开关（2026-10-08）：同样只做表面的读写回环。
        check(OpiEngine.fuzzyEnabled(), "fuzzy 默认开启");
        // 逐对掩码（2026-10-13）：**默认值必须在任何写操作之前读** —— `setFuzzy` 是
        // 掩码的包装（false ≡ 0），先写再读「默认」读到的是刚写进去的值。这条断言
        // 起草时排在 `setFuzzy(false)` 后面，红在 CI 的 jni_smoke（2026-10-14 修；
        // M1a/M1c 那两次 CI 都挂在 rustup 下载上，这个 job 直到今天才第一次跑到）。
        check(OpiEngine.fuzzyPairs() == 0xFF, "fuzzyPairs 默认全开（0xFF）");
        OpiEngine.setFuzzy(false);
        check(!OpiEngine.fuzzyEnabled(), "setFuzzy(false) 生效");
        check(OpiEngine.fuzzyPairs() == 0, "setFuzzy(false) 是掩码包装：全关 = 0");
        OpiEngine.setFuzzyPairs(0x41);
        check(OpiEngine.fuzzyPairs() == 0x41, "setFuzzyPairs(0x41) 原样读回");
        OpiEngine.setFuzzyPairs(0x141);
        check(OpiEngine.fuzzyPairs() == 0x41, "越界位（bit8）被钳掉");
        OpiEngine.setFuzzyPairs(0);
        // 拼音纠错（2026-10-13）：缺省开、关得掉、开得回。
        check(OpiEngine.correctionEnabled(), "correction 缺省开");
        OpiEngine.setCorrection(false);
        check(!OpiEngine.correctionEnabled(), "setCorrection(false) 生效");
        OpiEngine.setCorrection(true);
        OpiEngine.setFuzzy(true);
        check(OpiEngine.jianpinEnabled(), "jianpin 默认开启");
        OpiEngine.setJianpin(false);
        check(!OpiEngine.jianpinEnabled(), "setJianpin(false) 生效");
        OpiEngine.setJianpin(true);

        // 输入方案（2026-10-09）：整数回环 + 越界忽略 + 笔画词典坏路径哨兵。
        check(OpiEngine.scheme() == 0, "scheme 默认全键盘");
        OpiEngine.setScheme(1);
        check(OpiEngine.scheme() == 1, "setScheme(1) 生效（9 键）");
        OpiEngine.setScheme(99);
        check(OpiEngine.scheme() == 1, "越界值忽略（仍为 1）");
        OpiEngine.setScheme(0);
        check(OpiEngine.scheme() == 0, "换回全键盘");

        // 双拼布局（2026-10-13）：同一条整数回环 + 钳制的两个方向（与 C 面 consumer.c
        // 逐条对照，那是双面的行为契约）。「布局真的能解码」归 engine-core。
        check(OpiEngine.shuangpinLayout() == 0, "双拼布局默认自然码（0）");
        OpiEngine.setShuangpinLayout(5);
        check(OpiEngine.shuangpinLayout() == 5, "setShuangpinLayout(5) 生效（拼音加加）");
        OpiEngine.setShuangpinLayout(99);
        check(OpiEngine.shuangpinLayout() == 5, "过大钳到末位（不是钳 0、也不是忽略）");
        OpiEngine.setShuangpinLayout(-3);
        check(OpiEngine.shuangpinLayout() == 0, "负数钳到 0");
        check(!OpiEngine.loadStroke(""), "loadStroke(空) 应为 false");

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

        // emojiSymbols（2026-10-09 换源）：**浏览全量** —— UTS#51 emoji-test.txt 的
        // 完全限定条目（官方分组序，含旗帜/家庭/键帽等多码位序列），**不再是
        // 「符号表 ∩ emoji 集」的子集**。旧断言两条都是老契约：♥(U+2665) 只在符号表里
        // （新表收的是带 VS16 的 ❤️）；「⊆ searchSymbols("")」也不再成立 —— 浏览路与
        // 搜索路是两张表（symbols.tsv 的 emoji 列管单键候选反馈，见 symbols.rs 的
        // builtin_emoji_texts 注释）。
        String[] emojis = OpiEngine.emojiSymbols();
        int emojiCount = emojis == null ? 0 : emojis.length;
        // 条数钉死（与 engine-core/tests/emoji_coverage.rs 同一个数）：上游/生成器/宿主
        // 哪一边漂了这里先红。改上游版本时两处（+ 生成器自检）一起改。
        check(emojiCount == 3963, "emojiSymbols 条数应为 3963（实得 " + emojiCount + "）");
        java.util.Set<String> emojiSet = new java.util.HashSet<>();
        if (emojis != null) {
            java.util.Collections.addAll(emojiSet, emojis);
        }
        check(emojiSet.size() == emojiCount, "emojiSymbols 不得有重复");
        check(emojiCount > 0 && "😀".equals(emojis[0]), "首条 = 官方分组序的 grinning face");
        check(emojiSet.contains("😀"), "emojiSymbols 必须含 😀");
        check(emojiSet.contains("🇨🇳"), "emojiSymbols 必须含国旗序列 🇨🇳（区域指示符成对）");
        check(emojiSet.contains("❤️"), "emojiSymbols 必须含 VS16 形 ❤️（旧表只有裸 ♥）");
        check(!emojiSet.contains("🞀"), "emojiSymbols 不得含非 emoji 的 🞀(U+1F780)");

        // emojiGroups（2026-10-12）：表情面板分类条 —— `[{name,start,count}]`，纯静态表
        // （与引擎装载无关，未装载也有值）。判据：JSON 形状 + 9 组 + 首组「笑脸」start=0
        // + 各组 count 之和 == emojiSymbols 条数（下标必须对得上，点分类才滚得准）。
        String groups = OpiEngine.emojiGroups();
        check(groups != null && groups.contains("\"name\""), "emojiGroups 含 name");
        // ⚠️ 逐字段抓、**不假设键序**：serde_json 未开 preserve_order ⇒ 键是**字典序**
        // （实发 `{"count":…,"name":…,"start":…}`），按 `name,start,count` 的顺序写一个
        // 正则会把整段解析成 0 条（本冒烟第一版就是这么红的）。
        java.util.regex.Matcher obj = java.util.regex.Pattern.compile("\\{[^{}]*\\}")
                .matcher(groups == null ? "" : groups);
        java.util.regex.Pattern pName = java.util.regex.Pattern.compile("\"name\":\"([^\"]+)\"");
        java.util.regex.Pattern pStart = java.util.regex.Pattern.compile("\"start\":(\\d+)");
        java.util.regex.Pattern pCount = java.util.regex.Pattern.compile("\"count\":(\\d+)");
        int groupCount = 0;
        int groupSum = 0;
        int lastStart = 0;
        String firstGroup = "";
        while (obj.find()) {
            String s = obj.group();
            java.util.regex.Matcher mName = pName.matcher(s);
            java.util.regex.Matcher mStart = pStart.matcher(s);
            java.util.regex.Matcher mCount = pCount.matcher(s);
            if (!mName.find() || !mStart.find() || !mCount.find()) {
                continue;
            }
            if (groupCount == 0) {
                firstGroup = mName.group(1);
                lastStart = Integer.parseInt(mStart.group(1));
            }
            groupCount++;
            groupSum += Integer.parseInt(mCount.group(1));
        }
        check(groupCount == 9, "emojiGroups 应有 9 组（实得 " + groupCount + "）");
        check("笑脸".equals(firstGroup), "首组应为「笑脸」（官方组序；实得 " + firstGroup + "）");
        check(lastStart == 0, "首组的 start 应为 0（实得 " + lastStart + "）");
        check(groupSum == emojiCount, "各组 count 之和应 == emojiSymbols 条数（"
                + groupSum + " vs " + emojiCount + "）");

        // displayBuffer（2026-10-09）：显示用拼音读数，与 buffer 是**两个出口** ——
        // 9 键下 buffer 是数字串、displayBuffer 是带分词的读音；非 9 键两者相同。
        // 这里只冒烟：两者都拿得到、且非 9 键逐字节相同（9 键形态归 scheme_keys.rs 钉）。
        String buf = OpiEngine.buffer();
        String disp = OpiEngine.displayBuffer();
        check(buf != null && disp != null, "buffer/displayBuffer 都必须可读");
        check(buf.equals(disp), "非 9 键方案下 displayBuffer 必须与 buffer 相同（实得：" + buf + "/" + disp + "）");

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

        // candidateReadings（2026-10-09；2026-10-12 用户裁决「左列去重」）：候选拼音
        // 读数，**按读音去重保序** —— 9 键左列的数据源。与上面标点那组同理：声明了就
        // 必须真调，否则这条 native 永不被执行过。
        OpiEngine.setScheme(1); // 9 键（setScheme 清缓冲）
        OpiEngine.inputKey("6");
        OpiEngine.inputKey("4");
        String[] rc = OpiEngine.candidates(8);
        String[] rr = OpiEngine.candidateReadings();
        check(rc != null && rr != null, "candidates/candidateReadings 都必须可读");
        check(rc != null && rr != null && rr.length <= rc.length,
                "读数数 = 去重后的候选数（≤ 候选数；实得 "
                        + (rc == null ? -1 : rc.length) + "/" + (rr == null ? -1 : rr.length) + "）");
        if (rr != null) {
            // 去重（保序）断言：同一读音只许出现一次。
            java.util.Set<String> seen = new java.util.HashSet<>();
            for (String r : rr) {
                check(seen.add(r), "读数不得重复（实得 " + java.util.Arrays.toString(rr) + "）");
            }
        }
        if (rc != null && rr != null && rr.length > 0) {
            // 首条读数 = displayBuffer（同一对缓存的两个视图）。不断言具体词
            // （内置回退词库与真库候选不同），分词形态归 scheme_keys.rs 钉。
            check(rr[0].equals(OpiEngine.displayBuffer()),
                    "首条读数必须等于 displayBuffer（实得 " + rr[0] + "/" + OpiEngine.displayBuffer() + "）");
        }
        OpiEngine.setScheme(0); // 回全键盘

        // switchLanguage（2026-10-09 二期）：语言切换**保留缓冲** —— 中→英→中 往返后
        // 打到一半的拼音还在、候选能接着出。声明了就必须真调（同上一条规矩）。
        OpiEngine.setScheme(1); // 9 键
        OpiEngine.inputKey("6");
        OpiEngine.inputKey("4");
        OpiEngine.switchLanguage(1, 0); // 英文 + 全键盘
        check("64".equals(OpiEngine.buffer()), "切英文后缓冲必须保留（实得：" + OpiEngine.buffer() + "）");
        OpiEngine.switchLanguage(0, 1); // 拼音 + 9 键
        check("64".equals(OpiEngine.buffer()), "切回中文后缓冲仍在（实得：" + OpiEngine.buffer() + "）");
        String[] back = OpiEngine.candidates(8);
        check(back != null && back.length > 0, "切回后候选按同一缓冲重新算出");
        OpiEngine.setScheme(0); // 回全键盘

        // pinReading（2026-10-09）：钉读音 —— 候选**收窄**、**不上屏**；空串解除。
        OpiEngine.setScheme(1); // 9 键
        OpiEngine.inputKey("6");
        OpiEngine.inputKey("4");
        String[] wide = OpiEngine.candidates(64);
        int wideN = wide == null ? 0 : wide.length;
        check(wideN > 0, "前置：64 有候选");
        OpiEngine.pinReading("ni'hao"); // 带分词的界面原串，引擎归一
        String[] narrow = OpiEngine.candidates(64);
        int narrowN = narrow == null ? 0 : narrow.length;
        check(narrowN > 0 && narrowN <= wideN, "钉读音后候选收窄（" + wideN + " → " + narrowN + "）");
        check("64".equals(OpiEngine.buffer()), "钉读音不上屏、不动缓冲");
        OpiEngine.pinReading("");
        String[] unpinned = OpiEngine.candidates(64);
        check(unpinned != null && unpinned.length == wideN, "解除后回全量");
        OpiEngine.setScheme(0); // 回全键盘

        // 导入词库（2026-10-14，M3）：四条出口的**逐字段行为矩阵**归 opi-ffi 的
        // tests/dict_import.rs（C ABI 面）；这里做的是 JNI 面唯一能做的事 ——
        // 真调一次证明函数体被执行过（RegisterNatives 成功只证明名字+描述符对得上）。
        // ⚠️ 词条是「词<TAB>拼音<TAB>频次」，**词在前**（列序真源 engine-data 的
        // dict_text.rs：cols[0]=word；M3 任务书里的「拼音在前」是反的）。
        // 非 ASCII 一律写成 Unicode 转义：javac 按平台编码读源码，字面量直接写中文
        // 在非 UTF-8 的 locale 下会被读坏（转义在词法层处理，与编码无关）。
        // ⚠️ 连**注释里**都不能出现反斜杠加 u 的那两个字（javac 在词法前先扫转义，
        // 注释里的也一样 —— 本行起草时就因它红过:非法的 Unicode 转义）。
        final String WORD = "\u6D4B\u8BCD"; // 测词（U+6D4B U+8BCD）
        java.nio.file.Path dd = java.nio.file.Files.createTempDirectory("opi_jni_smoke_dict");
        java.nio.file.Path tsv = dd.resolve("smoke.tsv"); // 词库名 = 文件名词干 ⇒ 必须正好叫 smoke
        java.nio.file.Files.write(tsv,
            (WORD + "\tce\t999999999\n").getBytes(java.nio.charset.StandardCharsets.UTF_8));
        int dn = OpiEngine.importDict(tsv.toString());
        check(dn == 1, "importDict 返回条目数 1（实得 " + dn + "）");

        // 真进了引擎：打 ce 的候选里有它（limit 给 64 —— CI 拿的是完整 luna.opid，
        // ce 一族候选比本地回退库多得多；频次贴 u32 上限保证排得进）。
        OpiEngine.clear();
        OpiEngine.inputKey("c");
        OpiEngine.inputKey("e");
        check(has(OpiEngine.candidates(64), WORD), "导入层的词应出现在候选里");
        String dl = OpiEngine.dictList();
        check(dl != null && dl.contains("\"name\":\"smoke\""),
            "dictList 里应有一条 name=smoke（实得 " + dl + "）");
        check(dl != null && dl.contains("\"name\":\"luna\""), "dictList 里应含系统词典 luna");

        check(OpiEngine.setDictEnabled("smoke", false), "停用已导入的词库 → true");
        OpiEngine.clear();
        OpiEngine.inputKey("c");
        OpiEngine.inputKey("e");
        check(!has(OpiEngine.candidates(64), WORD), "停用后候选里不该再有它");
        check(OpiEngine.setDictEnabled("smoke", false), "已是目标态 → 幂等 true");
        check(!OpiEngine.setDictEnabled("luna", false), "系统词典不可停用 → false");

        // 数据留在 FFI 层内存：**先把文件删掉**再启用，照样打得出来
        // （宿主删文件与引擎状态解耦 —— 这是本族裁决 1 的 JNI 面执行点）。
        java.nio.file.Files.delete(tsv);
        check(OpiEngine.setDictEnabled("smoke", true), "原文件没了也要能再启用");
        OpiEngine.clear();
        OpiEngine.inputKey("c");
        OpiEngine.inputKey("e");
        check(has(OpiEngine.candidates(64), WORD), "再启用后候选里应有它");

        check(OpiEngine.importDict(tsv.toString()) == -1, "路径已不存在 → -1");
        check(OpiEngine.removeDict("smoke"), "removeDict → true");
        check(!OpiEngine.removeDict("smoke"), "已经不在 → false");
        check(!OpiEngine.removeDict("luna"), "系统词典 → false");
        check(!OpiEngine.setDictEnabled("smoke", true), "删掉之后名字认不得 → false");
        OpiEngine.clear();
        java.nio.file.Files.delete(dd); // 目录已空 ⇒ 可删（别把空壳留在 /tmp）

        // 五笔旋钮族（2026-10-14，M4 第二波）：缺省值 → 读写回环 → 越界语义 → 编码提示。
        // ⚠️ 真码表（.opid）不在本冒烟的可选参数里：`loadWubi` 只验**坏路径哨兵**
        //    （好路径那条由 C 消费者 run.sh 传 wubi86.opid 真跑，见 consumer_wubi.c）。
        check(!OpiEngine.loadWubi("/nonexistent/wubi86.opid"), "坏路径 → false");
        // 方案切到五笔（整数表 5；没装码表 ⇒ 如实降级、不假装有候选）。
        OpiEngine.setScheme(5);
        check(OpiEngine.scheme() == 5, "setScheme(5) 后应读到五笔");
        check(OpiEngine.wubiStyle() == 0, "风格缺省 = 纯五笔");
        check(OpiEngine.wubiMixedEntry() == 0, "混入方式缺省 = 直接字母");
        check(OpiEngine.wubiReturnStyle() == 0, "回车缺省 = 清空");
        check(OpiEngine.wubiAutoCommit(), "自动上屏缺省 = 开");
        check(OpiEngine.wubiCandidateLimit() == 5, "候选个数缺省 = 5");
        OpiEngine.setWubiStyle(1);
        check(OpiEngine.wubiStyle() == 1, "setWubiStyle(1) 生效");
        OpiEngine.setWubiStyle(99);
        check(OpiEngine.wubiStyle() == 1, "越界风格忽略");
        OpiEngine.setWubiStyle(0);
        OpiEngine.setWubiMixedEntry(1);
        check(OpiEngine.wubiMixedEntry() == 1, "setWubiMixedEntry(1) 生效");
        OpiEngine.setWubiMixedEntry(0);
        OpiEngine.setWubiReturnStyle(1);
        check(OpiEngine.wubiReturnStyle() == 1, "setWubiReturnStyle(1) 生效");
        OpiEngine.setWubiReturnStyle(0);
        OpiEngine.setWubiAutoCommit(false);
        check(!OpiEngine.wubiAutoCommit(), "关掉自动上屏");
        OpiEngine.setWubiAutoCommit(true);
        OpiEngine.setWubiCandidateLimit(99);
        check(OpiEngine.wubiCandidateLimit() == 9, "过大钳到 9");
        OpiEngine.setWubiCandidateLimit(0);
        check(OpiEngine.wubiCandidateLimit() == 1, "0/负数归 1");
        OpiEngine.setWubiCandidateLimit(5);
        check("".equals(OpiEngine.wubiNextKeys()), "空缓冲 → 无编码提示（空串）");
        // 回车判据（2026-10-14 接线）：空缓冲 0；打字后按设置位（行为，非只读设置位）。
        check(OpiEngine.enterAction() == 0, "空缓冲 → 回车判据 0（选首候选）");
        OpiEngine.inputKey("w");
        check(OpiEngine.enterAction() == 1, "五笔 + 缓冲非空 + 缺省档 → 1（清空）");
        OpiEngine.setWubiReturnStyle(1);
        check(OpiEngine.enterAction() == 2, "上屏原文档 → 2");
        OpiEngine.setWubiReturnStyle(0);
        OpiEngine.setScheme(0);
        check(OpiEngine.scheme() == 0, "切回全键盘");

        if (failures == 0) {
            System.out.println("SMOKE-OK");
        } else {
            System.out.println("FAIL: " + failures + " checks failed");
            System.exit(1);
        }
    }
}
