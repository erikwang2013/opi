# OPI 鸿蒙（HarmonyOS）端 —— **unsigned HAP 已构建，真机未验证**

> ## ✅ 2026-10-19 订正：**已能真构建（unsigned HAP）**
>
> 本文标题与下文「从未编译」是历史记录。`harmony/` 现在是完整 hvigor 工程，
> 本机 Command Line Tools 5.1.1.820 / API 19 上 `hvigorw assembleHap` **成功**，
> 产出 `entry/build/default/outputs/default/entry-default-unsigned.hap`
> （本机实测 debug 约 6.6 MB、release 约 6.4 MB；含 `libs/arm64-v8a/libopiime.so`）。
>
> 源码路径已迁移：`harmony/{ets,resources,cpp,module.json5}` →
> `harmony/entry/src/main/{ets,resources,cpp,module.json5}`；`pages/Index.ets`
> 移到 `ets/pages/Index.ets`（页面路径相对 `ets/` 根）。新旧对照、构建命令、
> 首轮 9 类错误与修复、签名/CI/未验证清单见 **`docs/install/harmony.md` §0**。
>
> **2026-10-21**：工具条/12 皮肤/键盘模式/平铺候选/符号/生僻字/表情/贴纸/设置持久化
> 已按 Android 继续对齐；release HAP 约 **49.4 MB**（含 `trad.opid` 与完整词库），
> 本机构建通过。仍无签名与真机运行证据。
>
> **2026-10-22**：工具条「关于」前新增齿轮设置页（`PanelView.Settings`）与更新检测
> （只 GET `https://opi.erik.xyz/version.json`，连接/读取各 4s；自动检测失败静默，
> 手动「检测更新」才显示原因）；`module.json5` 加 `ohos.permission.INTERNET`。
> 设置页只镜像本端**已有出口**的项；模糊音、纠错、简拼、双拼布局、五笔六键、
> 词库导入导出/清除仍未做（不做假开关）。
>
> ⚠️ 边界：**能构建 ≠ 能安装 ≠ 能用**。没有签名材料、没有设备；
> 真机安装/运行一条证据都没有。下文「最不确定的 API」仍然有效。

> ## ⚠️ 先读这段
>
> **ArkTS（`entry/src/main/ets/**`）已随 hvigor 工程真编译并打进 unsigned HAP**；
> 但真机安装、输入法列表与运行行为仍未验证。下面这段「一个字符都没有编译过」是历史记录。
>
> ⚠️ **2026-09-28 订正归因（原写作「本机没有 HarmonyOS SDK」，实测为假）**：
> **本机是有鸿蒙工具链的** —— `/home/component/command-line-tools/` 下装着
> Command Line Tools `5.1.1.820`、SDK `default/openharmony/{ets,js,native,toolchains,previewer}`
> （`oh-uni-package.json` 逐个读过：apiVersion **19** / 5.1.1.202）、以及 `hvigor` ·
> `codelinter` · `ohpm 5.1.4`。注意它是 **Command Line Tools，不是 DevEco Studio**（两个东西）。
>
> **真正的原因是本目录不是一个工程**：没有 `build-profile.json5`、没有 `hvigorfile.ts`、
> 没有工程级 `oh-package.json5`（判据：`find harmony/ -name 'build-profile.json5'` 零命中）。
> 实测 `codelinter harmony/ets` 报 **「The entered inspection path is incorrect, please make
> sure this path is under the project path.」** ⇒ **任何工具都处理不了它，连静态检查都不肯做。**
>
> ⇒ 结论（ArkTS 一个字符没被检查过）**仍然成立**，但原因是**缺工程脚手架**，不是机器缺 SDK。
> **补上脚手架之后，ArkTS 在这台机器上是能被真解析的**（行为级证据：`codelinter` 会
> 如实报出故意塞进去的语法错误 —— 但**注意退出码陷阱**：有缺陷时 EXIT 仍是 0，
> 布局不对时它**照样印** `No defects found`，判据只能是文本 + `checkedFiles` 里真有你的文件）。
> ⚠️ **「乃至被构建」没有任何证据** —— `hvigorw` / assembleHap 本机没测，别从这句推断。
> **2026-10-13 补**：这条本轮**真用上了** —— 按上面那份最小布局搭的探针工程跑过 `codelinter`，
> `ets/**` 全量 5 个文件（sha256 与仓库定稿逐文件相同）**零 parse 错**，且先跑了**两个正控**
> 证明它有分辨力（原样输出 + 三个坑见 **§5e ⑥**）。措辞照旧卡死：**它只到解析层** ——
> 无类型检查、无装饰器语义、连类型错都放过 ⇒ 本节第一句「一个字符都没有编译过」**仍然成立**。
> **2026-10-14 补**：ets 从 5 个文件长到 8 个（`EmojiPad` / `InputModes` / `KeyPanels` 新增，
> `pages/Index.ets` 重写），**用同一份探针重跑过**：`ets/**` 全量 8 个文件零 parse 错，
> `checkedFiles` 里 8 个一个不少、`module language is ets`；正控（往 `KeyPanels.ets` 删一个 `}`）
> 当场报 `@parsing-error` + `-Defects: 1`。**新加的负控**：把 `pages/Index.ets` 的命名导入
> 改成指向不存在的导出（`MODE_NO_SUCH_MODE`）—— **0 defects**，即它**也不解析命名导入**
> （与「不校验 `@kit.*` 说明符」是同一条：名字类错误它一条都拦不住）。
> **2026-10-14 二补**：9 键「123」键与 `pages/Index.ets` 的数字键面分支改完后（见 §5f ⑤）
> **又整跑了一轮**：同样 8 个文件逐文件 sha256 与仓库相同、正控照旧报 `@parsing-error`
> (`-Defects: 1`) + 真树 `No defects found` + `errorFileCount: 0`；两轮日志段按字节偏移
> 取（2336184→2357916 正控 / 2357916→2379629 真树）。**措辞仍卡在解析层**，一格没松。
> **2026-10-18 补**：`ets/**` 从 8 个文件长到 9 个 —— 新增机械生成的 `KaomojiData.ets`
> （颜文字 7 类 / 390 条，源 `data/raw/kaomoji.tsv`），`EmojiPad.ets` 重写为 Android
> 2026-10-18 布局（顶部 Emoji/颜文字 Tab、底部横滑分类条、自有/颜文字）。同一份探针
> 重跑：9 个文件 `checkedFiles` 一个不少、`module language is ets`、`errorFileCount: 0`；
> 探针里的 9 个文件与仓库逐文件 sha256 相同。正控：往探针副本的 `EmojiPad.ets` 头塞
> `const brokenSyntax: = ;` → `@parsing-error`（1:20）+ `-Defects: 1` / `errorFileCount: 1`。
> **仍然只到解析层**：无类型检查、无装饰器语义；`OpiPet.ets` 的 9 个形态绘制缺口见「已知缺口」14。
> **2026-10-18 二补**：`ets/**` 再从 9 个长到 11 个 —— 新增「自有」贴纸两件
> `PetStickerImages.ets`（12 张 280×280 PNG 的 base64 内嵌，生成物）与
> `PetStickerClipboard.ets`（base64 → `PixelMap` → 系统剪贴板）。同一份探针重跑
> （base64 已填好的那一版）：11 个文件 `checkedFiles` 一个不少、`module language is ets`、
> `errorFileCount: 0`，探针与仓库逐文件 sha256 相同。正控：往探针副本的
> `PetStickerImages.ets` 头塞 `const brokenSyntax: = ;` → `@parsing-error`（1:20）+
> `-Defects: 1` / `errorFileCount: 1`，随后恢复并 `cmp` 复核。
> **仍然只到解析层** —— base64 只是字符串字面量：图片能否被 `Image(dataUri)` 画出、
> `pasteboard` 的签名对不对，这条命令一概不知道（见「已知缺口」15）。
> **2026-10-19 三补**：`ets/**` 从 11 个长到 12 个 —— 新增生成物
> `PetEmojiFont.ets`（TTF/TSV 内嵌 + `font.registerFont`）与 `resources/base/rawfile/pet-font/`
> 三份字体副本；`PetStickerImages.ets` 增补 12 张 GIF，`EmojiPad` / `PetStickerClipboard` /
> `InputMethodService` 改成「GIF 优先、PNG 兜底 + 启动注册字体」（2026-10-20 起改为
> **PNG 优先**，见文件清单）。**这一轮的 12 个文件
> 尚未跑 codelinter**（上一轮的 11 个文件记录仍然成立），行为更未上机。
> **2026-10-20 四补（用户裁决）**：删除 `PetEmojiFont.ets` 与 `rawfile/pet-font/` 三份字体副本，
> 自有贴纸仍用原 GIF/PNG；`EmojiPad` 增加 `SHOW_OWN = false`，**「自有」子页缺省隐藏**
> （鸿蒙没有设置页，要显示改这一个常量）。`ets/**` 回到 11 个文件，**未重跑 codelinter**。
> **2026-10-20 五补（词库随包）**：新增 `DictionaryAssets.ets`（`ets/**` 共 12 个文件）——
> 启动时把 rawfile 里的 `luna.opid` / `stroke.opid` / `wubi86.opid` 拷进
> `ctx.filesDir`，再把**绝对路径**交给 C ABI；失败回退内置库并置 `dictionaryDegraded`
> （小欧会画 DEGRADED）。rawfile 里的词库由 `scripts/prepare_harmony_dicts.sh` 在
> 构建期从 `android/app/src/main/assets/` 复制，**复制品不入库**（见 `.gitignore`）。
> 本机 `hvigorw assembleHap -p buildMode=release` **构建通过**，HAP（约 48 MB）含三张
> 词库 + `libopiime.so` + `ets/modules.abc`；`codelinter` 报 0 defects。
> `trad.opid` 仍未接（`opi_load_trad` 未进桥，见已知缺口 5），`wubi98.opid` 按
> 「86/98 二选一」不随包。
> **这一轮也是 `ets/**` 第一次被 hvigor 的 `CompileArkTS`（真 ArkTS 编译器）编过** ——
> 下文「0 行编译过」是历史记录；真机行为仍全部未验证。
> **最小工程布局、两道墙（模块级 `build-profile.json5`、模块根下再一层 `src/`）、
> 正控与负控，全写在 [`docs/install/harmony.md`](../docs/install/harmony.md) 的 §1.1**
> —— **本行不复述那份清单**：它是一份实测的最小布局，抄到这里就多一份会漂的枚举。
> 这与原来那句
> 「做不到」是不同的处境。
>
> （本机是否有鸿蒙**设备或模拟器**，本行未核实，不要据本段推断。）
>
> **N-API 桥（`cpp/napi_bridge.c`）本机跑过语法/类型检查，也真编出过 `.o`**
> （见下方 §4、§4b）—— 但那用的是**宿主** clang + node 的真 N-API 头，
> **不是鸿蒙 NDK**；**从未链接成 `.so`、从未加载运行过**。
> 「过得了本机这两关」离「鸿蒙端能用」还差着 OHOS 工具链那一整段。
>
> **不要把这里当成「鸿蒙端已完成」。**
>
> 这份东西的定位是：**给鸿蒙开发者的起点 + 精确契约**。
> 它替你省掉「从零读 C ABI、从零搭输入法骨架、从零搞清 N-API 怎么注册」的时间，
> **不**替你做「代码能跑」这件事。
>
> 本项目已经被「没被编译器看过的代码」坑过两次，不要让它成为第三次：
> - **fcitx5 C++**：写在仓库里、README 记着「待验收」，一编译发现 7 处 API 误写
>   （连 `fcitx::InputMethod` 这个类都不存在）
> - **Windows TSF**：README 记着「完成，待在 Windows 上验收」，实测
>   `DllGetClassObject` 恒返 `CLASS_E_CLASSNOTAVAILABLE`、无 CLSID、
>   文本插入从未实现 → 整个端不可用
>
> **本目录里经过验证的东西，全部列在下方「实测记录」里**（2026-09-27 重跑并扩充）：
> Rust 侧三个鸿蒙目标 `cargo check` 通过、aarch64 真归档产出；`cpp/` 两个 C 文件
> 过了 clang 语法/类型检查**且是对着真 N-API 头**；`CMakeLists.txt` 首次被 CMake
> 配置通过、`napi_bridge.c` 首次被真编译成目标文件；`opi_ffi.h` 的**每一条**声明都与
> `cabi.rs` 的**签名**逐条核过；`index.d.ts` 与 N-API 注册表**机械核对一致**（名字一一对应）；
> 模式整数编码对着 `convert.rs` 核过。
>
> **但 `.ets` 那 12 个文件仍然是 0 行编译过，N-API 的注册与运行期语义仍全未验证。**
> **2026-10-20 订正**：`.ets` 已由 hvigor `CompileArkTS` 真编译通过（见头部五补）；
> 这句话只剩「N-API 注册与运行期语义未验证」仍然成立。
> **全目录没有一处可以称为「完成」或「已实现」—— 上面那些是「C 面对得上」，
> 不是「鸿蒙端能用」。**

---

## 在 DevEco 上第一件要做的事

**顺序不能反：先让编译过，一行逻辑都别改。** 不要一上来就改 UI、也不要先看功能对不对 ——
在这些代码从未过过编译器的情况下，「写得对不对」这个问题还没有意义。
**把编译错误当成草案的待办清单来读**：这份草案是尽力按记忆写的，但记忆一定有错，
编译器报的每一条都是在替你列 TODO。

```
第 1 步：建工程，把 cpp/ 与 ets/ 放进去，让它编译过
  用 DevEco 的 `New > Extension Ability > InputMethod` 生成一份最小输入法模板
  （生成的模板**才是权威**，本目录的 module.json5 是给你对答案用的片段，
   不要整个覆盖）。然后把：
     cpp/*            → entry/src/main/cpp/
     cpp/types/libopiime/ → entry/src/main/cpp/types/libopiime/
     ets/InputMethodExtensionAbility/* → entry/src/main/ets/InputMethodExtensionAbility/
     resources/base/profile/input_method_config.json → 同名位置
  并在 module.json5 里按 harmony/module.json5 的注释逐键补 extensionAbilities。
  → 目标：**编译通过**。此时功能一个都不能用，这是正常的。
  → 大概率要修一堆 API 名字与签名，见下方「最不确定的 API」。

第 2 步：补 main_pages.json 与 string.json
  resources/base/profile/main_pages.json 里加上 "pages/Index"
    （与 KeyboardController 的 setUiContent('pages/Index') **必须一致**，
     不一致的表现是「面板起来了但是一片白」）。
  resources/base/element/string.json 里补 input_method_config.json 用到的三个 key：
    subtype_pinyin_label / subtype_english_label / subtype_number_label
    缺了会让 profile 解析失败，可能表现为输入法直接不出现在系统列表里。
    推荐值（2026-10-09 起全端显示名统一为「小欧输入法」）：三个子键盘写
    `小欧输入法 拼音` / `小欧输入法 英文` / `小欧输入法 数字`；module.json5 的
    `MainAbility_label` 写 `小欧输入法`。
  （本仓库**故意没有**生成 string.json 与 main_pages.json —— 那是 DevEco
   生成的文件，覆盖会毁掉你已有的其它配置。）

第 3 步：按下方「构建集成」把 libopi_ffi.a 弄出来，让链接通过
  → 目标：链接通过 + `cabi.rs` 的**全部**符号都能解析（数量以
    `crates/opi-ffi/tests/c_abi_contract.rs` 门禁为准，别抄数字）；
    少一个的表现是「undefined reference to opi_xxx」。
  → **链接期要外部提供的符号，本机已经用 `nm -u` 列全了**（见 §4b）：
    桥引用的全部 `opi_*`（来自 libopi_ffi.a）+ **14 个 napi_***（来自 libace_napi.z.so）
    + malloc/free/memcpy。报 `undefined reference to napi_xxx` 时对着那 **14** 个名字查
    （这 14 个是稳定集，不会随导出扩容而变），就知道是鸿蒙那边的库没链上，
    不是 C 面写错了。
  → 至此，`import opiime from 'libopiime.so'` 应该不再是 undefined。
    **如果仍是 undefined，先查四个名字是否一致**（见 cpp/napi_bridge.c 顶部）。

第 4 步：在真机「设置」里启用这个输入法，然后把光标放进任意输入框
  → 到这一步才算「鸿蒙端能用」的第一天。
  → 第一件该验证的事：**打 `ni`，看候选栏出不出字**
    （`ni` 的期望值是 `你` 居首、共 1178 页 —— 完整词库在 2026-10-20 已随包；
    若启动时拷贝/装载失败会退回内置库，小欧画 DEGRADED，见下方「词库随包」）。

第 5 步之后才是：补用户词持久化、（rawfile 完整词库已于 2026-10-20 补）、
  补子类型切换（setSubtype）、调键盘高度。
  ⚠️ **2026-10-13 订正**：上面原文里的「补符号面板」**只完成了一半** ——
  Symbol 模式换成了**表情页**（分组条 + 网格 + 搜索框，见下方 §5e），
  但**符号面板本身仍是缺的**（`opi_symbol_blocks` / `opi_symbols_in_block`
  未桥，原来那 29 个 ASCII 符号也不再一键可达）。原文不改，留痕。
  ⚠️ **2026-10-18 更新**：表情页已改成 Android 同布局 —— 顶部
  `[‹ 返回][自有 | Emoji | 颜文字][🔍]`、**底部**可横滑分类条、Emoji = 引擎
  9 组、自有 = 12 张内嵌 PNG（点击复制图片到剪贴板，不插入文本）、内嵌颜文字
  数据（见 §5e 的 10-18 注）。上面「分组条 + 网格 + 搜索框」那句描述的是旧版，
  已被取代。
```

**不要跳过第 1 步去「顺手重构一下分层」。** 这份代码的分层是按
「Rust 侧是唯一逻辑真源」设计的：本目录**没有一行输入法逻辑**，
改之前先读 `crates/engine-core/src/router.rs` 与 `crates/engine-core/src/keys.rs`
的模块头注释（那里解释了键码为什么不能照抄 fcitx5 / TSF 轨）。

---

## 文件清单

| 文件 | 作用 |
|------|------|
| `cpp/opi_ffi.h` | C ABI 的 **use-site 声明面**（**只声明桥真正调用的那些**，**故意不抄全** —— 全量以 `cabi.rs` 为准）+ 键码常量 + 字符串所有权约定 |
| `cpp/napi_bridge.c` | **N-API 薄壳**：JS 字符串 ⇄ UTF-16 搬运、模块注册、释放 `OpiString`。模块导出与 C 函数**不是一样多**（一个 `Js*` 可能调多个 `opi_*`，且含 `opi_ffi_free_string`）—— 两个数不一样是正常的 |
| `cpp/CMakeLists.txt` | 把 Rust 静态库 + N-API 桥编成 `libopiime.so` |
| `cpp/types/libopiime/index.d.ts` | `libopiime.so` 的 ArkTS 类型声明，与 bridge 的 `desc[]` **机械核对一致**（名字一一对应；两边都不写条数） |
| `cpp/types/libopiime/oh-package.json5` | 把 `libopiime.so` 映射到上面的 `.d.ts` |
| `ets/InputMethodExtensionAbility/InputMethodService.ets` | `InputMethodExtensionAbility` 入口。**故意只做转发**，零逻辑 |
| `ets/InputMethodExtensionAbility/KeyboardController.ets` | 输入法会话唯一持有者：开面板、拿 `InputClient`、按键按 `action` 分派、状态推给 UI；表情数据的**缓存与去重**也在这（`emojiSymbols`/`emojiGroups`/`searchEmoji`）；输入模式四个转发（`selectInputMode`/`toggleModePanel`/`closeModePanel`/`retype`，2026-10-14） |
| `ets/InputMethodExtensionAbility/OpiEngine.ets` | C ABI 薄桥：枚举、**键码常量唯一定义处**、JSON 候选解码；表情数据三条（`emojiSymbols`/`emojiGroups`/`searchSymbols`）在文件末尾 |
| `ets/InputMethodExtensionAbility/OpiPet.ets` | 项目宠物「小欧」的 ArkTS 复刻（键帽精灵，表情跟引擎状态走） |
| `ets/InputMethodExtensionAbility/InputModes.ets` | **输入模式那件事的数字面**（2026-10-14）：方案/双拼布局/五笔六键的出口包装 + 九项 `INPUT_MODES` 表 + 模块级模式状态机（`selectInputMode` 等）。数字真源逐条注在文件头 |
| `ets/InputMethodExtensionAbility/DictionaryAssets.ets` | **词库随包**（2026-10-20）：rawfile → `ctx.filesDir` 的幂等拷贝（大小校验），返回沙箱**绝对路径**。失败返回空串（并打 `[opi]` 错误日志），由 `KeyboardController` 回退内置库并置 degraded |
| `ets/InputMethodExtensionAbility/KeyPanels.ets` | 9 键 / 大 9 键 / 14 键 / 笔画 四个键面 + 生僻字面板骨架 + 「输入模式」九宫格面板（2026-10-14）。与 Android 的三处差异（左列读数、底行、生僻字无数据）写在文件头 |
| `ets/InputMethodExtensionAbility/EmojiPad.ets` | 表情页（Symbol 模式）。2026-10-18 重写为与 Android `EmojiPanel.kt` 同布局：顶部 `[‹ 返回] [自有 \| Emoji \| 颜文字] [🔍]`、网格、**底部**可横滑分类条；Emoji 子页 = 引擎 9 组（**不再含「自有」偏移**），自有子页 = 4 列 GIF 网格（2026-10-19，点击复制图片）。**2026-10-20 起「自有」缺省隐藏**（`SHOW_OWN = false`；无设置页） |
| `ets/InputMethodExtensionAbility/PetStickerImages.ets` | 自有 12 张**小欧贴纸 PNG + 动画 GIF**（PNG 280×280 / GIF 200×200 十帧，base64 内嵌）+ 标签/文件名映射（顺序 = `PetEmojiMap.kt`，文件名 = `PetStickerAssets.kt`）。生成物，**别手改**；由 `scripts/gen_pet_stickers.py` 的产物再编码 |
| `ets/InputMethodExtensionAbility/PetStickerClipboard.ets` | 自有贴纸的落点：PNG → `PixelMap` → `pasteboard.createData(MIMETYPE_PIXELMAP)`（2026-10-20 起 PNG 优先），PNG 缺失才走 GIF 字节 → `createData('image/gif', ArrayBuffer)`；`promptAction` toast。**剪贴板回退**（IME Kit 不能往宿主输入框插图片），**不提交任何文本**。⚠️ release 构建已编译，API 行为未上机 |
| `ets/InputMethodExtensionAbility/KaomojiData.ets` | **机械生成**的颜文字数据（7 类 / 390 条，源 `data/raw/kaomoji.tsv`；解析/分类口径同 Android `KaomojiCategories.parse`）。**别手改** |
| `ets/InputMethodExtensionAbility/pages/Index.ets` | 键盘 UI 最小骨架：候选栏（候选带页内序号 + ⌨ 输入模式入口）+ preedit + 按键面**分支**（**数字键面按语言模式**、26 键族 QWERTY / 9 键 / 14 键 / 笔画 / 生僻字 / 表情页，2026-10-14）+ 功能行 |
| `module.json5` | `extensionAbilities` 配置**片段**（不是能直接用的完整 manifest） |
| `resources/base/profile/input_method_config.json` | 输入法子类型配置 |

**不写行数**：行数随 SPDX 头、注释增删而漂（本表曾普遍少 3 行），
与本仓库「不写会漂的数字」的规矩一致。上表即本目录的核心文件，未列进表的
`resources/base/media/icon.png` 等工程骨架文件不在此枚举；**本目录能验的都列在下方「实测记录」里**（Rust 侧目标编译 / C 侧
clang 与符号核对 / CMake 配置与真编译），**没列进去的一律是未验证**。

⚠️ **缺一个文件（有意不建）：`resources/base/profile/main_pages.json`。**
它必须存在，否则面板起来是一片白（见第 1 步）；但**不由本目录提供** ——
DevEco 工程自己生成的那份才是权威，补一份进来可能与已有配置冲突。
内容（`src` 里那条路径必须与 `KeyboardController.setUiContent('pages/Index')` 逐字一致）：

```json
{ "src": ["pages/Index"] }
```

### 宠物小欧的真源不是本目录

`OpiPet.ets` 是复刻，不是发明。几何与表情的规格在别处，改小欧请四处同步改：

| 文件 | 角色 |
|------|------|
| `docs/opi-pet.svg` | 静态基准：240×250 设计空间、锚点坐标、配色释义 |
| `shared/pet/OpiPet.kt` | 带情绪的 Compose 版（坐标已按情绪分化，**`OpiPet.ets` 照抄的是它**） |
| `android/app/src/test/kotlin/xyz/erik/opi/pet/OpiPetMoodTest.kt` | **`petMood()` 的验收规格** —— 鸿蒙端跑不了测试，这份 Kotlin 单测就是它的标准答案 |
| `harmony/ets/.../OpiPet.ets` | 本文件 |

`petMood()` 的四条分支顺序**不能改**（学习关闭优先于一切，连候选都不看）。

---

## 实测记录（唯一经过验证的部分）

### 1. Rust 侧为鸿蒙目标编译 —— **三个目标全部通过**

`cargo check` 不链接，所以 Linux 上不需要 OHOS SDK。实测原始输出：

```
$ rustup target add aarch64-unknown-linux-ohos armv7-unknown-linux-ohos x86_64-unknown-linux-ohos
（exit 0）

$ cargo check -p opi_ffi --target aarch64-unknown-linux-ohos
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.10s
exit=0

$ cargo check -p opi_ffi --target armv7-unknown-linux-ohos
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
exit=0

$ cargo check -p opi_ffi --target x86_64-unknown-linux-ohos
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.04s
exit=0
```

> 途中一次「失败」值得记一笔：首次跑 `x86_64-unknown-linux-ohos` 时报
> `error[E0463]: can't find crate for 'std'`，看着像代码问题，
> 实际是后台的 `rustup target add` 还没下完 —— 重跑即过。
> **判断「环境问题还是代码问题」时，要确认目标真的装上了** —— 但
> ⚠️ **别拿 `rustup target list --installed` 当判据**：它对**已装**的目标也会漏报。
> 2026-09-27 实测（`ffi-contract`）：`aarch64-apple-darwin` **不在**该列表里，
> `rustup target add` 也以 `detected conflict: libaddr2line-*.rlib` 失败，
> 但 `cargo rustc --target aarch64-apple-darwin --crate-type staticlib` **exit=0**、
> 归档正常产出 ⇒ **目标实际可用**。
>
> ⚠️ **但漏报不是全称的 —— 别把这条读成「本节的 ohos 目标也可能不在列表里」**：
> 2026-09-27 本机复测，本节那三个 ohos 目标 `rustup target list --installed` **三条全列**
> （`grep -c ohos` = 3），且与 `~/.rustup/toolchains/*/lib/rustlib/` 下的目录一一对应。
> 反方向（列表里有、目录却没有）实测**假阳性 0 条** ⇒ 这条命令是「**只漏报、不虚报**」。
> 两种读法都要留：**非空输出可以信，空输出不能当判据**。
>
> **可靠做法是拿它真跑一次**：`cargo check -p opi_ffi --target <目标>`。
>
> ⚠️ `macos/README.md` 里**本就写着**「别拿 `rustup target list` 的空输出当成『目标没装』」
> —— 而读了它的人（包括写它的人）仍然踩了。**写在文档里的警告不构成防线，重跑一条命令才是。**

### 2. 真产物：aarch64 静态库 —— **产出成功**

不只过了类型检查，静态库也真的产出来了（静态库归档不需要 OHOS 链接器）：

```
$ cargo rustc -p opi_ffi --target aarch64-unknown-linux-ohos --release --crate-type staticlib
    Finished `release` profile [optimized] target(s) in 20.24s
exit=0

$ ls -l target/aarch64-unknown-linux-ohos/release/libopi_ffi.a
-rw-rw-r-- 1 erik erik 9085656  9月27日 01:08 .../libopi_ffi.a

$ ar t libopi_ffi.a | wc -l
404
```

归档里的目标文件**确实是 AArch64**（不是拿宿主 x86_64 糊弄的）：

```
$ file opi_ffi-*.rcgu.o
ELF 64-bit LSB relocatable, ARM aarch64, version 1 (SYSV), with debug_info, not stripped

$ readelf -h opi_ffi-*.rcgu.o | grep -E '类别|系统架构'
类别:     ELF64
系统架构: AArch64
```

### 3. 导出符号 —— **三处数量一致（数量以门禁为准，别抄数字）**

> ⚠️ **2026-09-27 订正**：本节实测时 `libopi_ffi.a` / `cabi.rs` / `macos/OpiFFI.h` **三处都是 28**。
> 当日 `cabi.rs` 扩容到 **31**（新增 `opi_toggle_fullwidth` / `opi_fullwidth_state` /
> `opi_toggle_symbol`），**三处仍一致，但下面那个 `28` 已指向不存在的状态**。
> ⇒ 本节从此**不再把数字当常量**：**判据是
> `crates/opi-ffi/tests/c_abi_contract.rs` 的门禁**（它逐条比名字与签名），
> 数字只留在下面这个**带日期的实测块**里。
> 立此订正的由来：`cabi.rs` 的导出数已走过 19 → 20 → 22 → 28 → 31，**改数字只是把下一次留给了下一个人**。

```
$ nm -g --defined-only libopi_ffi.a | awk '$2 ~ /^[TtDdBbRr]$/ {print $3}' \
    | grep '^opi_' | grep -vx 'opi_ffi' | sort -u | wc -l
28          ← 2026-09-27 当时的值（现以门禁为准）
```

`opi_backspace` 到 `opi_symbols_in_block`，与 `crates/opi-ffi/src/cabi.rs`、
`macos/OpiFFI.h` 的数量**逐个数一致**。

### 4. 本目录的 C 代码 —— 过了 `clang -fsyntax-only`，**且这次是对着真 N-API 头**

**本节在 2026-09-27 被重跑并大幅升级。** 上一版这里是「手写 stub 头跑通的，
证明不了 N-API 签名」—— 那个保留**已经可以撤掉大半**：本机装了 node v22，
它的 `include/node/{node_api.h,js_native_api.h}` 就是 **N-API 规范的真头**
（N-API 是**稳定 ABI 规范**，OHOS 实现的是同一份规范）。

做法：在 `/tmp/opi-napi-shim/napi/native_api.h` 放一个**只有一行**
`#include <node_api.h>` 的垫片，把桥里的 `#include "napi/native_api.h"`
接到 node 的真头上去。垫片 **故意不放进仓库** —— 放进 `harmony/cpp/`
会 shadow 掉真 NDK 头，那比没有更危险。

```
$ clang -std=c11 -Wall -Wextra -Wpedantic -fsyntax-only \
    -I/tmp/opi-napi-shim -I/usr/local/node/include/node -I. \
    napi_bridge.c
exit=0                      # 零诊断

$ clang -std=c11 -Wall -Wextra -Wpedantic -fsyntax-only ... -x c opi_ffi.h
exit=0                      # 唯一输出是 -Wpragma-once-outside-header
                            # （把 .h 当主文件编的副产物，不是头的问题）

$ clang -std=c++17 -Wall -Wextra -fsyntax-only ... -x c++ opi_ffi.h
exit=0                      # extern "C" 卫哨可用
```

头文件确实被读到了（`clang -H` 的包含树，缩进即层级）：

```
. ./opi_ffi.h
. /tmp/opi-napi-shim/napi/native_api.h
... /usr/local/node/include/node/js_native_api.h
.... /usr/local/node/include/node/js_native_api_types.h
.. /usr/local/node/include/node/node_api.h
```

> **「零诊断」这次是验过有没有牙齿的。** 全绿本身不算证据 —— 得能红过。
> 另写一份负向对照 `/tmp/negctl.c`，把「最不确定的 API」里几条**故意写错**：
>
> ```
> $ clang -std=c11 -Wall -Wextra -fsyntax-only ... /tmp/negctl.c
> exit=1   （3 个 error）
> error: field designator 'nm_module_name' does not refer to any field in type
>        'napi_module'; did you mean 'nm_modname'?
> error: too few arguments to function call, expected 5, have 4
>        （napi_get_value_string_utf16）
> error: incompatible pointer to integer conversion passing 'size_t *' to
>        parameter of type 'size_t'   （bufsize 传成指针）
> ```
>
> 三条全报错 ⇒ 上面那个 exit=0 **不是空跑**：字段名写错、参数个数写错、
> 参数类型写错，这一关都拦得住。

**这次升级把这几条从「未验证」移走了：**

| 原先的不确定项 | 现在的状态 |
|---|---|
| U5 `napi/native_api.h` 的**函数声明位置** | 签名已对着真头核对通过（头**文件名**仍以 NDK 为准） |
| U6 `napi_module` 的**字段名与顺序** | 已核：`nm_version/nm_flags/nm_filename/nm_register_func/nm_modname/nm_priv/reserved[4]` 全对 |
| U8 的**参数个数与类型**（`napi_get_value_string_utf16` 五参、`bufsize` 是 `size_t` 不是指针） | 已核 |

**仍然未验证的（别把上面读成「能用」）：**
`napi_module_register` 那套注册方式鸿蒙认不认（U7，**运行时问题，语法检查看不出来**）；
`napi_create_string_utf16` 是否真拷贝（U9）；`bufsize` 是否含 NUL（U8 的**语义**部分，
规范说含，实现未验）；鸿蒙 NDK 自己的 `napi/native_api.h` 是否提供同名同签名符号
（本机拿不到它）。

### 4b. `CMakeLists.txt` + 真编译 —— **首次被 CMake 配置、首次编出目标文件**

上一版写的「**从未被 CMake 配置过**，连语法都没验」**已经不成立**：本机有 cmake 3.31.4。
在 `/tmp` 里照着 DevEco 的目录形状摆了一份（`cpp/` + 空占位 `libs/arm64-v8a/libopi_ffi.a`），
**仓库里没动一个文件**：

```
$ cmake -S cpp -B build-arch64 -DOHOS_ARCH=arm64-v8a \
      -DCMAKE_C_FLAGS="-I/tmp/opi-napi-shim -I/usr/local/node/include/node"
-- Configuring done (0.6s)
-- Generating done (0.0s)
exit=0                       # 配置期**全绿**

$ cmake --build build-arch64
[ 50%] Building C object CMakeFiles/opiime.dir/napi_bridge.c.o     # ← 编译成功
[100%] Linking C shared library libopiime.so
/usr/bin/ld: 找不到 -lace_napi.z: 没有那个文件或目录
/usr/bin/ld: 找不到 -lhilog_ndk.z: 没有那个文件或目录
exit=2
```

**这个失败正是预期的、也是好消息**：卡的是 `libace_napi.z.so` 与 `libhilog_ndk.z.so`
—— **鸿蒙专有的库**，本机当然没有。也就是说 CMake 这条链本身是通的：
`cmake_minimum_required` / `add_library` / `target_link_libraries` /
三条 `target_link_options`（`--gc-sections` `--allow-multiple-definition` `-s`）
全被接受，**编译步骤真的跑过**（宿主 gcc，非 OHOS 编译器），
只剩 OHOS 运行时库与 OHOS 链接器没接上。

配置期还报了一条**值得上报的**：

```
CMake Deprecation Warning at CMakeLists.txt:23 (cmake_minimum_required):
  Compatibility with CMake < 3.10 will be removed from a future version of CMake.
```

`cmake_minimum_required(VERSION 3.4.1)` 在 cmake ≥3.31 是**弃用警告**，
在 **CMake 4.0+ 是硬错误**。DevEco 自带的多半是老版本 cmake（碰不到），
但**换了新工具链的人会直接配置失败** —— 起来第一件事可以先把它抬到 3.10+。

**顺带拿到一份「桥到底要外部给什么」的完整清单**（`nm -u` 那个 `.o`，
这是**编译器产出的**，不是手抄的）：

```
$ nm -u napi_bridge.c.o | awk '{print $2}' | sort
共 36 个未定义符号 = 19 个 opi_* + 14 个 napi_* + malloc/free/memcpy

opi_*（19）：opi_buffer opi_candidates_page opi_clear opi_ffi_free_string
  opi_fullwidth_state opi_input_key opi_key_event opi_learner_enabled opi_load
  opi_mode opi_page opi_page_count opi_select_page opi_set_learner opi_set_shift
  opi_shift_state opi_switch_mode opi_toggle_fullwidth opi_toggle_symbol

napi_*（14）：napi_create_int32 napi_create_object napi_create_string_utf16
  napi_create_uint32 napi_define_properties napi_get_boolean napi_get_cb_info
  napi_get_undefined napi_get_value_bool napi_get_value_int32
  napi_get_value_string_utf16 napi_get_value_uint32 napi_module_register
  napi_set_named_property
```

（`memcpy` 是 2026-09-27 接 `opi_toggle_symbol` 之后**新出现的**，编译器为「按值返回
`OpiString` 结构体」生成的拷贝 —— 不是我们写的调用，但照样要 OHOS 的 libc 提供。
`free`/`malloc` 来自 `js_to_utf16` 的缓冲分配。）

两个**双向**断言（都过了，脚本在 `nm` 输出上跑 `comm`）：

| 断言 | 结果 |
|---|---|
| 桥引用的 `opi_*` ⊆ `opi_ffi.h` 的 19 条声明 | ✅ 无遗漏（少一条就是 `undefined reference`） |
| `opi_ffi.h` 的 19 条声明 ⊆ 桥引用的 `opi_*` | ✅ 无多余（没有死声明） |

> **这 14 个 `napi_*` 就是 DevEco 上的验收清单**：`libace_napi.z.so` 必须提供这 14 个
> 符号，少一个就是 `undefined reference to napi_xxx`。名字是编译器给的、不会再漂。

### 5. 声明与真符号的机械核对 —— **19/19 命中，0 落空**

我数着 `nm` 的输出，把 `cpp/opi_ffi.h` 的每条声明都对了一遍
（2026-09-27 D4 接线后重测，`libopi_ffi.a` 无缓存重建）：

```
declared in harmony/cpp/opi_ffi.h : 19
real exports in libopi_ffi.a      : 31
declared but NOT in archive       : （空）
not bridged                       : 12
```

19 条**被桥调用的 `opi_*` C 函数**已桥、其余未桥，两者相加 = `cabi.rs` 的**全部**导出
（数量以门禁为准，别把上面那个数当常量），**没有一条声明指向不存在的符号**。

⚠️ **未桥清单不在这里手抄** —— 手抄的清单会被当**工作项**读，而上面那句免责只盖住了数字：
本行原写死的「12」是 v1.2.0 的 31 − 19，**本轮新增三条中文标点导出后就已经不对**
（缺的正是 `opi_chinese_punct` / `opi_set_chinese_punct` / `opi_toggle_chinese_punct`）。
要今天的清单，跑这条（左边是 `cabi.rs` 的导出，右边是桥真调用的 `opi_*`，第二个集合的
求法与 §5b 那两条 `comm` 同一口径）：

```
$ comm -23 \
    <(grep -o 'pub unsafe extern "C" fn opi_[a-z_0-9]*' crates/opi-ffi/src/cabi.rs \
        | sed 's/.*fn //' | sort -u) \
    <(grep -oE '\bopi_[a-z_0-9]+\s*\(' harmony/cpp/napi_bridge.c \
        | sed 's/[[:space:]]*($//' | sort -u)
```

清单里是符号面板、用户词导入导出、繁体词库、中文标点开关这些**能力已在 `cabi.rs`、
桥还没挂**的出口。要做哪个，就去 `cabi.rs` 抄那条声明、加进 `opi_ffi.h`、在 bridge 里挂上 ——
⚠️ **但加声明必须同时在桥里加一个真调用**：`c_abi_contract` 的
`harmony_header_is_consistent_use_site_subset` 断言 `opi_ffi.h` 是**双向** use-site 子集
（每条声明都同型 **且** 每条声明都真的被桥调用），只加声明不加调用会红那一条。

> 本次接线**没有从这份清单里拿走任何一条** —— 新桥的三条（全角/符号开关）本来就不在里面，
> 它们是同日新加的导出。所以「未桥数」**那次是巧合相等**，不是不变量 ——
> 这正是上面把清单换成 `comm` 命令的原因（写死的清单会把「巧合」固化成「事实」）。

> **订正注记（2026-10-13，表情页接线）**：本节标题的 19/19 是**写下当时**的数。
> 本轮桥里加了 3 条 `opi_*` 调用（`opi_emoji_symbols` / `opi_emoji_groups` /
> `opi_search_symbols`），**且这回第一次从「未桥清单」里真的拿走了一条** ——
> `opi_search_symbols` 原先正是缺口 3 点名的那一条（`opi_symbol_blocks` /
> `opi_symbols_in_block` 仍未桥）。实测输出与红/绿判据见 **§5e**，原文不改，留痕。
>
> ⚠️ 本轮还撞上了本节那条规矩的一个实例：**新声明必须与桥里的真调用同一批落地**。
> 那三条声明在 `cpp/opi_ffi.h`（**cabi 名下、本端不改**），本轮由 cabi 补 ——
> 在他落地之前，桥里的调用会让 `clang -fsyntax-only` 报 `implicit-function-declaration`
> （**三处，正好对应三条声明**）。这个红的成因是**跨文件的先后**，不是 C 面写错了，
> 别照着报错去改桥。
>
> **收尾（2026-10-13 02:33）**：cabi 已落（他补在「符号 / 表情」那段，三行与 `macos/OpiFFI.h`
> 逐字相同）。中间两边**撞过一次**（文件里一度两份同名声明，已去掉多余的那份）。
> 去重后：`clang` 对真头 **exit=0**、双向 `comm` **两侧皆空**、`c_abi_contract` **14/14**。
> 逐条讨论在原样输出里 —— 见 **§5e** 的 ③/④/⑤ 三段。

**2026-09-27 补：签名也核过了（不只是名字）。** 上一条保留写的是「签名没有核对」——
现在每一条都逐条对过 `cabi.rs`：参数类型、返回值、`#[repr(C)]` 结构体字段顺序全部一致
（下表含同日新加的三条）：

| opi_ffi.h | cabi.rs | |
|---|---|---|
| `void opi_ffi_free_string(OpiString s)` | `fn opi_ffi_free_string(s: OpiString)` | ✓ |
| `bool opi_load(const uint16_t*, size_t)` | `fn opi_load(*const u16, usize) -> bool` | ✓ |
| `OpiString opi_input_key(const uint16_t*, size_t)` | `fn opi_input_key(*const u16, usize) -> OpiString` | ✓ |
| `void opi_clear(void)` | `fn opi_clear()` | ✓ |
| `OpiString opi_select_page(uint32_t)` | `fn opi_select_page(u32) -> OpiString` | ✓ |
| `OpiString opi_candidates_page(void)` | `fn opi_candidates_page() -> OpiString` | ✓ |
| `OpiString opi_buffer(void)` | `fn opi_buffer() -> OpiString` | ✓ |
| `uint32_t opi_page_count(void)` | `fn opi_page_count() -> u32` | ✓ |
| `uint32_t opi_page(void)` | `fn opi_page() -> u32` | ✓ |
| `int32_t opi_mode(void)` | `fn opi_mode() -> i32` | ✓ |
| `void opi_switch_mode(int32_t)` | `fn opi_switch_mode(i32)` | ✓ |
| `int32_t opi_shift_state(void)` | `fn opi_shift_state() -> i32` | ✓ |
| `void opi_set_shift(bool)` | `fn opi_set_shift(bool)` | ✓ |
| `bool opi_learner_enabled(void)` | `fn opi_learner_enabled() -> bool` | ✓ |
| `void opi_set_learner(bool)` | `fn opi_set_learner(bool)` | ✓ |
| `OpiKeyEventResult opi_key_event(uint32_t, uint32_t)` | `fn opi_key_event(u32, u32) -> OpiKeyEventResult` | ✓ |
| `bool opi_toggle_fullwidth(void)` | `fn opi_toggle_fullwidth() -> bool` | ✓ |
| `bool opi_fullwidth_state(void)` | `fn opi_fullwidth_state() -> bool` | ✓ |
| `OpiString opi_toggle_symbol(void)` | `fn opi_toggle_symbol() -> OpiString` | ✓ |

`OpiString{ptr,len}` 与 `OpiKeyEventResult{action,text}` 两个 `#[repr(C)]` 结构体的
字段顺序也逐字一致。**这一条仍然不等于「能链接上」**（那要 `libopi_ffi.a` +
OHOS 链接器，本机没有）。

### 5b. `index.d.ts` 与 N-API 注册表 —— **18/18 逐条一致**

`cpp/types/libopiime/index.d.ts` 是 ArkTS 侧的类型声明面，**必须**与
`napi_bridge.c` 里 `napi_property_descriptor desc[]` 那张注册表逐条对齐
（少一个 = 运行期 `xxx is not a function`；多一个 = 到真机上打到那条路径才炸）。

> ⚠️ **两个「鸿蒙桥 N 条」不是一回事，引用时必须说清是哪个 N**：
> 本节的 **N** 数的是 `desc[]` 里的 **JS 描述符**（`JsXxx`，ArkTS 能调到的名字）；
> 上面 §5 那个 **N** 数的是桥**调用的 `opi_*` C 函数**（来自 `libopi_ffi.a`）。
> 两者数量不相等是正常的，不是漂移 —— C 侧含 `opi_ffi_free_string` 这种没有 JS 描述符的
> 辅助出口，且一个 `JsXxx` 也可能调多个 `opi_*`。同一句提醒也写在
> `napi_bridge.c` 顶部（那段还给了 `diff` 的跑法，且**两边都不写条数**）。

> **订正注记（2026-10-13，表情页接线）**：本节原文写的是 18/18 —— 那是**写下当时**的数。
> 本轮桥里加了 3 条（`emojiSymbols` / `emojiGroups` / `searchSymbols`，见 §5e），
> 实测 §标题那个数已经过期；**确数不写在这里**（它随出口增删而漂），跑下面第一条 `diff`：
> 2026-10-13 实测输出 `IDENTICAL`。原文不改，留痕。

机械核对（无输出 = 一致）：

```
$ diff <(bridge desc[] 里的 18 个名字) <(index.d.ts 的 18 个 export const 名)
IDENTICAL           # 两边各 18 条，名字一一对应

$ comm -23 <(桥实际调用的 opi_* 函数) <(opi_ffi.h 声明的 opi_*)
（空）              # 桥调用的 19 个全都有声明
$ comm -13 <(桥实际调用的 opi_* 函数) <(opi_ffi.h 声明的 opi_*)
（空）              # 声明的 19 个全部被调用 —— 没有多余声明
```

> 上面两条 `comm` **不是**我自己写的脚本说了算 —— 项目自己的门禁
> `crates/opi-ffi/tests/c_abi_contract.rs` 的 `harmony_header_is_consistent_use_site_subset`
> 就断言这两件事（每条声明都同型 + 每条声明都真的被桥调用），
> **它是别人写的机械**。2026-09-27 跑：`14 passed; 0 failed`，exit 0。

> ⚠️ 但 `.d.ts` 这一条核的是**名字**。`.d.ts` 是手写的，**没有 tsc 编译过**
> （ArkTS 的 `.d.ts` 解析与 TypeScript 并不完全等同），**返回类型对不对仍属于
> 「第 1 步编译要抓的东西」**。注册表那一侧则已被 §4 的 clang 检查覆盖。

### 5c. 模式整数编码 —— **已对着 `convert.rs` 核过**

`opi_switch_mode` / `opi_mode` 的编码是**本仓的静默失败陷阱**：
它**不等于** `Mode` 枚举的声明序（声明序是 `Pinyin, Traditional, English, Number, Symbol`
—— 照声明序推会得到 `Traditional = 1`）。真源在 `crates/opi-ffi/src/api/convert.rs`：

```
$ grep -A8 'fn mode_to_int' crates/opi-ffi/src/api/convert.rs
Pinyin => 0, English => 1, Number => 2, Symbol => 3, Traditional => 4
```

`harmony/ets/.../OpiEngine.ets` 的 `OpiMode` 枚举与 `toMode()` 的映射**逐条一致**，
`pages/Index.ets` 的 `modeLabel()/nextMode()/letterRows()` 也按同一套数字走。✓
（`OpiEngine.switchMode` 还会先把值夹紧到 0..4 —— 因为 `opi_switch_mode`
对越界值是**静默不动作**，没有返回值可判。）

### 5d. 全角 / 符号开关接线（2026-09-27）—— **三条新出口 + 四条契约**

引擎里 `Engine::toggle_fullwidth` / `fullwidth` / `toggle_symbol` **一直都在**，
但直到 2026-09-27 才进 C ABI —— 在此之前 `grep -rn 'fullwidth\|toggle_symbol'
crates/opi-ffi/src/` 是**空的**，也就是说**任何客户端（含安卓）都调不到**。
本轮把这三条接进桥：

| 层 | 本轮改动 |
|---|---|
| `cpp/opi_ffi.h` | +3 条声明（use-site 子集 16 → 19） |
| `cpp/napi_bridge.c` | +3 个 `Js*` 函数、`desc[]` +3 条（15 → 18） |
| `cpp/types/libopiime/index.d.ts` | +3 条声明（15 → 18） |
| `ets/.../OpiEngine.ets` | +3 个类型化包装（`toggleFullwidth` / `fullwidthState` / `toggleSymbol`） |

**四条契约**（写进了上面四处的注释，改之前先读）：

1. **只有两个出口会在调用后改变全角：`opi_switch_mode` 与 `opi_toggle_symbol`。**
   后者最易漏：它内部走了一次 `switch_mode`，而 Symbol 的默认是**半角**
   ⇒ 按符号键时全角指示**悄悄灭掉**。
2. **调完 `toggle_symbol` 要重读四样**：mode / buffer / candidates / fullwidth。
3. **不许自己推导映射**：`'` 在缓冲非空时是**音节分隔符**（`xi'an`）
   ⇒ 全角映射**不是 `(mode, fullwidth)` 的纯函数**。`fullwidthState()` 只喂状态栏。
4. **`toggle_symbol` 返回的不是「刚切出来的那个符号」**，是**切模式前那截缓冲的待提交文本**
   （有候选 → 首候选；乱码缓冲如 `zzz` → 清掉且**不上屏**）。空串 = 无提交；
   **非空必须上屏**。拿不到「插入文本」通道的端不要调它。

**为什么单独记一段**：第 1、2 条是**静默失效**型（不报错、不崩溃，指示/状态就是错的），
而第 1 条曾经以「只在 `opi_switch_mode` 之后重读」的形式**写进过契约草案**——
是在核对时发现漏了 `toggle_symbol` 这条路径才补上的。写成「只在 switch_mode 之后」
的客户端**不会重读符号键那一下**。

**负向对照（这次验了两层，都不只是「跑绿了」）**：

```
# 第一层：clang 层面 —— 新加的三条真的进了类型检查
$ clang -std=c11 -Wall -Wextra -fsyntax-only -I/tmp/opi-napi-shim \
      -I/usr/local/node/include/node -I. /tmp/negctl3.c
exit=1（3 errors）
  error: too many arguments to function call, expected 0, have 1
         ./opi_ffi.h:136:6: note: 'opi_toggle_fullwidth' declared here
  error: initializing 'bool' with an expression of incompatible type 'OpiString'
  error: initializing 'OpiString' with an expression of incompatible type 'bool'

# 第二层：项目门禁层面 —— 它抓得到「声明了但没调用」
#   临时去掉 opi_toggle_symbol 的调用（声明保留），跑门禁：
$ cargo test -p opi_ffi --test c_abi_contract
test harmony_header_is_consistent_use_site_subset ... FAILED
  `opi_toggle_symbol` 在 harmony/cpp/opi_ffi.h 里声明了，但 napi_bridge.c 从未调用
test result: FAILED. 13 passed; 1 failed        # 只红这一条，其余 13 条不受影响
#   恢复后重跑 → 14 passed; 0 failed，exit 0
```

⇒ 两层一起说明**上面那些 `exit=0` 不是空跑**：类型写错会被拦，少调用也会被拦。

> ⚠️ 本轮**没有**做的事：**符号面板**（能提交字面符号的 UI）仍然没有 ——
> 见「已知缺口」第 3 条。`toggleSymbol()` 现在**可以从 ArkTS 调到**了，
> 但**没有键绑它**，所以用户仍然按不出来。这与 §5 那条 `comm` 打出来的未桥清单是同一类：
> 能力已到桥，UI 未接。

**仍未验证**：这三条与 §6 的其余条目**同档** —— ETS 侧那三个包装函数
**一行都没编译过**，`toggleSymbol` 返回的字符串**没有真的上屏过**，
全角指示**没有真的显示过**。

### 5e. 表情页接线（2026-10-13）—— **三条新出口：桥已挂、UI 已接**

引擎侧同日长出的**表情页数据面**三条（`crates/opi-ffi/src/cabi_symbols.rs`，
语义与 JNI 面的 `opijni_emoji_symbols` / `opijni_emoji_groups` / `opijni_search_symbols`
逐条一致、共享 `api::` 的同一份实现）。本端接的是「分组条 + 网格 + 搜索框」那一页
（**2026-10-14 起是 `EmojiPad.ets` 这个组件**，原为 `pages/Index.ets` 的 `EmojiPage`），
**没有新路由、没有新面板** —— 走的是 `build()` 里
`this.mode === OpiMode.Symbol` 那个分支，与 `letterRows()` 换键行是**同一处判据**。

| C 出口（`cabi_symbols.rs`） | 桥里 | ArkTS 包装 | 空语义 |
|---|---|---|---|
| `OpiString opi_emoji_symbols(void)` | `JsEmojiSymbols` | `OpiEngine.emojiSymbols()` | **不经引擎单例**：未 `load` 也返回全量 ⇒ 空数组只意味真出错 |
| `OpiString opi_emoji_groups(void)` | `JsEmojiGroups` | `OpiEngine.emojiGroups()` | 同上（**与 `opi_symbol_blocks` 未装载回空串相反**） |
| `OpiString opi_search_symbols(const uint16_t *, size_t)` | `JsSearchSymbols` | `OpiEngine.searchSymbols()` | **走引擎单例**：未装载 → `[]`（与上面两条相反） |

⚠️ **三种「空」的语义两两不同**（前两条不退化、第三条退化、`candidatesPage` 也退化）。
三条的 ArkTS 注释里各写了一遍 —— 混用会把「引擎没起来」读成「表情库是空的」，
而这两种情况该做的事完全不同（一个要等 `inputStart`，一个要查模块）。

⚠️ 搜索那条出口返回的是**符号表**的匹配条目、**不是表情集**，表情页自己与
`emojiSymbols()` 求交（`KeyboardController.searchEmoji`）—— 与 Android
`SymbolPanel` 的 `results.filter(catalog::isEmoji)` 是同一件事、同一层。

> **2026-10-18 布局更新**：本节开头写的「分组条 + 网格 + 搜索框」是 2026-10-13 的旧版。
> 现 `EmojiPad.ets` 与 Android `EmojiPanel.kt` 的 2026-10-18 布局对齐：顶部
> `[‹ 返回][自有 | Emoji | 颜文字][🔍]`，网格下方**一条可横滑分类条**。
> **`自有` 是独立顶级子页**（用户裁决，不再混进 Emoji 分类）：4 列网格画
> `PetStickerImages.ets` 内嵌的 12 张真实小欧 GIF（2026-10-19；缺失退回 PNG）；
> 点击**复制 PNG/PixelMap（GIF 仅在 PNG 缺失时用）到系统剪贴板**（`PetStickerClipboard.ets`）+ toast，
> **不提交任何文本**，该子页没有分类条也没有 🔍。**2026-10-20 起该子页缺省隐藏**
> （`EmojiPad.ets` 的 `SHOW_OWN = false`；鸿蒙无设置页，要显示改常量并重编）。
> **Emoji 子页只用引擎 `emojiGroups()` 的 9 组**（删掉了旧版的「自有」分类与整体偏移）；
> 颜文字来自 `KaomojiData.ets`（7 类 390 条，机械生成）。
> 分类点击采取**切片**而不是 `scrollToItem`（网格只画当前组），所以高亮 = 当前组；
> 这是本机查不到 `Scroller` API 时的取舍，不是 Android 的滚动语义。搜索出口、去重、
> 「无匹配」宠物仍沿用本节下文，搜索只覆盖 Emoji / 颜文字。

**本机跑过的（原样输出）：**

```
# ① 注册表 ↔ 手抄 .d.ts 的名字集合（§5b 那条 diff）
$ diff <(grep -o '{"[a-zA-Z]*"' cpp/napi_bridge.c | tr -d '{"' | sort) \
       <(grep -o '^export const [a-zA-Z]*' cpp/types/libopiime/index.d.ts | awk '{print $3}' | sort)
IDENTICAL                      # 两边名字集合相等（条数两边都不写，见 §5b）

# ② 桥调用 ∖ 头声明：差的三条**正是等 cabi 落进 opi_ffi.h 的**
$ comm -23 <(桥里调用的 opi_*) <(opi_ffi.h 声明的 opi_*)
opi_emoji_groups
opi_emoji_symbols
opi_search_symbols
$ comm -13 <(桥里调用的 opi_*) <(opi_ffi.h 声明的 opi_*)
（空）                          # 反向：声明了但桥没调用 —— 没有

# ③ clang：桥本身对着**补了那三条声明的副本头**零诊断
$ clang -std=c11 -Wall -Wextra -Wpedantic -fsyntax-only \
      -I/tmp/opi_bridge_check -I/usr/local/node/include/node /tmp/opi_bridge_check/napi_bridge.c
exit=0
#   （副本与仓库里的 napi_bridge.c **sha256 当场相同**，只多了三行声明；垫片同 §4）
$ # 同一份桥、对**真头文件**（尚未补声明）：
exit=1，3 个函数 / 共 6 条 error（每条 undeclared 后还跟着一条类型错）
#   napi_bridge.c:364 'opi_emoji_symbols'  → passing 'int' to 'OpiString'
#   napi_bridge.c:371 'opi_emoji_groups'   → 同上
#   napi_bridge.c:384 'opi_search_symbols' → initializing 'OpiString' with 'int'
⇒ **这是跨文件的先后，不是 C 面写错**：`opi_ffi.h` 是 cabi 的声明面（本端不改），
  三条声明与桥里的调用必须**同一批**落地；只加声明不加调用会红
  `harmony_header_is_consistent_use_site_subset`（§5 里那条规矩的另一半）。

# ③b ⚠️ **方法坑（当场踩到，值得记住）**：桥里是 `#include "opi_ffi.h"`，**引号形式**
#    ⇒ clang 先在**源文件所在目录**找，`-I` 的先后**压不过**它。
#    第一次写「对着真头编」时源文件还在补丁副本目录里 ⇒ 读到的**还是副本**，
#    exit=0 的**假绿**。把 napi_bridge.c 也拷进真头目录后再编才是上面这个 B 的真值。
#    （判「读的是哪份头」别靠 -I 顺序，靠**源文件位置**。）

# ④ 头侧 comm 的两个坑（同一次实测）
$ grep -oE '\bopi_[a-z_0-9]+\b' cpp/opi_ffi.h   # ← 会连**注释里**的名字一起抓
#   假阳性：opi_input_space / opi_select —— 两条只出现在头文件的注释里
#   （`opi_select_page` 的说明旁），不是声明。⇒ 头侧必须**行首锚定**（`^<类型> opi_名`）
$ # 锚定后：声明 19 条、桥调用 22 条
$ comm -23 调用 声明   → opi_emoji_symbols / opi_emoji_groups / opi_search_symbols（就这三条）
$ comm -13 调用 声明   → （空）

# ⑤ 项目自己的门禁（crates/opi-ffi/tests/c_abi_contract.rs）—— 分两次跑，两次原样输出都留
#    第一次（②③ 还没落地、产物也是旧的）：
test harmony_header_is_consistent_use_site_subset ... ok    ← 本端那条
test exported_symbols_exist_in_built_library ... FAILED     ← **不是本端红**，别算到桥头上
    找不到 ["opi_emoji_symbols", "opi_emoji_groups"]
#   归因（取证不是猜）：产物 target/release/libopi_ffi.so 是 10-12 20:01 的，
#   cabi_symbols.rs 01:33 / cabi.rs 01:41 ⇒ 缺的正是新加的两条；
#   opi_search_symbols 在旧产物里**本来就有**（它是老出口）。⇒ **产物陈旧**，不是源码缺陷。
#    第二次（收尾：声明已落、产物已是新的）：
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured
$ nm -D --defined-only target/release/libopi_ffi.so | grep -E 'opi_(emoji|search)'
00000000000b28b0 T opi_emoji_groups
00000000000b29b0 T opi_emoji_symbols
00000000000b2cc0 T opi_search_symbols
#   ⚠️ 产物**不是我重编好的**：我那条 `cargo build --release -p opi_ffi` 只印
#      「Finished in 0.20s」（零工作可做）⇒ 跑之前它就已是新的（他方重编）。归因写清，别记成我的功劳。
```

⚠️ **那条绿不能反推「声明已落地」**：`harmony_header_is_consistent_use_site_subset` 是**单向**
子集（头里每条声明都要同型 **且** 被桥调用）；**头里没有那三条，它就无从检查**。
判「落没落」要看 ④ 的 `comm -23`。（**本条写于落地之前**；落地后那一侧已空 —— 但规矩不变，
下一次加出口时它又会非空一次。）

⚠️ **反方向没有任何自动守卫（cabi 指出，我去源码核过）**：那条门禁的循环是
`for (name, csig) in &decls`（只遍历**头里的声明**），**从不**反过来遍历桥的调用 ⇒
「**桥调了、头没声明**」没有任何自动守卫。
它唯一的守卫就是 §4 那套**手工** `clang -fsyntax-only`（要 node 头垫片）与 ④ 那个**双向** `comm`。
⇒ 这两条不是「顺带跑跑」，而是**这半个方向的全部证据**；加出口的人得自己跑，不会有人替你红。

⚠️ **订正（2026-10-13，cabi 复核后）：本段上一版写的是「CI 也不管」，那是错的。**
错因正是**判据射程**：我拿 `grep -rni 'harmony\|hongmeng\|ohos' .github/workflows/` 零命中当
「CI 不管鸿蒙」的证据 —— 而 CI 里 `ci.yml:40` 的 `cargo test --workspace --no-fail-fast`
**真的管着这条线**，只是它跑的门禁把路径**硬编码**在测试里（`c_abi_contract.rs` 读 `HARMONY_H`），
workflow 文本里一个「harmony」字样都没有 —— **名字 grep 看不见硬编码路径**。
正确的分法是两半各归各的（正方向 CI 管、反方向无人管）：

· **正方向：CI 管。** `cargo test --workspace` ⇒ `cabi_unloaded`/`c_abi_contract` 等
  ⇒ `harmony_header_is_consistent_use_site_subset`。两种会红的样子：**头里声明了但桥没调**、
  **声明与 `cabi.rs` 不同型**。且该门禁读 `read_cabi()`（= `cabi.rs` + 拆出去的族文件）
  ⇒ 拆族时漏加文件它同样红。
· **反方向：无人管。** 「桥调了、头没声明」只有 `clang` 的 implicit-function-declaration 能抓，
  而 **CI 从不编 `napi_bridge.c`**（workflow 里对 `napi_bridge` 零引用；唯一那处 `clang`
  编的是 `macos/OpiFFI.h`）⇒ **① 反方向、② `napi_bridge.c` 自身的 C 编译**，这两件
  就是上面 §4 + ④ 的**独家证据**。
· 顺带：`cargo test --workspace` 还覆盖着 `cabi_unloaded`（未装载哨兵 + 末尾那条
  `every_export_is_actually_exercised_here` —— 它**现场**从 `read_cabi()` 解析导出全集，
  新导出没被调用就点名）与 `line_limit` 的 500 行。
  ⇒ 鸿蒙这条线被 CI 覆盖的**是这三样**（正方向契约 / 未装载哨兵与导出覆盖 / 行数），
  **不是「零覆盖」** —— 别按「CI 里零命中」那句话去推「所以随便写都行」。

**收尾（2026-10-13 02:33）：三条声明已落。** 落在 `harmony/cpp/opi_ffi.h` 的
「符号 / 表情（2026-10-13 加…）」那一段 —— **是 cabi 自己补的**（与我拟补的三行逐字相同）。
lead 事后授权我代落，我落完才发现**两边撞了**（文件里一度有两份同名声明）；已删掉我那份、
保留文件主人那份。去重后的实测（对**最终文件**）：`clang -fsyntax-only` 对真头
**exit=0（0 error / 0 warning）**；双向 `comm` **两侧皆空**（22 调用 / 22 声明）；
`c_abi_contract` **14 passed / 0 failed**。**回归对照**（同一份桥、对 `git show HEAD:` 那份头）
仍是 **exit=1 / 6 条 error** ⇒ 这次的绿不是空转。
`line_limit` 剩 **1** 处红（`android/.../OpiImeService.kt` 501 行，**别人的文件**）；本目录全部 < 500。
②③④ 三条在我复跑时都重来了一遍（对去重后的同一份文件）：`clang` 仍 exit=0、
`comm` 仍两侧空。**判据用 `grep -cE '^<类型> opi_<名>\('` 数「同名声明恰好 1 条」** ——
不抄行号、也不抄文件 hash：那两样都会静默漂。
（写完之后该头又落了一次**纯注释**修订 —— 把「空关键字 = 全表」拆成「实现层事实」与
「测试只钉了三条」两句。**我据新版本把 ②③④ 再跑了一遍**：`clang` exit=0（0 warning）、
`comm` 22/22 两侧空、同名声明各 1 条；三条声明与 `macos/OpiFFI.h` 仍**逐字节相同**。
⇒ **结论按「跑的最后那版」为准**：本条不记 hash，也就不存在「记录作废」这回事。）

⚠️ **本轮撞出来的一个门禁盲区（值得单列）**：`harmony/cpp/opi_ffi.h` 里**一度有两份**同名声明
（cabi 与我各落了一份，谁都不是抄错）。**没有任何门禁会红** —— `crates/opi-ffi/tests/c_abi/checker.rs`
的 `check_header` 把两侧解析结果收进 `BTreeMap<&str, &Sig>`（**按名字做键**，重复项静默覆盖），
且全仓没有一处断言「声明有几条」。⇒ 「门禁全绿」在这里**不蕴含「声明只有一份」**：
这份重复是**肉眼 + `grep -c`** 发现的。补出口时顺手数一遍同名声明条数（一条命令的事）。

⚠️ **出口面减法**：`cabi_symbols.rs` 现在共 **5** 条导出，本端桥了 **3** 条。
未桥的两条是 `opi_symbol_blocks()` / `opi_symbols_in_block(id)`（**通用符号块**，
`[{id,start,end,name,common}]`，与 `opi_emoji_groups` 不是一回事）——
它们是**引擎单例**语义（未装载 → 空串/`[]`，与表情那两条**相反**），
留给「符号分类页」那条线；本端没有调用方，**不预挂**（照 §5 的规矩：加声明必须同批加真调用）。

```
# ⑥ codelinter（本机 Command Line Tools 5.1.1.820 里那把 ArkTS linter）——**只证解析层**
$ codelinter -f json -o /tmp/codelinter-final.json /tmp/harmony-probe/src/src/main/ets
[]          # 零 parse 错
#   分辨力**先跑过两个正控**（否则 `[]` 可能是「根本没扫到」）：
#     正控 A：根目录塞一个 `const broken: number = ;` ⇒ 报告非空、指向该文件该行
#     正控 B：把 pages/Index.ets 抄一份、只坏一个常量、放回 pages/ ⇒ 报告指向
#             ZZBadCopy.ets:35:30 —— 一次证两件事：**会递归下探子目录**、
#             且**这种形状（装饰器 / `struct` / `@Builder`）确实被解析过**
#   范围：探针里 **`ets/**` 的全量 5 个文件**与仓库定稿**逐文件 sha256 相同**
#         （当场核过，不是「记得同步过」——第一次跑就踩了：有 2 个文件是旧快照）
#         ⚠️ 这个「逐文件相同」是**跑之前**的门槛，不是跑之后的自证：之后再改一行就不算数了。
#         `OpiEngine.ets` 收尾时加过一段注释（空关键字契约），**加完重跑过**，同样是 `[]`。
#   ⚠️ 三个坑：① 退出码**恒 0**，判据只看报告内容；② 参数写错（例如把目录喂给 `-c`）
#     会**不产出报告**且照样 exit 0 —— 静默假绿；③ 它**连类型错都放过** ⇒
#     `[]` 只等于「**语法没断**」，不等于「类型对」「ArkTS 语义对」。
```

**仍未验证**：ETS 侧那几条新包装与整个表情页**一行都没编译过**（U21–U23）；
表情**没有真的上屏过**；分组条**没有真的滚过**；`Set`/`Array.from` 那行是否被 ArkTS
接受未知。**DevEco 上第一次构建的核对顺序**写在 `cpp/napi_bridge.c` 顶部与
`EmojiPad.ets`（2026-10-14 从 `pages/Index.ets` 搬出）的注释里（照第 1 步的既有套路：
先编译、再按报错改名）。

### 5f. 输入模式接线（2026-10-14）—— **桥挂 16 条出口，UI 是九项面板**

引擎侧**输入方案 / 双拼布局 / 五笔六键**这一族（`cabi_scheme.rs` + `cabi_wubi.rs`）
本端一次接上：`cpp/opi_ffi.h` 加 16 条声明（**逐条抄 `macos/OpiFFI.h` 的同名同型**，
不自造签名）、`cpp/napi_bridge.c` 加 16 个 `Js*` + 16 行 `desc[]`、
`cpp/types/libopiime/index.d.ts` 加 16 条同名声明（**名字集合两边必须相等** ——
§5b 那条 diff 命令当场跑过，见下）。ArkTS 侧：

- `InputModes.ets`：出口包装 + **九项 `INPUT_MODES` 表**（逐条对齐 Android `InputMode.kt`：
  拼音26键/英文26键/拼音9键/拼音14键/拼音大9键/笔画键盘/生僻字键盘/拼音双拼/五笔）+
  模块级状态机（`selectInputMode` 换布局 + `setScheme` + 必要时切语言模式）；数字真源注在文件头。
- `KeyPanels.ets`：9 键 / 大 9 键 / 14 键 / 笔画四个键面 + 生僻字面板骨架 + 九宫格模式面板。
- `pages/Index.ets`：`build()` 按 `opiLayout` 分支挑键面；候选栏加「⌨」入口。

⚠️ **如实记下的五条**（细节在各文件头部）：
① **选模式不提交待选缓冲**（Android 会 `commitPendingBuffer`；本端没有「插入文本」通道 ⇒
未确认的拼音是**丢掉**的）；② 9 键左列**没有候选读音**（那条出口 `opijni_candidate_readings`
是 JNI 专属，**不在 C ABI 里** ⇒ 能力缺口，不是漏做）；③ **生僻字面板无数据**
（`rare_index.tsv` 不在仓库）——只画骨架，**不编假内容**；④ 五笔六键**暂无调用方**
（本端没有设置页），先接通是为了「设置页落地」变成纯 UI 工作。
⑤ **「123」键只切语言模式、不动输入模式**（对齐 Android `ImeState.openNumber`：
`switchMode(NUMBER)` + 换 view，**不碰 input mode**）——数字键面由 `opiMode` 画
（`pages/Index.ets::build` 把数字分支排在**布局**分支前面），所以 9 键用户按完 123
再退回来**还是 9 键**。⚠️ 别把它写成 `selectInputMode(拼音26键)`：那是真换输入模式
（`setScheme` 清缓冲），用户回来时键面已被悄悄换掉。
⚠️ **订正（2026-10-20）**：上面这句「两张表不在仓库」仍然成立（仓库里确实没有），
但**已不再「查不出字」** —— `stroke.opid` / `wubi86.opid` 现在由构建期脚本放进
rawfile、启动时拷进沙箱，`loadOptionalTables()` 拿绝对路径装载。仍传空串的
唯一情形是拷贝失败，此时 C ABI 的「返回 false 且引擎保持原样」语义不变：
**不崩、不假装有候选**。`wubi98.opid` 按「86/98 二选一」不随包。
（原文保留：它描述的是 2026-10-20 之前的树。）

**本机跑过的（原样输出）：**

```
$ diff <(grep -o '{"[a-zA-Z]*"' cpp/napi_bridge.c | tr -d '{"' | sort) \
       <(grep -o '^export const [a-zA-Z]*' cpp/types/libopiime/index.d.ts | awk '{print $3}' | sort)
IDENTICAL                      # 名字集合相等（两边都是 37 条；条数两边都不写，见 §5b）
$ cargo test -p opi_ffi --test c_abi_contract harmony_header_is_consistent_use_site_subset -- --nocapture
[契约] harmony/cpp/opi_ffi.h：38 条声明，全部与 cabi.rs 同型且都被桥调用（全集 67 条，有意子集）
test result: ok. 1 passed; 0 failed                        # 14 passed 的全量输出见交付简报
```

**仍未验证**：九项面板**没有在真机上点过**、四个键面的排布**没有看过**、
方案切换后引擎的候选**没有真的出过**；`checkedFiles` 只证到**解析层**（见头部两条补）。

### 6. 没验证的（**比上面长得多 —— 本节才是主体**）

**ArkTS（`ets/**` 全部文件）：0 行编译过。**
> **2026-10-20 订正**：这条已过期 —— release 构建的 `CompileArkTS` 成功，
> `ets/modules.abc` 进了 HAP。下面这段描述的是 2026-10-20 之前；
> **编译过 ≠ 真机可用**，运行期 ArkUI/IME Kit 行为仍全部未验证。

不只是「没测过」——**连类型都没过过编译器**。装饰器（`@Entry` `@Component` `@Prop`
`@Watch` `@StorageLink` `@Builder`）、`@kit.IMEKit`、`@kit.ArkUI` 的 `display`、
Canvas / `CanvasRenderingContext2D`、`JSON.parse(...) as string[]` 的断言，
**全部凭记忆写的**。上面「最不确定的 API」那张表 U1–U20 一条都没消。

> **2026-10-13 补充（缩的是「语法」那一格，不是「编译」）**：`ets/**` 全量 5 个文件
> 过了 codelinter 的**解析器**（零 parse 错，且先跑过两个正控证明它有分辨力）。
> 但**这仍然不是编译**：没类型检查、没装饰器语义、没 ArkTS 特有的约束
> （它连类型错都放过）。所以本节标题的「0 行编译过」**不变**。
>
> **2026-10-14 补充**：文件数已到 8（见头部那条补），同一探针重跑后**仍是零 parse 错**，
> 结论一格没松；另**新增的负控说明了它连命名导入都拦不住**（详见头部 2026-10-14 补）。

> 本轮新接的三条（`toggleFullwidth` / `fullwidthState` / `toggleSymbol`）**同样落在这一段**：
> `OpiEngine.ets` 里那三个包装函数**一行都没编译过**，`toggleSymbol` 返回的字符串
> **没有真的上屏过**，全角指示**没有真的显示过**。§5d 那两层门禁证的是
> 「声明与调用对得上、类型写得对」，**不证「行为对」**。

**N-API 的实现侧：**
- 模块能否**注册成功**（U7）：`__attribute__((constructor))` + `napi_module_register`
  鸿蒙认不认 —— **语法检查看不出来**，注册失败的表现是 `import` 得到 `undefined`
  且不报错。这是最难查的一条，**仍未验证**。
- 运行期语义：`napi_create_string_utf16` 是否真拷贝（U9，规范说拷贝）；
  `napi_get_value_string_utf16` 的 `bufsize` 是否含 NUL（U8 的**语义**部分，规范说含）。
  **实现未验。**
- 鸿蒙 NDK 自己的 `napi/native_api.h` 是否提供 §4b 那 14 个 `napi_*` 符号。
  （**签名**已对着 node 的真头核过；**这个头在鸿蒙侧存不存在、内容是否一致**没验。）

**配置与清单：**
- `module.json5` / `input_method_config.json` / `oh-package.json5`：
  **未过任何 schema 校验**（没有 DevEco，没有 hvigor）。
- `input_method_config.json` 的 `mode: "lower"`：**合法取值至今没查到**（U12）。

**链接与产物：**
- **从未把 `libopi_ffi.a` 链进任何东西**。§4b 那次「链接」是在宿主 gcc 上、
  且根本没走到符号解析（先卡在 OHOS 专有库上）。Rust 静态库带 std 与
  compiler_builtins，**与 OHOS 运行时是否符号冲突未知**。
- 那三条 `target_link_options`（`--gc-sections` `--allow-multiple-definition` `-s`）
  被 GNU ld 接受，**但 OHOS 的 ld（lld）认不认没验** —— CMakeLists 里原本就标了
  「凭记忆的兜底」。

**真机行为：完全没有。** 没有设备、没有模拟器、没有 SDK、没有 DevEco。
**「打 `ni` 出不出字」这件事，本机无法回答。**

---

## 构建集成

### 为什么是 N-API 薄壳 + 复用 C ABI

鸿蒙原生模块走 **Node-API（N-API）**，不是 JNI、也不是裸 C ABI 直连。
但**底下的 Rust 引擎不必改，也不该改**：

```
  ArkTS (ets/**)
      │  import opiime from 'libopiime.so'
      ▼
  libopiime.so  ←─ cpp/napi_bridge.c（约 300 行，只做搬运 + 释放）
      │  extern "C" 直接调
      ▼
  libopi_ffi.a  ←─ crates/opi-ffi（**同一个 crate，C 导出以 `cabi.rs` 为准，一个字符没改**）
      │
      ▼
  crates/engine-core（与 Android / iOS / macOS / 桌面 / fcitx5 / TSF **同一份引擎**）
```

**不给鸿蒙再造一套引擎接口** —— 那会变成第 N 份会漂移的语义拷贝，
而本项目已经为「同一语义抄三份」付过代价。

**为什么不引 `napi-rs`**：它要在 `crates/opi-ffi/Cargo.toml` 加依赖、加 `#[napi]` 宏，
那会改到 `crates/**`，并把「一份引擎 + 各端薄壳」变成「Rust 侧长出平台分支」。
手写 N-API 声明约 300 行、**零新依赖、Rust 侧一个字符不用动**。
代价就是那 300 行**没有鸿蒙侧的编译器保护** —— 所以它被明确标成草案。
（2026-09-27 补：本机现在至少把它过了 clang 语法/类型检查、对着真 N-API 头、
并真编出过 `.o`，见 §4 与 §4b。**那仍然不是 OHOS 编译器**，
注册与链接依然未验 —— 「草案」这个定性不变。）

### 产出 Rust 静态库

静态库（而不是 `cdylib`）能让最终只出一个 `.so`，
不必再把第二个库塞进 HAP 的 `libs/` 目录、也不必操心加载顺序。
`crates/opi-ffi/Cargo.toml` 的 `crate-type` 已含 `staticlib`，无需改动：

```bash
# 在仓库根执行。三个 ABI 按需产出。
rustup target add aarch64-unknown-linux-ohos armv7-unknown-linux-ohos x86_64-unknown-linux-ohos

cargo rustc -p opi_ffi --target aarch64-unknown-linux-ohos --release --crate-type staticlib
cargo rustc -p opi_ffi --target armv7-unknown-linux-ohos  --release --crate-type staticlib
cargo rustc -p opi_ffi --target x86_64-unknown-linux-ohos --release --crate-type staticlib

# 拷进 DevEco 工程（目录名是鸿蒙 ABI 名，不是 Rust triple 名）
cp target/aarch64-unknown-linux-ohos/release/libopi_ffi.a  entry/src/main/cpp/libs/arm64-v8a/
cp target/armv7-unknown-linux-ohos/release/libopi_ffi.a    entry/src/main/cpp/libs/armeabi-v7a/
cp target/x86_64-unknown-linux-ohos/release/libopi_ffi.a   entry/src/main/cpp/libs/x86_64/
```

ABI 目录名对应关系（`CMakeLists.txt` 靠 `libs/${OHOS_ARCH}/` 找）：

| 鸿蒙 ABI | Rust target |
|---|---|
| `arm64-v8a` | `aarch64-unknown-linux-ohos` |
| `armeabi-v7a` | `armv7-unknown-linux-ohos` |
| `x86_64` | `x86_64-unknown-linux-ohos` |

还要在 `entry/build-profile.json5` 里登记 CMake，**否则 CMake 根本不会被调用**：

```json5
"buildOption": {
  "externalNativeOptions": {
    "path": "./src/main/cpp/CMakeLists.txt",
    "abiFilters": ["arm64-v8a"]
  }
}
```

> 体积提醒：本机产出的 release 静态库 **9.0 MB / 404 个目标文件**，
> 因为带了 debug_info、`std`、`compiler_builtins`，**还含一份 Android JNI 出口**
> （`crates/opi-ffi/src/jni.rs` 在鸿蒙目标下也照编，是死重量）。
> 想去掉 JNI 那份得给 `jni.rs` 加 `#[cfg]` 门 —— 那要改 `crates/**`，不在本轮边界内，
> 记在「已知缺口」。`CMakeLists.txt` 里已有 `-Wl,-s` 与 `--gc-sections` 兜着。

### 词库随包（2026-10-20）

rawfile **不是文件系统路径**，而引擎只收绝对路径 ⇒ 三步：

1. **构建期**：`scripts/prepare_harmony_dicts.sh` 从 `android/app/src/main/assets/`
   复制 `luna.opid` / `stroke.opid` / `wubi86.opid` 到
   `entry/src/main/resources/rawfile/`（脚本用 `cmp` 复核，复制品被 `.gitignore`
   挡住，**不入库**）。CI 的 `harmony.yml` 在 `assembleHap` 前跑同一脚本，并在
   `unzip -l` 里逐张断言三张表进了 HAP。
2. **首次运行**：`DictionaryAssets.syncDictionaries()` 把 rawfile 读成
   `Uint8Array`，写进 `ctx.filesDir`。**同一份已存在且 `statSync().size` 相等就跳过**；
   每次读/写后都校验非空与字节数，任一步失败返回空串并打 `[opi]` 日志。
3. **装载**：`KeyboardController.ensureDictionary()` 每进程只跑一次
   （`dictLoaded` 挡板），把沙箱绝对路径交给 `opi_load`。**返回 false 就回退
   `opi_load('')`**（引擎契约：坏路径 → false 且引擎保持为空，不回退就一个字都不出），
   并置 `dictDegraded`；`Index.ets` 把它传给 `petMood()`，小欧画 DEGRADED。
   `loadOptionalTables()` 只在启动时传 `stroke.opid` 与 `wubi86.opid` 两条路径 ——
   五笔**只装一张表**（86 固定，没有设置页可切 98）。

为什么不随包 `trad.opid`：`opi_load_trad` **不在**鸿蒙桥的 use-site 子集里
（`opi_ffi.h` / `napi_bridge.c` 都没有），装了也没有出口可调用，见已知缺口 5。

本机 release 产物实测：HAP **约 48 MB / 13 个文件**，含
`resources/rawfile/{luna,stroke,wubi86}.opid`（33,305,708 / 5,439,555 / 3,157,765 B）、
`libs/arm64-v8a/libopiime.so`（3,221,088 B）、`ets/modules.abc`（1,589,700 B）、
`resources/base/profile/main_pages.json` 与 `input_method_config.json`。
另用 C 探针直接链 `libopi_ffi.a` 读 HAP 里那三份文件：`ni` 在回退库下是
`你`+8 个符号位（5 页），换完整 luna 后是 `你/泥/拟/呢/腻/祢/铌/鲵`（1178 页），
`opi_load_stroke` / `opi_load_wubi` 均返回 1 —— 证明**随包的字节确实是有效词库**。

> ⚠️ 首次 `inputStart` 的拷贝是**同步**的（三张表约 42 MB）：真机首字可能有可感停顿，
> 之后命中的是 `statSync` 大小相等分支，不再复制。本轮无设备，未量。

### 建议进 CI（**本节只是建议；`harmony.yml` 已按此接好，`ci.yml` 仍未改**）

`.github/workflows/ci.yml` 里已有 TSF（Windows）与 Apple 两条同样思路的 job。
鸿蒙可以照抄 Apple 那条的形状，加在它后面：

```yaml
      # 同理：check 不链接，Linux runner 上无需 OHOS SDK。
      # 这守的是「C ABI 还能为鸿蒙目标编译」—— 它**不**证明能链接成 .so，
      # 更**不**证明 harmony/ 的 ArkTS 与 N-API 桥能编译（那需要 DevEco + OHOS SDK）。
      - name: cargo check -p opi_ffi (鸿蒙目标)
        run: |
          # 与 Apple 不同：这三个目标不在默认安装里，**必须先 add**。
          rustup target add aarch64-unknown-linux-ohos armv7-unknown-linux-ohos x86_64-unknown-linux-ohos
          for t in aarch64-unknown-linux-ohos armv7-unknown-linux-ohos x86_64-unknown-linux-ohos; do
            echo "== $t =="
            cargo check -p opi_ffi --target "$t"
          done
```

---

## 平台限制

> ⚠️ **本节每一条都是「凭记忆 + 公开文档检索」，本机无法验证。逐条标了需要核对什么。**
> 检索到的官方文档之间还有互相矛盾之处，见「最不确定的 API」。

| # | 限制 | 状态 |
|---|------|------|
| P1 | **用户必须在系统「设置」里手动启用**这个输入法，它才会出现在输入法列表里。装完不会自动生效。 | 凭记忆 + 检索，**需核对** |
| P2 | 输入法必须注册成 **`extensionAbilities[].type = "inputMethod"`**，且 `exported: true` —— 否则装了也看不到。**不是** `module.type`。 | 检索确认，**与部分网上文章冲突**（那些文章说要写 `module.type: "inputmethod"` + `BIND_INPUT_METHOD` 权限，多篇资料指出是错的） |
| P3 | **版本兼容是大坑**：部分 HarmonyOS NEXT 版本**不开放 InputMethod 类型的 Extension Ability**；有社区反馈旧版 DevEco 也建不出 InputMethod 扩展（建议 DevEco 6.0+）。**开发前先确认目标系统版本支持**。 | 检索，**需核对** |
| P4 | 子类型（中/英/数字键盘）**共用同一个 `InputMethodExtensionAbility`**，靠 `inputMethodAbility.on('setSubtype')` 区分。本目录**尚未监听它** → 切子类型不会换键盘。 | 检索确认 + 已知缺口 |
| P5 | 安全模式：输入法扩展运行在受限沙箱，网络/剪贴板/联系人/定位等能力受限；主应用与输入法扩展**是隔离沙箱**，要共享词库/配置需申请 `dataGroupId`。 | 检索，**需核对** |
| P6 | 输入法实例由系统**按输入会话反复创建/销毁**，进程可能被回收 —— 不能把词库/会话状态当应用级单例长期持有，状态必须能快速重建。 | 检索，代码已按此写（`init()` 有幂等挡、`inputStart` 时清缓冲） |
| P7 | 拉起输入法选择弹框用 `inputMethod.getSetting().showOptionalInputMethods()`（旧接口 `displayOptionalInputMethod` 自 API 9 废弃）。**这是主应用侧**的设置入口，不是输入法扩展里调的。 | 检索确认 |
| P8 | `showOptionalInputMethods` 有跨用户限制（仅 user 0 授权，错误码 12800025）；社区反馈该弹框在某些版本上「点了不生效」，替代方案是 `switchInputMethod` 或 `startAbility` 拉起设置页（`com.huawei.hmos.settings` / `uri: "set_input"`）。 | 检索，**需核对** |
| P9 | 上架前需 strip；release 静态库 9 MB（本机实测），未 strip 会明显撑大 HAP。 | 本机实测（体积） |

---

## 我最不确定的 API

**这是给接手人最省时间的一节。** 以下按「错了会怎样」的代价从高到低排。
每一条我都**无法验证**（没有 SDK），只能是「凭记忆 + 部分检索」。

> **订正注记（2026-09-27）**：下表是**写下时的**不确定清单，**不改原文**（留痕）。
> 本轮在本机重跑后，**U5 / U6 / U8 的「签名」部分已经不再是未知**：
> `cpp/napi_bridge.c` 对着 node v22 的**真 N-API 头**过了 `clang -fsyntax-only`
> 且做了负向对照，`napi_module` 字段名、`napi_get_value_string_utf16` 的
> 参数个数与类型都核过（详见上面 §4）。**U5 的头文件名、U7 的注册方式、
> U8 的 NUL 语义、U9 的拷贝语义仍然未验证** —— 那几条是**运行时/鸿蒙实现**问题，
> 语法检查看不出来。**U1–U4、U10–U20 一条都没消。**
>
> **再补（2026-10-13，表情页接线）**：本轮又添 **U21–U23** 三条，全在「中」档
> （错了 UI 不对、键盘还能用），来源是表情页那三处新写法（`TextInput` / 横向 `Scroll` /
> `Set`+`Array.from`）。**一条都没验证** —— 本机没有 ArkTS 编译器，
> 但**解析层过了 codelinter**（它只证语法解析、不证类型，判据与退出码陷阱见开头那段
> 2026-09-28 订正与 §5e 的原样输出）。
>
> **再补（2026-10-14，输入模式接线）**：又添 **U24–U25** 两条，同样「中」档，
> 来源是新键面那两处写法（自定义组件当键帽 / 九宫格的两层嵌套 `ForEach`）。
> **一条都没验证**；解析层同 10-13（8 个文件零 parse 错，原样输出见头部 10-14 补）。
>
> **再补（2026-10-18，自有贴纸）**：又添 **U26** 一条（「中」档：错了自有页不出图
> 或复制不到剪贴板，键盘其余部分可用）。来源是 `PetStickerImages.ets` 与
> `PetStickerClipboard.ets` 的五处新写法。**一条都没验证** —— 本机没有 ArkTS 编译器
> 与鸿蒙设备；解析层由 codelinter 复跑（见头部 10-18 补 / 二补）。

### 极高：错了整个端起不来

| # | 我写的 | 不确定什么 | 错了的表现 |
|---|--------|-----------|-----------|
| U1 | `InputMethodExtensionAbility` 继承自 `@kit.IMEKit`，重写 `onCreate(want)` / `onDestroy()` | **导入路径与基类名**。旧文档是 `@ohos.inputMethodExtensionAbility`，新的是 `@kit.IMEKit`。生命周期方法签名也可能带别的参数。 | 编译不过。若签名不符会编译过但生命周期不被调用 → 键盘永远不出现 |
| U2 | `inputMethodEngine.getInputMethodAbility()` → `.createPanel(ctx, panelInfo)` → `panel.resize(w,h)` / `.moveTo(x,y)` / `.setUiContent('pages/Index')` / `.destroyPanel()` | **整条创建面板的链路**。方法名、`PanelInfo` 的字段名（`type`/`flag`）、`PanelType.SOFT_KEYBOARD`、`PanelFlag.FLG_FIXED`、以及 `resize`/`moveTo` 的参数顺序与单位（px 还是 vp）全是记忆。 | 键盘面板不出现，或尺寸/位置错乱 |
| U3 | `ability.on('inputStart', (kbController, inputClient) => {})` / `on('inputStop', ...)` / `off(...)` | **回调签名与参数顺序**。我按 `(kbController, client)` 写的。 | 拿不到 `InputClient` → 一个字都上不了屏 |
| U4 | `client.insertText(text)` / `client.deleteForward(n)` | **`InputClient` 的方法名与参数**。`deleteForward(1)` 的「1」是字符数还是码点数也没把握（emoji 可能算 2）。 | 上屏/删除失效 |

### 高：错了功能静默坏掉

| # | 我写的 | 不确定什么 | 错了的表现 |
|---|--------|-----------|-----------|
| U5 | `napi/native_api.h` 是 N-API 的包含路径；`napi_module_register()` 在其中声明 | **头文件名与函数声明位置**。有资料用 `node_api.h`。 | 编译不过（相对容易发现，属好情况） |
| U6 | `napi_module` 结构体字段名：`nm_version` / `nm_flags` / `nm_filename` / `nm_register_func` / `nm_modname` / `nm_priv` / `reserved[4]` | **字段名与顺序**。 | 编译不过，或注册静默失败 → `import` 得到 `undefined` |
| U7 | 注册靠 `__attribute__((constructor))` + `napi_module_register(&mod)` | **构造器属性是否是鸿蒙认的注册方式**（也可能是 `napi_register_module_v1` 之类的固定符号）。 | `import` 得到 `undefined`，且**不报错** —— 最难查的一种 |
| U8 | `napi_get_value_string_utf16(env, v, NULL, 0, &len)` 先问长度、再取内容；`bufsize` 含结尾 NUL | **第五个参数语义（是否含 NUL）**。多算/少算会截断字符串。 | 候选词被截断，或写入越界 |
| U9 | `napi_create_string_utf16` 会**拷贝**数据，所以可以「先 create 再 free」 | **是否确实拷贝**（Node-API 规范说拷贝，鸿蒙实现未验证）。若不拷贝，`take_string` 就是 use-after-free。 | 花屏/崩溃，且**难以复现** |
| U10 | `module.json5` 的 `metadata.name`：我写了 `"ohos.extension.input_method"` | **点 vs 下划线**：V14 文档写 `ohos_extension.input_method`，其它文档写 `ohos.extension.input_method`。**官方文档自己就不一致。** | 输入法能选，但子类型（中/英键盘）不出现 |
| U11 | `srcEntry`（新） | 旧文档是 `srcEntrance`。 | 编译/安装不过 |
| U12 | `input_method_config.json` 的 `mode` 字段取值 `"lower"` | **合法取值完全没查到**。 | profile 解析失败 → 输入法可能不出现 |
| U13 | `@Entry` 页面路径 `'pages/Index'`（`setUiContent` 与 `main_pages.json` 两处一致） | **路径格式**：相对 `src/main/ets`？带不带 `.ets`？ | 面板起来了但是**一片白** |

### 中：错了 UI 不对但键盘能用

| # | 我写的 | 不确定什么 |
|---|--------|-----------|
| U14 | `CanvasRenderingContext2D` 有 `ellipse(cx,cy,rx,ry,rot,a0,a1)`（小欧的嘴和落地投影用它） | 是否提供 `ellipse`。**没有的话改用 4 段 `bezierCurveTo` 的椭圆近似（kappa≈0.5523）**，`arcTo`/`quadraticCurveTo`/`arc` 是确定有的。 |
| U15 | `ctx.arcTo()` 拼圆角矩形（`roundRect` 在 ArkUI 上不保证存在，故没用） | `arcTo` 的参数与行为 |
| U16 | `ctx.measureText(s).width`、`ctx.fillText(s,x,y)`、`ctx.font = 'bold 21px monospace'`、`ctx.textBaseline` | Canvas 字体串能否解析 `monospace`（鸿蒙系统字体是 HarmonyOS Sans，等宽族是否落到它身上不确定）→ 刻字可能变宽/变窄。成功与否**只影响观感**。 |
| U17 | `AppStorage.setOrCreate<T>(key, value)` 与 `@StorageLink` 能双向同步 `string[]` | 数组类型的同步行为（ArkUI 对数组的观测有时要 `@Observed`） |
| U18 | `@Builder` 方法带**多个参数 + 函数参数**（`KeyCap(label, action, weight, active)`） | ArkUI 的 `@Builder` 多参传递在部分版本有「值传递不刷新」的限制。不刷新只影响高亮。 |
| U19 | `display.getDefaultDisplaySync()` 返回的 `width`/`height` **单位是 px**，直接喂给 `panel.resize()` | 单位是否为 px、要不要 `vp2px` 换算。**键盘高度明显不对时先查这里。** |
| U20 | `JSON.parse(raw) as string[]` 在 ArkTS 严格模式下可用 | ArkTS 对 `JSON.parse` 返回值（`Object`）的断言限制 |
| U21 | 搜索框 `TextInput({ placeholder, text })` + `.onChange(v => …)`（2026-10-13 新增） | **组件名与这三个入参**；更要紧的是**IME 面板内能否聚焦**（面板自己就是输入法，系统没有第二个输入法可唤起）。 | 编译不过（好）；或**能显示、打不了字**（静默退化：只能点分组看）→ 备选 `Search({ value, placeholder })`，再退是面板字母键直接喂 `emojiQuery` |
| U22 | 分组条 `Scroll(){…}.scrollable(ScrollDirection.Horizontal)`（2026-10-13 新增） | `scrollable` 与 `ScrollDirection` 的名字/取值。 | 编译不过，或分组条不滚；备选 `List(){…}.listDirection(Axis.Horizontal)`，再退裸 `Row`（9 组会横向溢出） |
| U23 | `new Set<string>(数组)` + `Array.from(set)` 去重、`Array.prototype.slice` 切行（2026-10-13 新增） | ArkTS 严格模式对泛型构造与 `Array.from` 的限制（与 U20 的 `JSON.parse` 同一族）。 | 表情页去重/切行那两处编译不过；备选是手写 `has`/`add` 循环与下标循环 |
| U24 | 自定义组件当键帽：`KeyBox`（`@Component export struct`，普通成员 `onTap: () => void` 随构造参数传入）、以及**在调用点**给自定义组件挂 `.layoutWeight(n)`（2026-10-14 新增） | 函数类型成员能否随构造参数传；`layoutWeight` 这类**属性修饰**能否作用于自定义组件（`OpiPet` 有 `.margin`/`.onClick` 的先例，但那是通用属性）。 | 编译不过；或键面布局塌成等分/挤成一行 —— 备选 = 传数字参 `weight`，由 `KeyBox` **内部**自己 `.layoutWeight(weight)` |
| U25 | 九宫格模式面板的两层嵌套 `ForEach`（外层 `MODE_GRID` 的行、内层每行三个 chip）（2026-10-14 新增） | 嵌套 `ForEach` 在 ArkTS 严格模式下的可用性（表情页网格是**同构先例**，但那边行数组是**现算**的，这里是常量表）。 | 编译不过或只出一行；备选 = 手写 9 个 chip（多 20 行，逻辑不变） |
| U26 | `Image('data:image/png;base64,…')` 直接画内嵌 PNG；`util.Base64Helper().decodeSync`、`image.createImageSource(ArrayBuffer).createPixelMap()`、`pasteboard.createData(pasteboard.MIMETYPE_PIXELMAP, pixelMap)` + `getSystemPasteboard().setData`、`promptAction.showToast`（2026-10-18 新增） | 五处 API 的名字/签名全凭 d.ts 记忆；`Image` 是否接受 base64 data URI、`decodeSync` 返回 `Uint8Array` 还是 `ArrayBuffer`、`createData` 是否直接收 `PixelMap`、IME 扩展里 toast/剪贴板是否受前台/权限限制。 | 自有页 PNG 画成空框；或 toast 正常但宿主**粘贴不到图**（剪贴板写失败，`.catch` 有 console 日志）。渲染备选 = 先解码成 `PixelMap` 再 `Image(pixelMap)` |
| U27 | `Image('data:image/gif;base64,…')` 播 GIF；`pasteboard.createData('image/gif', ArrayBuffer)`（2026-10-19 新增） | ArkUI 是否逐帧播放 GIF data URI、`createData` 是否接受任意 MIME + `ArrayBuffer`。 | 面板只画 GIF 首帧或空框（退回静态 PNG 的代码在生成数据里，但 UI 不做运行时探测）；GIF 粘贴失败会落 PNG |

### 待 DevEco 确认：`@StorageLink` 的入参必须是字符串字面量

| 我写的 | 不确定什么 | 错了的表现 |
|---|-----------|-----------|
| `@StorageLink('opiBuffer') buffer: string = ''` 等**内联字面量**（2026-10-14 起 `pages/Index.ets` 9 条 + `KeyPanels.ets` 3 处 `opiMode`） | **规则是否真的只收字面量**。华为规格与一则同症状求助帖都说装饰器入参是编译期静态处理的、传常量（`@StorageLink(KEY_BUFFER)`）编译不过；但这两条都是**二手资料，本机无法验证**。 | 若规则其实更宽松：现在的写法**仍然合法**（字面量两种规则下都成立），只是把键名写了两份。 |

**为什么还是按这个改**：`KeyboardController.ets` 原本把 `KEY_*` 集中在一处、注释写着
「集中在这里，避免两边写不一致的字符串」—— 而 `@StorageLink(KEY_BUFFER)` 恰恰是
ArkTS **不许做**的事。内联字面量在「规则严」与「规则松」两种情况下都能编译，
是唯一稳妥的写法；代价是**键名在用到它的每个文件里各有一份**（`pages/Index.ets`、`KeyPanels.ets`；写入侧常量在 `KeyboardController.ets`）：

- 写入侧 `KeyboardController.publish()`：`AppStorage.setOrCreate(KEY_X, …)` —— 运行时调用，
  **不受限**，继续用常量（那才是真源）。
- 读取侧 `pages/Index.ets` / `KeyPanels.ets` 的装饰器：只能字面量，**漏改不报错**。

漏改的症状是「面板起来了、按键也有反应，但候选栏 / preedit 永远不刷新」——
**静默失败**，最难查的一类。所以核对只能靠人：

```bash
grep -rn "opiBuffer\|opiCandidates\|opiPage\|opiPageCount\|opiMode\|opiShift\|opiLearner" harmony/
```

两边的字面量必须一一对应。**DevEco 上第一件要试的事**：把某一处改回常量看是否真
编译不过 —— 若编译得过，说明规则比资料说的宽松，那时可以考虑收敛回常量。

### 明确「没查到、也没猜」的

- 鸿蒙是否要求输入法声明某个权限（如 `ohos.permission.INPUT_METHOD`）。
  检索**没有**找到可靠说法；P2 里那个 `BIND_INPUT_METHOD` 疑似错误信息，未采纳。
- `deleteForward(n)` 的 `n` 对代理对（emoji）如何计数。
- **「本机字体画不画得出这个字形」的对应 API。** Android 是
  `Paint.hasGlyph`、iOS 是 `CTFontGetGlyphsForCharacters`，**鸿蒙侧本机没有 SDK，
  查不到对应物**（是 `@ohos.graphics.text` / 字体管理那几族里？没有把握）。
  按规矩**不编一个**：表情页**不做这个过滤**，代价（个别新码位画成空心框）如实写在
  `EmojiPad.ets`（原 `Index.ets` 的 `EmojiPage`）注释与「已知缺口」第 3 条。⚠️ 补的时候**只判单码位** ——
  多码位序列（国旗/家庭/键帽）会被误判成「没有」，把整类能显示的藏掉，比空心框更坏
  （Android `SymbolCatalog.canRenderGlyph` 为此写了 `codePointCount > 1 ||` 的短路）。
- **把回车「交还给应用」的接口在鸿蒙叫什么。** 引擎在缓冲为空时按设计放行回车
  （`KeyAction::PassThrough`，语义是「这个键归应用」），客户端必须有个通道把它送出去 ——
  Android 侧是 `InputConnection.performEditorAction`（`KeyRouter.kt` 的 `performEnter`），
  **鸿蒙 IME Kit 的对应物本轮没查到、也没猜**。所以 `tapEnter` 的注释里只写了缺口
  与做法，**没有**凭记忆写一个 API 名进来。这条不补上，空缓冲下的 ↵ 就是死键
  （见「已知缺口」第 10 条）。

---

## 已知缺口（**不是 bug，是还没做**）

1. **未监听 `setSubtype`** → 切中/英子类型不会换键盘（P4）。
2. ~~**词库是内置回退库**（`loadDictionary('')`）。完整词库要先把 rawfile
   拷进沙箱再传绝对路径 —— 引擎要文件系统路径，不是资源 ID。
   步骤写在 `KeyboardController.ensureDictionary()` 的注释里。~~
   **2026-10-20 已补**：完整三张表随包，rawfile → `filesDir` → 绝对路径已接通；
   内置库现在只在拷贝/装载失败时兜底（并置 DEGRADED）。见「词库随包」。
3. **无符号面板**（`opi_symbol_blocks` / `opi_symbols_in_block` 未桥接）。

   > **2026-10-20 对齐轮**：符号页与生僻字面板已落地（`SymbolPage.ets` /
   > `SymbolData.ets`，rawfile TSV 驱动：`symbol_categories.tsv` 22 类、8×4/页 +
   > 页头 ‹ › 翻页；`rare_index.tsv` 214 部首 → 笔画数 → 字表）。
   > 点格直接 `insertText`，不经过引擎。引擎那两条分块出口仍未桥，但本路径不依赖它们。
   > 下面 2026-10-13 记录里的「29 个 ASCII 符号不可达」已由符号页补上；
   > 搜索输入通道 / 字形过滤 / 网格虚拟化三条原样保留。

   > **2026-10-13 部分更新（表情页接线）**：`opi_search_symbols` **已桥**
   > （见 §5e）；Symbol 模式不再是一页硬编码符号，换成了**表情页**
   > （分组条 + 网格 + 搜索框）。**但「符号面板」这件事只做了一半** ——
   > 上面原文那三个出口里还有两个没桥，而下面四条是本轮**新产生的**代价/缺口：
   >
   > 1. **29 个 ASCII 符号不再一键可达**：原 `SYM_ROW_1..3`（`[]{}#%^*+=_\|~<>€£¥•` 等）
   >    随本轮删除，而 Android 那边这些符号在「常用/全部」两个 Tab 里 —— 本端**没有 Tab**。
   >    补法有两条，都要先裁决：① 加「常用/全部/表情」三 Tab（要桥 `opi_symbol_blocks` +
   >    `opi_symbols_in_block` 两条出口）；② 只在表情页里加一个「符号」分组（**不是**
   >    emoji 表的内容，得另找数据源）。**别**把符号行塞回 `letterRows()`：那会与
   >    「Symbol 模式整片是表情页」的分支打架。
   > 2. **搜索框的输入通道未定**：面板自己就是输入法，`TextInput` 聚焦时系统没有第二个
   >    输入法可唤起 ⇒ 真机上大概率「能显示、不能打字」。Android 的做法是搜索激活时把
   >    **面板自己的键行**重定向进搜索框（`ImeState::searchKey`），本端照做要先解决
   >    「键盘往哪儿摆」（面板只有 0.42 屏高，见 `KeyboardController.PANEL_HEIGHT_RATIO`），**本轮没做**。
   > 3. **画不出的字形不过滤**：Android 用 `Paint.hasGlyph`、iOS 用
   >    `CTFontGetGlyphsForCharacters`，鸿蒙侧的对应 API **本机查不到、也没编**
   >    （现 `EmojiPad.ets`、原 `Index.ets` 的 `EmojiPage` 里如实标着）⇒ 个别新码位（如 U+1FAEB）画成空心框。
   >    补的时候**只判单码位**，且**分组 `start`/`count` 要跟着重算** ——
   >    Android `SymbolCatalog.emojiGroups` 就是为此把下标按过滤后列表重算的。
   > 4. **网格不虚拟化**：`Scroll + Column + ForEach` 是**全量铺节点**，
   >    「人物」组 2430 项 ≈ 300 行的节点会在真机点它那一下一起建出来。
   >    换 `List{LazyForEach}` 需要一个 `IDataSource` 实现，且 ArkUI 的 `LazyForEach`
   >    **只能**做 `List`/`Grid`/`Swiper` 的直接子节点（**塞不进 `Scroll`/`Column`**）——
   >    所以这不是「加一行」，是换容器。骨架本轮**没写**（写一个没编译过的
   >    `IDataSource` 只会再欠一笔）。
   >
   > **2026-10-18 补**：布局已换成 Android 版（顶部三 Tab + 底部横滑分类条），
   > 缺口 ②③④ 原样保留（搜索输入通道未解决、无字形过滤、网格不虚拟化）。
   > 颜文字走内嵌 `KaomojiData.ets`，不依赖引擎；`自有` 是独立子页，画 12 张真实
   > 小欧 PNG（标签/文件名映射逐条等于 `shared/pet/PetEmojiMap.kt`），点击走
   > **剪贴板回退**（见第 15 条），不再画 `OpiPet` 也不提交 emoji 文本。
4. **无用户词持久化**（`opi_import_user_words` / `opi_export_user_words` /
   `opi_clear_user_words` / `opi_remove_user_word` 未桥接）。
5. **无繁体切换入口**（`opi_load_trad` 未桥接；`OpiMode.Traditional` 有枚举
   但 UI 的轮转里进不去）。**2026-10-20 词库随包时仍然如此**：因为桥里没有这个出口，
   `trad.opid` **有意不随包**（装了也调不到）；在桥补上这个出口之前，鸿蒙端
   没有「切换整本繁体词典」的能力。要补先桥 `opi_load_trad`。

   > **2026-10-20 对齐轮**：UI 入口已补 —— 26 键模式键是三态循环（中→繁→英→
   > 上次中文），9/14/笔画底行「繁」键只切简繁。但 `opi_load_trad` 仍未桥、
   > `trad.opid` 仍不随包 ⇒ 引擎 `Traditional` 回退 luna **简体**表，输出仍是简体。
   > 上面「UI 的轮转里进不去」已过期；现在应读作「有入口、无繁体词典」。
   >
   > **2026-10-21 订正**：`opi_load_trad` 已桥接、`trad.opid` 已随包
   > （`scripts/prepare_harmony_dicts.sh` 的 FILES 含 trad，`DictionaryAssets.ets`
   > 拷贝并装载）；上面「未桥 / 不随包」已过期。真机繁体仍未验收，
   > 见 `docs/install/harmony.md`。

6. **未处理「抬手」事件**（`STATE_RELEASED`）。软键盘一次点击只送一次按下，
   对路由够用 —— 注意**⇧ 三态不会因此卡住**：`router.rs` 的 `handle_shift`
   明确忽略 released/repeat，⇧ 只认按下。（本节原先写的「要接物理键盘就得补抬手，
   否则 ⇧ 三态会卡住」**因果是错的**，本轮订正；`OpiEngine.ets` 的
   `STATE_RELEASED` 注释同步改了。）
   接**物理键盘**时确实要补抬手，但理由是另一条：`router.rs` 对退格/回车/空格/
   可打印键**都判 released**（按下放行则抬起也必须放行），否则宿主收到 keydown
   收不到 keyup，依赖键状态的控件会卡键。
7. **深色主题未接**（`OpiPet` 支持 `dark: true`，但 `Index.ets` 恒传 `false`，
   没读系统主题）。

   > **2026-10-20 对齐轮**：深色与长辈模式已接到键盘模式面板的两个 chip
   > （改色板 / 字号），但仍是**手动开关** —— 不读系统主题，也不持久化。
8. **Android JNI 出口在鸿蒙目标下是死重量**（约 9 MB 里的一部分），
   要拆得改 `crates/**`，不在本轮边界内。

**以下三条是 2026-09-27 读契约时发现的，加进来（前两条是本目录能修的，第三条不能）：**

9. **⇧ 的 LOCK 态到不了**（本目录缺陷）。`OpiShift.Lock` 有枚举、`Index.ets`
   有 `⇪` 的显示分支，但**没有任何路径能把状态推到 Lock**：单击走
   `router.rs` 的 `shift_tap`，它只在 `Off ↔ Single` 之间来回；进 Lock 的唯一入口是
   `shift_long_press()`，判据是 states 里的 `KEY_STATE_LONG_PRESSED` —— 而这个位
   **原先在 `OpiEngine.ets` 里漏抄了**（只抄到 `STATE_RELEASED`），且 UI 也没给 ⇧
   绑长按手势。**本轮补了常量与 `KeyboardController.tapShiftLongPress()`，
   但没绑手势**（理由写在 `Index.ets` 那个分支的注释里：ArkUI 的长按 API 本机
   无法查证，而 `KeyCap` 是字母键与整个功能行共用的 `@Builder`（5 个调用点），见 U18）。
   DevEco 上要做的是**一行**：⇧ 键绑长按 → `tapShiftLongPress()`。

   > **2026-10-20 对齐轮已补**：26 键 ⇧ 绑了
   > `LongPressGesture({ repeat: false, duration: 500 })` → `tapShiftLongPress()`
   > （`QwertyPad.ets`）。真机手势未验证。

10. **回车在「缓冲为空」时是死键**（本目录缺陷）。引擎在空缓冲时按设计返回
    **PassThrough**（「这个回车归应用」），而 `handlePassThrough` 只接退格/Delete
    与可打印段 —— `KEY_RETURN` = `0x1_000D` 大于 `0xFFFF`，两个分支都不进，
    **于是什么都不发生**。修它需要 IME Kit 的「把按键交给应用」接口
    （Android 侧对应 `InputConnection.performEditorAction`），
    **那个接口在鸿蒙叫什么本机查不到**，所以本轮只把缺口写在 `tapEnter` 的注释里，
    **没有凭记忆补一个 API 调用**。

11. **全角↔半角开关、以及符号模式的 `toggle_symbol`，在 C ABI 里根本不存在**
    （**不是本目录的问题，改不了**）。

    > ✅ **2026-09-27 已解决 —— 这是本轮唯一一条「报出去并落地」的缺口。**
    > `cabi.rs` 加了 `opi_toggle_fullwidth` / `opi_fullwidth_state` / `opi_toggle_symbol`，
    > `macos/OpiFFI.h` 补齐，**本目录也已接进桥**（见 §5d）。
    > 下面这段原文**保留不改**，作为「发现当时」的记录。
    > ⚠️ **但缺口没有完全消失** —— 能力到了桥，**UI 还没接**，见下面的 **12**。

    引擎本轮长出了 `Engine::toggle_fullwidth()` /
    `Engine::fullwidth()` / `Engine::toggle_symbol()`，但：
    ```
    $ grep -rn 'fullwidth\|toggle_symbol' crates/opi-ffi/src/
    （空）        # cabi.rs 与 jni.rs 都没有出口
    ```
    ⇒ **所有客户端（含鸿蒙、含 Android）都调不到**，`fullwidth` 只能由
    `switch_mode` 按 `Mode::default_fullwidth()` 重置（拼音/繁体 = 开，其余 = 关）。
    补出口要改 `crates/opi-ffi/**`，不在本轮边界内 —— 报给 lead 裁定。

    **后果说具体**（本轮顺着读出来的，两条都是「能力在、路不通」）：
    - Android 的 `keyboard/NumberPad.kt` 注释写着「`,` `.` 经引擎层标点表出文本
      （默认半角原样交回，**全角开关打开后**出 `，` `．`）」—— 那句括号里的状态
      **没有任何客户端能到达**。默认路径（Number 模式、全角关）出的是 ASCII `,` `.`，
      这**与鸿蒙端逐字同构**（`Index.ets` 的 `NUM_ROW_3` 有 `,` `.`，
      引擎 PassThrough，`handlePassThrough` 原样插入）—— 所以**两边都不是 bug，
      是同一个出口缺失**，别只在鸿蒙端「修」。
    - 中文标点（`。`）在鸿蒙端**出不来**：拼音模式键盘只有字母（与 Android
      `QwertyKeyboard.kt` 的三行字母 + 功能行同构），没有标点键；而能提交字面符号的
      只有符号面板 —— 那是**缺口 3**（未桥接 `opi_search_symbols` 一族）。
      也就是说引擎的标点表本身没问题，鸿蒙端缺的是**能提交它的 UI**。

12. **全角 / 符号开关：能力已经到桥，但 UI 没接**（本目录缺陷，本轮随 §5d 一起记）。
    §5d 那三条现在**从 ArkTS 可以调到**（`OpiEngine.toggleFullwidth()` /
    `fullwidthState()` / `toggleSymbol()`），但本目录里：

    - **没有任何键绑 `toggleSymbol()`** —— 模式键轮转走的是
      `KeyboardController.switchMode()`（先 `clear()` 再切），
      所以「按符号键时先把待提交的缓冲提交掉」这条路**用户按不出来**；
    - **全角指示没有接** —— `KeyboardController.publish()` 推的是
      buffer / candidates / page / pageCount / mode / shift / learner **七样**，
      **没有 fullwidth**；`pages/Index.ets` 里也没有任何显示全角状态的控件。
      所以 `toggleFullwidth()` 就算被调了，**用户也看不出状态变了**。

    这两条合起来是**同一件事**：全角/符号这套 UI 还没做（与缺口 3 的符号面板是同一轮）。
    做的时候回到 §5d 的四条契约 —— 特别是**第 1 条**：在 `publish()` 里读
    `fullwidthState()` 的位置不能只挂在「切模式之后」，否则**按符号键那一下**会显示错的指示。

13. **无中文标点开关**（`opi_chinese_punct` / `opi_set_chinese_punct` /
    `opi_toggle_chinese_punct` 三条同日新加的导出**本目录没实现** —— 不是「桥了没人调」：
    `harmony/cpp/opi_ffi.h` 里根本没有这三条声明）。
    **后果与 3/4/5 不同，别混**：中文标点表**默认开**（`engine.rs` 的 `chinese_punct: true`），
    而本目录桥的两个入口都会问到标点表（`opi_input_key` 走 `Engine::input_key`，
    `opi_key_event` 走 `router.rs` 的可打印分支）
    ⇒ **中文标点照常出得来**。缺的是**关掉它 / 读它**：用户没有任何入口把 `，` `。`
    切回半角 ASCII —— 与第 11 条末尾那句「`。` 在鸿蒙端出不来」**不是同一件事**
    （那是拼音键盘上没有标点键，引擎侧无关）。要接就按 §5 那条 `comm` 取今天的清单，
    别照本行抄。

14. **`OpiPet.ets` 只画了 5 种形态**（2026-10-18 发现）。枚举 14 个值齐全、跨端序号一致，
    但 `draw()` 只对 Sleepy / Puzzled / Waiting / Degraded 分化；其余形态**全部落回
    Idle 外观**。真源是 `shared/pet/OpiPet.kt` 的 `drawPet` / `drawPetFace`；补齐要按它
    逐形态移植，本轮没做（本轮只改表情页与数据，不动宠物绘制；若另一路正在补，以那边为准）。

    > **2026-10-18 补**：表情页「自有」已改用 `PetStickerImages.ets` 内嵌的 12 张 PNG
    > （真源 = 桌面同一套 Compose 绘制的渲染产物），**不再依赖 `OpiPet.ets` 的绘制**；
    > 本缺口现在只剩搜索「无匹配」那一处宠物插画（Puzzled，恰好是已分化的 5 种之一）。

15. **「自有」贴纸只能复制到剪贴板，不能直接插入宿主输入框**（2026-10-18）。
    IME Kit 的 `insertText` 只接受字符串，没有插入图片的接口 —— 这是平台能力边界，
    不是本端漏做。实际行为：点一格 = 把 PNG/PixelMap（2026-10-20 起优先；PNG 缺失
    才用 GIF）写进系统剪贴板（`pasteboard`）+ toast「已复制，去粘贴」，**不提交任何文本**；用户回
    宿主 App 自己粘贴。⚠️ 整条链路（`PetStickerClipboard.ets`）、`Image(dataUri)`
    的 base64/GIF 渲染**行为均未上机**（release 构建已于 2026-10-20 通过）；
    剪贴板在 IME 扩展里是否受前台/权限限制也未验证 —— 真机失败时按该文件头注的顺序
    排查（先 `setData` 的 `.catch`，再 `Image`）。

> 另有一处**不是缺口、是刻意取舍**，写在这里免得有人当 bug 修：
> `OpiEngine.ets` 与 `opi_ffi.h` 都**没有** `KEY_UP/DOWN/LEFT/RIGHT`
> （`keys.rs` 里有）。软键盘没有方向键，路由对这四个键一律 PassThrough，
> 客户端抄了也没有分支可写。接物理键盘时再补。
>
> **同理，本轮没有做那「三个热键」（`Ctrl+'` / `Ctrl+\` / `Shift+Space`）。**
> 它们的判据是「必须在客户端侧、**调引擎之前**判掉」，机制已核过：
> `router.rs` 里对 `KEY_STATE_CTRL | KEY_STATE_ALT | KEY_STATE_META` 有一条
> **前置直通**，而 `KEY_SPACE` 分支只看 `released`、**不看 Shift 位**
> ⇒ 原生 `Shift+Space` 会一路走到 `handle_space()`（提交首候选），
> 不会变成全角开关。**但本端是软键盘，没有物理键入口**，产生不了这种组合键；
> 硬造一个软键出来反而会与既有语义打架。接物理键盘时再补，判据照上面两条走。

---

### 属性设置（**未接**）

四个开关（学习 / 中文标点 / 模糊音 / 简拼）**引擎侧已支持**
（`Engine::set_learner` / `set_chinese_punct` / `set_fuzzy` / `set_jianpin`，默认全开；
门禁见 `crates/engine-core/tests/fuzzy_jianpin_switches.rs`），另两端各有配置面
（Linux = fcitx5 addon 配置 `opi_config.h` + Compose 设置窗；Windows =
`%LOCALAPPDATA%\opi\settings.ini` + 同一个设置窗）。

**C ABI 出口已落地**（2026-10-08：`opi_fuzzy_enabled` / `opi_set_fuzzy` /
`opi_jianpin_enabled` / `opi_set_jianpin`；学习与中文标点的四个出口更早就有）。
本端缺的是三件：**设置界面**（各端自己的设置页 / 首选项窗）、**落盘**
（iOS/macOS = `UserDefaults`，鸿蒙 = preferences）、**启动时把四个值套到引擎**
（对照 `tsf-opi/src/logic.rs` 的 `apply_settings` 与 fcitx5 的 `opi_config.h` +
`applyOpiSettings`）。

⚠️ **别新造第五个开关、别改语义**：真源是 `engine.rs` 的那四个 setter。

## 参考（同为「未被验证」的同类，可对照着读）

- `macos/OpiFFI.h` + `ios/README.md`：同一套 C ABI 的 Apple 侧声明面与草案写法
  （那两份的 Swift 同样一行没编译过，但 `OpiFFI.h` 至少过了 `clang -fsyntax-only`）。
- `crates/fcitx5-opi/cpp/opi_fcitx5.cpp` + 其 `README.md`：**已被真实 fcitx5 加载并测过**
  的同类适配层 —— 里面那张「7 处 API 误写对照表」就是本文件「最不确定的 API」
  这一节的由来。

---

## 2026-10-24 v1.25.0 订正：键盘版式、9 键读数与自定义短语缺口

> 本块只记**工作区源码**的现状（设计留痕，不改上文）。鸿蒙这轮改动**尚未提交**，
> 未被任何 CI 编译；上一轮 release 构建成功是 2026-10-20，不含这些工作区改动。
> 没有真机运行。下面每一条都是读源码得到的，不要读成「已验证」。

- **9 / 14 / 26 键版式（参考图改版）**：
  - 9 键：`KeyPanels.ets` 的 `NINE_ROWS` 为 3×3，右上角按**格序**标 1–9
    （`rowIndex * 3 + colIndex + 1`，分词格也是 1；大 9 键不画）。长按/上滑浮层
    `nineChoices` = 字母字号递减 + 格序数字最后，`defaultChoice = k.hint.length`
    （**默认格序数字**）；普通点击仍送 T9 数字。左列 `StripColumn` 可竖滚，
    末格「自定义」打开 `SymbolPicker.ets` 的 `CustomSymbolPicker`，选择存
    `SettingsStore.saveCustomPunc`。输入中若 `readings` 非空，左列整列换成
    `ReadingColumn`（`opi_candidate_readings` 已由 `napi_bridge.c` 的
    `JsCandidateReadings` 桥出，旧的「左列恒标点」说法已过期）。
  - 14 键：`FOURTEEN_ROWS` 前两行 `qw er ty ui op` / `as df gh jk l` 带 1–0 角标；
    第三行 `⇧ zx(~) cv(!) bn(@) m(?) ⌫`；只有第三行四枚字母对有浮层，
    `fourteenChoices` 配 `defaultChoice: 1` = **默认符号**；底行
    `符 123 , 空格 。 中英 换行`。
  - 26 键：`QwertyPad.ets` 拼音 26 无独立数字行、键面大写、Q–P 右上角 1–0、
    A/Z 行上方符号提示；英文 26 有 0–9 行、键面小写；浮层
    `[大写, 符号, 小写]`、默认中间项（拼音 Q 行中间项是角标数字）。
    五笔键面同拼音 26，第三行最左换成「拼」（走输入模式状态机回拼音 26），
    空格键面写「五笔」。
- **9 键读数修复（不再显示按键数字）**：`TopBar.ets` 的 `composingText()` 先取
  `displayBuffer`；9 键下若为空取 `readings[0]`，仍空**留空行**，绝不回退数字
  缓冲（`468` 那类）。退格/空格/回车的行为判定仍读 `buffer`，两个出口别合并。
- **自定义短语：没有 UI，也没有桥出口**：`harmony/` 全目录
  `custom_phrase` / `customPhrase` / `CustomPhrase` 零命中；`napi_bridge.c`、
  `OpiEngine.ets`、`SettingsStore.ets` 都没有任何短语相关导出。
  ⇒ 鸿蒙上既配不了、也没有可装载的表。
