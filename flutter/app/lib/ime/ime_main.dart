import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import 'package:app/candidates/candidate_bar.dart';
import 'package:app/engine/engine_controller.dart';
import 'package:app/ime/ime_router.dart';
import 'package:app/keyboards/number_pad.dart';
import 'package:app/keyboards/qwerty.dart';
import 'package:app/keyboards/symbol_panel.dart';
import 'package:app/platform/ime_channel.dart';
import 'package:app/src/rust/api.dart';
import 'package:app/src/rust/frb_generated.dart';

/// IME 独立 entrypoint（M4：Flutter 键盘进 IME 窗口）。
@pragma('vm:entry-point')
Future<void> imeMain() async {
  // IME 入口先于 runApp：确保 binding 就绪，rootBundle 才能读 luna 词库
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  final controller = await EngineController.load();
  runApp(ImeApp(controller: controller, channel: MethodChannelIme()));
}

class ImeApp extends StatelessWidget {
  const ImeApp({super.key, required this.controller, required this.channel});

  final EngineController controller;
  final ImeChannel channel;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      home: ImeScreen(controller: controller, channel: channel),
    );
  }
}

/// UI 视图（与引擎 ApiMode 解耦；打开面板前提交 pending buffer，无候选则保留）。
enum ImeView { qwerty, number, symbol }

class ImeScreen extends StatefulWidget {
  const ImeScreen({super.key, required this.controller, required this.channel});

  final EngineController controller;
  final ImeChannel channel;

  @override
  State<ImeScreen> createState() => _ImeScreenState();
}

class _ImeScreenState extends State<ImeScreen> {
  late final ImeRouter _router = ImeRouter(widget.controller, widget.channel);
  ImeView _view = ImeView.qwerty;

  // 符号面板搜索态：焦点联动 qwerty（IME 窗口内 TextField 无系统键盘）。
  final TextEditingController _symbolSearchCtrl = TextEditingController();
  final FocusNode _symbolSearchFocus = FocusNode();
  bool _searchActive = false;

  @override
  void initState() {
    super.initState();
    _symbolSearchFocus.addListener(_onSearchFocus);
  }

  @override
  void dispose() {
    _symbolSearchFocus.removeListener(_onSearchFocus);
    _symbolSearchCtrl.dispose();
    _symbolSearchFocus.dispose();
    super.dispose();
  }

  void _onSearchFocus() {
    if (_symbolSearchFocus.hasFocus != _searchActive) {
      setState(() => _searchActive = _symbolSearchFocus.hasFocus);
    }
  }

  void _toggleMode() {
    if (kDebugMode) {
      debugPrint('OPI toggleMode mode=${widget.controller.mode} '
          'buffer="${widget.controller.buffer}"');
    }
    if (widget.controller.mode == ApiMode.pinyin) {
      // 切英文前清残留拼音（M4 偏差记录"M4 接受，M5 处理"），
      // 防止残留拼音被空格/回车意外提交。
      widget.controller.clear();
      widget.controller.switchMode(ApiMode.english);
    } else {
      widget.controller.switchMode(ApiMode.pinyin);
    }
  }

  /// 打开面板前提交 pending 拼音（有候选选第一个提交；无候选的乱码缓冲
  /// 如 abc 清掉，避免面板往返后残留噪音）。
  void _commitPendingBuffer() {
    if (widget.controller.buffer.isEmpty) return;
    if (widget.controller.candidates.isEmpty) {
      widget.controller.clear();
      return;
    }
    final text = widget.controller.select(0);
    if (text.isNotEmpty) _router.commitText(text);
  }

  void _openNumber() {
    if (kDebugMode) debugPrint('OPI open number panel');
    _commitPendingBuffer();
    setState(() => _view = ImeView.number);
  }

  void _openSymbol() {
    if (kDebugMode) debugPrint('OPI open symbol panel');
    _commitPendingBuffer();
    setState(() => _view = ImeView.symbol);
  }

  void _backToLetters() {
    _symbolSearchFocus.unfocus();
    _symbolSearchCtrl.clear();
    setState(() => _view = ImeView.qwerty);
  }

  void _closeSearch() => _symbolSearchFocus.unfocus();

  // ---- 搜索态 qwerty 路由 ----

  void _searchKey(String ch) {
    final next = _symbolSearchCtrl.text + ch;
    _symbolSearchCtrl.text = next;
    _symbolSearchCtrl.selection = TextSelection.collapsed(offset: next.length);
  }

  void _searchSpace() => _searchKey(' ');

  void _searchBackspace() {
    final text = _symbolSearchCtrl.text;
    if (text.isEmpty) return;
    final runes = text.runes.toList();
    final next = String.fromCharCodes(runes.take(runes.length - 1));
    _symbolSearchCtrl.text = next;
    _symbolSearchCtrl.selection = TextSelection.collapsed(offset: next.length);
  }

  Widget _buildKeyboard() {
    switch (_view) {
      case ImeView.number:
        return NumberPad(
          onKey: _router.commitText,
          onSymbol: _openSymbol,
          onLetters: _backToLetters,
          onSpace: _router.handleSpace,
          onBackspace: _router.handleBackspace,
          onEnter: _router.handleEnter,
        );
      case ImeView.symbol:
        // 面板恒挂载（搜索态下搜索框+结果网格保持可见，状态不因重挂载丢失）；
        // 焦点态时下方叠 qwerty 搜索盘供键入（IME 窗口内 TextField 无系统键盘）。
        // 修 bug：此前搜索态整体替换为裸 qwerty，导致输入框/结果不可见（tester 补测）。
        return Column(
          children: [
            Expanded(
              flex: 3,
              child: SymbolPanel(
                catalog: widget.controller.symbols,
                onCommit: _router.commitText,
                onClose: _backToLetters,
                onBackToNumber: _openNumber,
                searchController: _symbolSearchCtrl,
                searchFocusNode: _symbolSearchFocus,
                searchActive: _searchActive,
              ),
            ),
            if (_searchActive)
              Expanded(
                flex: 2,
                child: QwertyKeyboard(
                  onKey: _searchKey,
                  onSpace: _searchSpace,
                  onBackspace: _searchBackspace,
                  onEnter: _closeSearch,
                  onModeSwitch: _closeSearch,
                  onNumber: _closeSearch,
                  onSymbolLongPress: _closeSearch,
                  onShift: () {},
                  onShiftLongPress: () {},
                  shiftState: ShiftState.off,
                ),
              ),
          ],
        );
      case ImeView.qwerty:
        final shiftVisible = widget.controller.mode == ApiMode.english;
        return QwertyKeyboard(
          onKey: _router.handleKey,
          onSpace: _router.handleSpace,
          onBackspace: _router.handleBackspace,
          onEnter: _router.handleEnter,
          onModeSwitch: _toggleMode,
          onNumber: _openNumber,
          onSymbolLongPress: _openSymbol,
          // pinyin 模式 ⇧ 无意义且残留状态会泄漏进 English（S1）：传 null 禁用
          onShift: shiftVisible ? widget.controller.shiftTap : null,
          onShiftLongPress:
              shiftVisible ? widget.controller.shiftLongPress : null,
          shiftState: shiftVisible ? widget.controller.shiftState : ShiftState.off,
          modeLabel: widget.controller.mode == ApiMode.pinyin ? '中' : '英',
        );
    }
  }

  @override
  Widget build(BuildContext context) {
    const bottomSafe = 48.0; // 曲面屏底部安全区（≈168px @3.5，避开圆角 r=147px），与 Java 侧一致
    // M5 动态高度：Kotlin 侧窗口高度为唯一高度源（onConfigureWindow 重入自动重建），
    // Dart 不再用 width*0.42 公式；键盘铺满 IME 窗口，仅底部避开安全区。
    return Scaffold(
      backgroundColor: Colors.white,
      body: Align(
        // IME 窗口全屏铺满，键盘必须自己贴底部（否则显示在窗口顶部）。
        alignment: Alignment.bottomCenter,
        child: ListenableBuilder(
          listenable: widget.controller,
          builder: (context, _) => Column(
            children: [
              // 候选栏：pinyin 有内容时显示；english 模式恒显示模式条（切换有明确反馈）。
              // 面板视图不显示候选栏
              if (_view == ImeView.qwerty &&
                  (widget.controller.mode == ApiMode.english ||
                      widget.controller.buffer.isNotEmpty ||
                      widget.controller.candidates.isNotEmpty))
                CandidateBar(
                  controller: widget.controller,
                  onTap: _router.handleCandidate,
                ),
              Expanded(
                child: Padding(
                  padding: EdgeInsets.only(bottom: bottomSafe),
                  child: _buildKeyboard(),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
