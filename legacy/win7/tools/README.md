# Win7 legacy tools

`check-imports.py` is the Win8+/Win10 import gate. Run it on every produced PE
before packaging:

```
python3 check-imports.py <file> --arch x86_64|i686 [--require-exports]
```

Local Linux linking: use `cargo xwin build` (licensed Microsoft CRT). Phase 0
linked with Wine export libs plus a hand-written CRT stub
(`/home/erik/tmp/win7spike`); **those stubs are not committed and must never be
shipped**. A stub CRT can hide a missing `/MT` symbol, so any local stub-based
link is evidence only for the import table, not for runtime correctness.

CI (`windows-legacy.yml`) links with the real MSVC CRT on `windows-latest`;
x86 is built there too (cargo-xwin 0.23.1 did not download the x86 SDK libs on
the Linux dev host).
