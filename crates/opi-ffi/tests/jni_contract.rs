// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **JNI 契约门禁**：`crates/opi-ffi/src/jni.rs` 的 `RegisterNatives` 表 ↔ 宿主类里
//! 手抄的 `native` 声明。
//!
//! 与 `c_abi_contract.rs` 同型、同一个病：**声明面手抄两遍，此前没有任何测试在看**。
//! 但 JNI 这一侧的失败方式比 C ABI 更急：C 的签名抄错是运行期静默 UB，JNI 的
//! 名字/描述符抄错是 **`System.load` 当场抛 `UnsatisfiedLinkError`** —— 而且
//! **整张表连带失败**：21 条里错 1 条，21 条全都装不上（本机实测，复现见文末注释）。
//!
//! 比对方式：两侧各折算成 JNI 描述符 `(参数...)返回` 后逐条比。Rust 侧读源码文本
//! （`jni_str!` 字面量），不依赖构建产物、不需要 JVM。
//!
//! 三个**非空转护栏**（本项目教训：全绿只证明被断言的部分）：
//!   1. 三份声明面各有一条 `*_parser_is_not_blind`：先钉锚点，且要求
//!      「解析出的条数 == 源码里声明标记的条数」—— 解析器漏读一条就等于门禁瞎一格。
//!   2. `checker_accepts_matching_fixture` 绿色基线：没有它，「有差异」这个断言在
//!      检查器常量出错（永远返回非空）时也会成立。**先证明绿不是瞎绿**。
//!   3. `checker_reports_*` 变异自检：故意改坏的 fixture 喂同一个检查器，断言它
//!      既报错、又**只**报那一条（`diffs.len() == 1` 卡住「乱报也算红」）。
//!
//! 覆盖边界（**诚实的漏检面**）：只比「注册表 ↔ 宿主类声明」，不比 JNI 函数体的语义、
//! 不比 `AndroidManifest`/ABI、不比 so 是否真被 `loadLibrary` 到。Rust 侧只认
//! `NativeMethod::from_raw_parts` 这一种写法 —— 换成 `Java_*` 命名导出会让解析器
//! 找不到任何条目，被 `rust_registration_parser_is_not_blind` 的计数护栏挡住。
//!
//! **名字/签名之外还有一层**：`switchMode(int)` 两侧都是 `(I)V`，把 Traditional 写成
//! `1` 照样全绿、只静默把「繁体」显示成「英文」。**整数参数的取值约定**单独一条门禁，
//! 见本文件末的 `mode_*` 一组与 `jni/mode.rs`。

#[path = "jni/checker.rs"]
mod checker;

#[path = "jni/mode.rs"]
mod mode;

use checker::*;
use mode::diffs_for;

// ---------- fixture：检查器的自检语料（**不读仓里的活文件**） ----------

const FIX_RS: &str = r#"
// 注释里的假声明不许被当真：NativeMethod::from_raw_parts(jni_str!("ghost"), …)
unsafe {
    [
        NativeMethod::from_raw_parts(
            jni_str!("alpha"),
            jni_str!("(Ljava/lang/String;)Z"),
            opijni_alpha as *mut c_void,
        ),
        NativeMethod::from_raw_parts(
            jni_str!("beta"),
            jni_str!("()V"),
            opijni_beta as *mut c_void,
        ),
        NativeMethod::from_raw_parts(
            jni_str!("gamma"),
            jni_str!("(IS)[Ljava/lang/String;"),
            opijni_gamma as *mut c_void,
        ),
    ]
};
"#;

const FIX_KT: &str = r#"
/** KDoc 里的示例也不许被当真：external fun ghost(): Int */
object Fake {
    external fun alpha(path: String?): Boolean
    override external fun beta()
    external fun gamma(id: Int, flag: Short): Array<String>?
}
"#;

/// 变异：`gamma` 的参数 `Short` → `Long`（描述符 `S`→`J`）。
const FIX_KT_PARAM_MUTANT: &str = r#"
object Fake {
    external fun alpha(path: String?): Boolean
    override external fun beta()
    external fun gamma(id: Int, flag: Long): Array<String>?
}
"#;

/// 变异：`alpha` 返回 `Boolean` → `Int`。
const FIX_KT_RET_MUTANT: &str = r#"
object Fake {
    external fun alpha(path: String?): Int
    override external fun beta()
    external fun gamma(id: Int, flag: Short): Array<String>?
}
"#;

/// 变异：删掉 `beta` 的声明（注册表里有、宿主类没有 = 实测的整表失败形态）。
const FIX_KT_MISSING: &str = r#"
object Fake {
    external fun alpha(path: String?): Boolean
    external fun gamma(id: Int, flag: Short): Array<String>?
}
"#;

/// 变异：宿主类多出一条注册表里没有的声明。
const FIX_KT_EXTRA: &str = r#"
object Fake {
    external fun alpha(path: String?): Boolean
    override external fun beta()
    external fun gamma(id: Int, flag: Short): Array<String>?
    external fun delta(): Int
}
"#;

/// 变异：名字拼错（`gamma` → `gamme`）：既缺一条又“多”一条，两边都要报。
const FIX_KT_TYPO: &str = r#"
object Fake {
    external fun alpha(path: String?): Boolean
    override external fun beta()
    external fun gamme(id: Int, flag: Short): Array<String>?
}
"#;

const FIX_JAVA: &str = r#"
final class Fake {
    static native boolean alpha(String path);
    static native void beta();
    static native String[] gamma(int id, short flag);
}
"#;

// ---------- 1. 护栏：三个解析器都不许失明 ----------

#[test]
fn rust_registration_parser_is_not_blind() {
    let src = read(JNI_RS);
    let reg = parse_rust_registrations(&src).expect("解析 jni.rs 注册表");

    // 锚点：抽出来的描述符必须逐字对，否则「解析成功但解析歪了」也会让后面的比对失真。
    for (name, desc) in [
        ("load", "(Ljava/lang/String;)Z"),
        ("candidates", "(I)[Ljava/lang/String;"),
        ("learnerEnabled", "()Z"),
        ("symbolsInBlock", "(S)[Ljava/lang/String;"),
    ] {
        let got = reg.iter().find(|m| m.name == name);
        assert_eq!(
            got.map(|m| m.desc.as_str()),
            Some(desc),
            "解析器没解析出锚点 `{name}` / `{desc}`（是解析器坏了，不是契约坏了）"
        );
    }

    // 计数护栏：`no_mangle` 的每条都应被登记，除 `JNI_OnLoad` 自己 ——
    // 漏读一条注册 = 门禁对那一条彻底失明。
    let no_mangle = count_no_mangle(&src);
    assert_eq!(
        no_mangle,
        reg.len() + 1,
        "#[unsafe(no_mangle)] 有 {no_mangle} 条，注册表解析出 {} 条（+JNI_OnLoad 应为 1:1）",
        reg.len()
    );
}

#[test]
fn kotlin_parser_is_not_blind() {
    let src = read(KOTLIN_HOST);
    let decls = parse_kotlin_externals(&src).expect("解析 OpiEngine.kt");
    for (name, desc) in [
        ("load", "(Ljava/lang/String;)Z"),
        ("inputSpace", "()Ljava/lang/String;"),
        ("symbolsInBlock", "(S)[Ljava/lang/String;"),
        ("removeUserWord", "(Ljava/lang/String;)V"),
        // 省略返回类型 = Unit = V（不是「解析不出返回类型」）
        ("clear", "()V"),
        // 可空数组 `Array<String>?` 与不可空同描述符
        ("candidates", "(I)[Ljava/lang/String;"),
    ] {
        let got = decls.iter().find(|m| m.name == name);
        assert_eq!(
            got.map(|m| m.desc.as_str()),
            Some(desc),
            "解析器没解析出锚点 `{name}` / `{desc}`"
        );
    }
}

#[test]
fn java_parser_is_not_blind() {
    // 活文件（不是 fixture）：这份 Java 宿主类在 README 里当着「JNI 连通性冒烟」发布，
    // 它的解析结果要用来判下面那条被 ignore 的用例。
    let src = read(JAVA_SMOKE);
    let decls = parse_java_natives(&src).expect("解析 Main.java");
    for (name, desc) in [
        ("load", "(Ljava/lang/String;)Z"),
        ("symbolsInBlock", "(S)[Ljava/lang/String;"),
        ("searchSymbols", "(Ljava/lang/String;)[Ljava/lang/String;"),
    ] {
        let got = decls.iter().find(|m| m.name == name);
        assert_eq!(
            got.map(|m| m.desc.as_str()),
            Some(desc),
            "解析器没解析出锚点 `{name}` / `{desc}`"
        );
    }
    assert!(!decls.is_empty(), "Main.java 里一条 native 都没解析出来");
}

// ---------- 2. 绿色基线 + 变异自检（检查器自己的红与绿） ----------

#[test]
fn checker_accepts_matching_fixture() {
    let reg = parse_rust_registrations(FIX_RS).expect("解析 fixture 注册表");
    let host = parse_kotlin_externals(FIX_KT).expect("解析 fixture Kotlin");
    assert_eq!(
        reg.len(),
        3,
        "fixture 注册表应有 3 条（注释里的 ghost 不算）"
    );
    assert_eq!(
        host.len(),
        3,
        "fixture Kotlin 应有 3 条（KDoc 里的 ghost 不算）"
    );
    assert_eq!(
        host[2],
        Method::new("gamma", "(IS)[Ljava/lang/String;"),
        "参数类型折算错（`Array<String>?` → `[Ljava/lang/String;`）"
    );
    let diffs = check_exact(&reg, &host, "fixture");
    assert!(
        diffs.is_empty(),
        "基线 fixture 必须无差异，否则「能红」是假的：\n  {}",
        report(diffs)
    );

    // 同一份声明写两遍语言的拼法（Kotlin `Array<String>?` / Java `String[]`）必须折算到
    // 同一个描述符 —— 否则 Java 宿主类那条门禁会永久假红/假绿。
    let java = parse_java_natives(FIX_JAVA).expect("解析 fixture Java");
    assert_eq!(java.len(), 3, "Java fixture 应有 3 条");
    assert_eq!(java, host, "Java 与 Kotlin 的同一组声明应折算成相同描述符");
}

#[test]
fn checker_reports_java_param_type_mutant() {
    let reg = parse_rust_registrations(FIX_RS).expect("解析 fixture 注册表");
    let java =
        parse_java_natives(&FIX_JAVA.replace("short flag", "long flag")).expect("解析变异体");
    let diffs = check_exact(&reg, &java, "fixture.java");
    assert_eq!(diffs.len(), 1, "应恰好报 1 条：{diffs:?}");
    assert!(diffs[0].contains("gamma"), "{}", diffs[0]);
}

/// 每个变异都要求「报错 **且只报那一条**」：只断言非空会放过「检查器乱报一通」。
fn one_diff(host_src: &str, needle: &str) -> String {
    let reg = parse_rust_registrations(FIX_RS).expect("解析 fixture 注册表");
    let host = parse_kotlin_externals(host_src).expect("解析变异体");
    let mut diffs = check_exact(&reg, &host, "fixture");
    assert_eq!(
        diffs.len(),
        1,
        "应恰好报 1 条差异，实际 {} 条：\n  {}",
        diffs.len(),
        report(diffs.clone())
    );
    assert!(
        diffs[0].contains(needle),
        "报的不是预期的 `{needle}`：{}",
        diffs[0]
    );
    diffs.remove(0)
}

#[test]
fn checker_reports_kotlin_param_type_mutant() {
    let d = one_diff(FIX_KT_PARAM_MUTANT, "gamma");
    assert!(
        d.contains("(IS)[Ljava/lang/String;"),
        "缺注册表侧描述符：{d}"
    );
    assert!(d.contains("(IJ)[Ljava/lang/String;"), "缺宿主侧描述符：{d}");
}

#[test]
fn checker_reports_kotlin_return_type_mutant() {
    one_diff(FIX_KT_RET_MUTANT, "alpha");
}

#[test]
fn checker_reports_missing_declaration() {
    one_diff(FIX_KT_MISSING, "beta");
}

#[test]
fn checker_reports_extra_declaration() {
    one_diff(FIX_KT_EXTRA, "delta");
}

#[test]
fn checker_reports_name_typo() {
    let reg = parse_rust_registrations(FIX_RS).expect("解析 fixture 注册表");
    let host = parse_kotlin_externals(FIX_KT_TYPO).expect("解析变异体");
    let diffs = check_exact(&reg, &host, "fixture");
    assert_eq!(diffs.len(), 2, "拼错名字应两个方向各报一条：{diffs:?}");
    // 注册了但宿主没声明（`gamma`）
    assert!(
        diffs
            .iter()
            .any(|d| d.contains("gamma") && d.contains("没有声明"))
    );
    // 宿主声明了但没注册（`gamme`）
    assert!(
        diffs
            .iter()
            .any(|d| d.contains("gamme") && d.contains("没注册"))
    );
}

#[test]
fn unknown_type_is_a_loud_error() {
    // 「认不出来就报错，不猜」：猜错 = 两侧折算成同一个错误值 = 假绿。
    assert!(desc_of("Foo").is_err(), "陌生类型必须报错");
    assert!(
        desc_of("Array<Int>").is_err(),
        "只支持 Array<String>，别的必须报错"
    );
    assert_eq!(desc_of("String?").unwrap(), "Ljava/lang/String;");
    assert_eq!(desc_of("String[]").unwrap(), "[Ljava/lang/String;");
    // Java 与 Kotlin 的拼写落在同一张表上
    assert_eq!(desc_of("boolean").unwrap(), desc_of("Boolean").unwrap());
    assert_eq!(desc_of("void").unwrap(), desc_of("Unit").unwrap());
}

// ---------- 3. 活文件门禁 ----------

/// `find_class` 找的类名必须与 Kotlin 宿主类的实际路径一致。
#[test]
fn kotlin_is_the_class_jni_onload_finds() {
    let want = parse_host_class(&read(JNI_RS)).expect("解析 find_class");
    let src = strip_comments(&read(KOTLIN_HOST));
    let pkg = src
        .lines()
        .find_map(|l| l.trim().strip_prefix("package "))
        .map(|p| p.trim())
        .expect("OpiEngine.kt 没有 package 声明");
    let obj = src
        .lines()
        .find_map(|l| l.trim().strip_prefix("object "))
        .map(|o| o.split([' ', ':']).next().unwrap_or(""))
        .expect("OpiEngine.kt 没有 object 声明");
    assert_eq!(
        want,
        format!("{}/{obj}", pkg.replace('.', "/")),
        "JNI_OnLoad 的 find_class 与宿主类对不上（改名只改了一边 = 装载时 NoClassDefFound）"
    );
}

#[test]
fn jni_registration_matches_kotlin() {
    let reg = parse_rust_registrations(&read(JNI_RS)).expect("解析 jni.rs 注册表");
    let host = parse_kotlin_externals(&read(KOTLIN_HOST)).expect("解析 OpiEngine.kt");
    assert!(!reg.is_empty() && !host.is_empty(), "有一侧解析成空 = 假绿");
    let diffs = check_exact(&reg, &host, "OpiEngine.kt");
    assert!(
        diffs.is_empty(),
        "JNI 注册表与 OpiEngine.kt 漂移（{} 条）。\n\
         ⚠️ 本仓实测：**注册表里错 1 条，整张表注册失败** → JNI_OnLoad 返 0 → \
         System.load 抛 UnsatisfiedLinkError（连没写错的那些也一起装不上）：\n  {}",
        diffs.len(),
        report(diffs)
    );
}

/// `android/jni_smoke/Main.java`：README 里当着「JNI 连通性冒烟」发布的宿主类。
///
/// **这条曾经是红的**（2026-09-27 实测）：当时它只声明 18 条、注册表 21 条 —— 缺
/// `loadTrad` / `removeUserWord` / `importUserWords`，于是**整表注册失败**（不是少三个
/// 方法，是一个都装不上），README 公开发布的那份冒烟**根本跑不起来**。复现（本机 Java 18）：
/// ```text
/// javac -d /tmp/smoke-out android/jni_smoke/Main.java
/// java -Dopi.so=target/release/libopi_ffi.so -cp /tmp/smoke-out xyz.erik.opi.jni.Main
/// → UnsatisfiedLinkError: unsupported JNI version 0x00000000 required by …/libopi_ffi.so
/// ```
/// （⚠️ 那句 "unsupported JNI version" 是**误导性**报错：真因是注册失败导致进程里没有
/// 可用的 JVM —— 追这句话会追错方向。补 1 条、2 条仍失败，补满 3 条才 `SMOKE-OK`。）
///
/// 2026-09-27 晚 `ime-tracks` 补到 21 条，本用例转绿并**摘掉 `#[ignore]` 变成常驻门禁**
/// （`#[ignore]` 当时是**待修**标记而不是免检）。它守的是「注册表里的每一条，冒烟宿主类
/// 都声明了」——`Main.java` 是**子集宿主**（只声明它调用到的），所以方向是单向包含，
/// 不是集合相等；多声明不报错，**少一条就红**（这正是整表失败的那个条件）。
#[test]
fn java_smoke_declares_every_registered_method() {
    let reg = parse_rust_registrations(&read(JNI_RS)).expect("解析 jni.rs 注册表");
    let smoke = parse_java_natives(&read(JAVA_SMOKE)).expect("解析 Main.java");
    assert!(!smoke.is_empty(), "Main.java 解析成空 = 假绿");
    let diffs = check_host_declares(&reg, &smoke, "jni_smoke/Main.java");
    assert!(
        diffs.is_empty(),
        "冒烟宿主类少了 {} 条声明 —— 整表注册会失败，System.load 直接抛异常：\n  {}",
        diffs.len(),
        report(diffs)
    );
}

// ---------- 4. 值契约：模式整数（名字/签名门禁对这里是全盲的） ----------

#[test]
fn mode_encoding_parsers_are_not_blind() {
    let (to_int, from_int) =
        mode::parse_rust_mode_maps(&mode::rust_src()).expect("解析 convert.rs");
    let kotlin = mode::parse_kotlin_mode_enum(&mode::kotlin_src()).expect("解析 EngineMode");

    // 陷阱值：`Mode` 的声明序是 Pinyin, **Traditional**, English, …，照它推 Traditional=1。
    // 所以锚点不能只钉「有 5 条」，必须钉死 Traditional 与 English 各是几。
    for (表, name, want) in [
        ("Rust", "TRADITIONAL", 4),
        ("Rust", "ENGLISH", 1),
        ("Kotlin", "TRADITIONAL", 4),
    ] {
        let got = if 表 == "Rust" {
            to_int.get(name)
        } else {
            kotlin.get(name)
        };
        assert_eq!(
            got,
            Some(&want),
            "{表} 侧 `{name}` 应为 {want}，实为 {got:?}（解析器失明，或契约真的漂了）"
        );
    }
    assert_eq!(to_int.len(), from_int.len(), "两个方向的条目数应相同");
    assert_eq!(to_int.len(), kotlin.len(), "两侧条目数应相同");
    assert!(
        to_int.len() >= 5,
        "只有 {} 条模式：解析器漏读了",
        to_int.len()
    );
}

#[test]
fn checker_accepts_matching_mode_fixture() {
    let diffs = diffs_for(mode::FIX_RS, mode::FIX_KT).expect("解析 fixture");
    assert!(
        diffs.is_empty(),
        "基线 fixture 必须无差异，否则「能红」是假的：\n  {}",
        report(diffs)
    );
}

#[test]
fn checker_reports_rust_declaration_order_mutant() {
    // 变异体是「照 `Mode` 声明序把**整张表**重写一遍」：Traditional/English/Number/Symbol
    // 四条全错 + 与反向表不自洽 = 5 条。这正是真出现时该报的量。
    let diffs = diffs_for(mode::FIX_RS_DECL_ORDER_MUTANT, mode::FIX_KT).expect("解析变异体");
    assert_eq!(diffs.len(), 5, "应报 4 条不一致 + 1 条不自洽：{diffs:?}");
    assert!(diffs.iter().any(|d| d.contains("不自洽")), "{diffs:?}");
    for mode in ["TRADITIONAL", "ENGLISH", "NUMBER", "SYMBOL"] {
        assert!(
            diffs.iter().any(|d| d.contains(mode)),
            "漏报了 `{mode}`：{diffs:?}"
        );
    }
}

#[test]
fn checker_reports_rust_from_int_swap_mutant() {
    // 只比一张表的话，这个变异是**隐形**的：to_int 全对，只有反向表把 1/4 对调了。
    let diffs = diffs_for(mode::FIX_RS_FROM_INT_SWAP, mode::FIX_KT).expect("解析变异体");
    assert_eq!(diffs.len(), 1, "应恰好报 1 条不自洽：{diffs:?}");
    assert!(diffs[0].contains("不自洽"), "{}", diffs[0]);
}

#[test]
fn checker_reports_kotlin_traditional_mutant() {
    let diffs = diffs_for(mode::FIX_RS, mode::FIX_KT_TRAD_MUTANT).expect("解析变异体");
    assert_eq!(diffs.len(), 1, "应恰好报 1 条：{diffs:?}");
    assert!(diffs[0].contains("TRADITIONAL"), "{}", diffs[0]);
}

#[test]
fn mode_encoding_matches_kotlin() {
    let diffs = diffs_for(&mode::rust_src(), &mode::kotlin_src()).expect("解析活文件");
    assert!(
        diffs.is_empty(),
        "`opi_switch_mode` 的取值约定在两侧漂移（{} 条）。\n\
         ⚠️ 签名是 `(I)V`，写错**不会编译失败**，只会静默把某个模式显示成另一个：\n  {}",
        diffs.len(),
        report(diffs)
    );
}
