import 'package:app/engine/engine_controller.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

/// 设置页（运行于 main() 应用 isolate，与 IME isolate 各自独立引擎实例——
/// M5 学习开关/清词/导出只作用于本实例，M6 SQLite 落盘后跨实例生效）。
class SettingsPage extends StatefulWidget {
  const SettingsPage({super.key, required this.controller});

  final EngineController controller;

  @override
  State<SettingsPage> createState() => _SettingsPageState();
}

class _SettingsPageState extends State<SettingsPage> {
  late bool _learner = widget.controller.learnerEnabled;

  Future<void> _clearWords() async {
    final ok = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('清除用户词库'),
        content: const Text('将删除所有学习到的用户词，确定吗？'),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('取消'),
          ),
          TextButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('清除'),
          ),
        ],
      ),
    );
    if (ok != true) return;
    widget.controller.clearUserWords();
    if (!mounted) return;
    ScaffoldMessenger.of(context)
        .showSnackBar(const SnackBar(content: Text('已清除用户词库')));
  }

  Future<void> _exportWords() async {
    final json = widget.controller.exportUserWords();
    await Clipboard.setData(ClipboardData(text: json));
    if (!mounted) return;
    ScaffoldMessenger.of(context)
        .showSnackBar(const SnackBar(content: Text('已复制到剪贴板')));
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('OPI 设置')),
      body: ListView(
        children: [
          SwitchListTile(
            title: const Text('学习'),
            subtitle: const Text('根据选词学习用户词频'),
            value: _learner,
            onChanged: (v) {
              setState(() => _learner = v);
              widget.controller.setLearner(v);
            },
          ),
          ListTile(
            title: const Text('清除用户词库'),
            onTap: _clearWords,
          ),
          ListTile(
            title: const Text('导出词库 JSON'),
            subtitle: const Text('复制到剪贴板（为云同步预留格式）'),
            onTap: _exportWords,
          ),
          Padding(
            padding: const EdgeInsets.all(16),
            child: Text(
              '注：学习/词库作用于本应用内嵌引擎实例；输入法引擎为独立实例（M6 SQLite 后统一）。',
              style: TextStyle(fontSize: 12, color: Colors.grey.shade600),
            ),
          ),
        ],
      ),
    );
  }
}
