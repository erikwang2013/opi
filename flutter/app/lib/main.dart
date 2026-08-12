import 'package:flutter/material.dart';
import 'package:app/src/rust/frb_generated.dart';

Future<void> main() async {
  await RustLib.init();
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      home: Scaffold(
        appBar: AppBar(title: const Text('OPI IME')),
        body: Center(
          child: Text(
            'Engine bindings ready (OPI IME).\nUI lands in a later milestone.',
          ),
        ),
      ),
    );
  }
}
