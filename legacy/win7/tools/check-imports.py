#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
"""Win7/8 import gate for the legacy DLL/binaries.

Parses the PE import and delay-import directories (no objdump/dumpbin needed,
works on Linux and Windows) and fails on DLLs/symbols that do not exist on
Windows 7 SP1.

Why a gate: a single static import of a Win8/Win10 API makes the whole DLL fail
to load with "procedure entry point not found", before any of our code runs.
That class of bug is invisible to `cargo check` and to a Windows 10 CI runner.

Usage:
    python3 check-imports.py FILE.dll --arch x86_64 [--require-exports]

Exit code 0 = clean, 1 = forbidden import / missing export / bad image.
"""

import argparse
import struct
import sys

# DLLs that are not present on a stock Windows 7 SP1 (or that indicate a
# dynamic CRT, which the legacy build must not use).
FORBIDDEN_DLLS = {
    "combase.dll",          # Win8+; windows-rs 0.62 pulls this in
    "bcryptprimitives.dll", # Win8+; ProcessPrng lives here
    "api-ms-win-core-path-l1-1-0.dll",       # Win10 API set
    "api-ms-win-core-synch-l1-2-0.dll",      # Win8 API set (WaitOnAddress)
    "ucrtbase.dll",         # dynamic UCRT: needs KB2999226, not /MT
    "vcruntime140.dll",
    "vcruntime140_1.dll",
    "msvcp140.dll",
    "concrt140.dll",
}

# Symbols introduced after Windows 7 SP1. Kept as bare names (matched in any
# DLL) because forwarders move between kernel32/kernelbase/ntdll.
FORBIDDEN_SYMBOLS = {
    "WaitOnAddress",
    "WakeByAddressSingle",
    "WakeByAddressAll",
    "ProcessPrng",
    "GetDpiForWindow",
    "GetDpiForSystem",
    "GetDpiForMonitor",
    "GetSystemMetricsForDpi",
    "AdjustWindowRectExForDpi",
    "SetProcessDpiAwarenessContext",
    "SetThreadDpiAwarenessContext",
    "EnableNonClientDpiScaling",
    "SetThreadDescription",
    "GetThreadDescription",
    "GetSystemTimePreciseAsFileTime",
    "CreateFile2",
    "GetPackageFamilyName",
    "GetCurrentPackageFullName",
}

# Every registration path needs all four; a missing export means regsvr32 or
# COM silently does nothing.
REQUIRED_EXPORTS = {
    "DllGetClassObject",
    "DllCanUnloadNow",
    "DllRegisterServer",
    "DllUnregisterServer",
}

MSVCRT_ALLOWED = {"atexit"}  # Phase 0 saw exactly this; revisit on real /MT.

MACHINES = {0x8664: "x86_64", 0x014C: "i686"}


class PeError(Exception):
    pass


def u16(b, o):
    return struct.unpack_from("<H", b, o)[0]


def u32(b, o):
    return struct.unpack_from("<I", b, o)[0]


def u64(b, o):
    return struct.unpack_from("<Q", b, o)[0]


class Pe:
    def __init__(self, data):
        self.b = data
        if data[:2] != b"MZ":
            raise PeError("not a PE file (no MZ header)")
        pe = u32(data, 0x3C)
        if data[pe:pe + 4] != b"PE\0\0":
            raise PeError("not a PE file (no PE signature)")
        coff = pe + 4
        self.machine = u16(data, coff)
        nsec = u16(data, coff + 2)
        opt_size = u16(data, coff + 16)
        opt = coff + 20
        magic = u16(data, opt)
        if magic == 0x20B:
            self.pe32plus = True
            self.image_base = u64(data, opt + 24)
        elif magic == 0x10B:
            self.pe32plus = False
            self.image_base = u32(data, opt + 28)
        else:
            raise PeError(f"unknown optional-header magic 0x{magic:x}")
        # Data directory count sits before the directory array.
        ndirs = u32(data, opt + (108 if self.pe32plus else 92))
        dirs = opt + (112 if self.pe32plus else 96)
        self.dirs = [struct.unpack_from("<II", data, dirs + 8 * i) for i in range(ndirs)]
        sec = opt + opt_size
        self.sections = []
        for i in range(nsec):
            o = sec + 40 * i
            vsize, va, rawsize, rawptr = struct.unpack_from("<IIII", data, o + 8)
            self.sections.append((va, max(vsize, rawsize), rawptr, rawsize))

    def off(self, rva):
        for va, vsize, rawptr, rawsize in self.sections:
            if va <= rva < va + vsize:
                delta = rva - va
                if delta >= rawsize:
                    raise PeError(f"RVA 0x{rva:x} is uninitialized data")
                return rawptr + delta
        raise PeError(f"RVA 0x{rva:x} is not in any section")

    def cstr(self, rva):
        o = self.off(rva)
        end = self.b.index(b"\0", o)
        return self.b[o:end].decode("ascii", "replace")

    def imports(self):
        """[(dll, [symbol|'#ordinal'])] for normal + delay-load imports."""
        out = []
        if len(self.dirs) > 1 and self.dirs[1][0]:
            out.extend(self._imports_at(*self.dirs[1]))
        if len(self.dirs) > 13 and self.dirs[13][0]:
            out.extend(self._delay_imports_at(*self.dirs[13]))
        return out

    def _imports_at(self, rva, _size):
        thunk_off = self.off(rva)
        out = []
        while True:
            oft, _ts, _fwd, name_rva, ft = struct.unpack_from("<IIIII", self.b, thunk_off)
            if not (oft or name_rva or ft):
                break
            dll = self.cstr(name_rva)
            table = oft or ft
            out.append((dll, self._thunks(table)))
            thunk_off += 20
        return out

    def _delay_imports_at(self, rva, _size):
        desc_off = self.off(rva)
        out = []
        while True:
            (attrs, name_rva, _hmod, _iat, int_rva, _bound, _unload,
             _ts) = struct.unpack_from("<IIIIIIII", self.b, desc_off)
            if not (name_rva or int_rva):
                break
            # Bit 0 = relative VA instead of RVA (old Borland-style); Rust/MSVC
            # emit RVAs, so reject the other form rather than misparse it.
            if attrs & 1:
                raise PeError("delay-import descriptor uses VA-relative fields")
            out.append((self.cstr(name_rva), self._thunks(int_rva)))
            desc_off += 32
        return out

    def _thunks(self, rva):
        names = []
        o = self.off(rva)
        step = 8 if self.pe32plus else 4
        read = u64 if self.pe32plus else u32
        high = 1 << (63 if self.pe32plus else 31)
        while True:
            v = read(self.b, o)
            if v == 0:
                break
            if v & high:
                names.append(f"#{v & 0xFFFF}")
            else:
                names.append(self.cstr(v + 2))  # skip the 2-byte hint
            o += step
        return names

    def exports(self):
        if not self.dirs or not self.dirs[0][0]:
            return set()
        o = self.off(self.dirs[0][0])
        n_names = u32(self.b, o + 24)
        names_rva = u32(self.b, o + 32)
        names_off = self.off(names_rva)
        return {self.cstr(u32(self.b, names_off + 4 * i)) for i in range(n_names)}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("file")
    ap.add_argument("--arch", choices=["x86_64", "i686"], required=True)
    ap.add_argument("--require-exports", action="store_true")
    args = ap.parse_args()

    try:
        pe = Pe(open(args.file, "rb").read())
        got_arch = MACHINES.get(pe.machine)
        if got_arch != args.arch:
            raise PeError(f"machine is {got_arch or hex(pe.machine)}, expected {args.arch}")
        imports = pe.imports()
        exports = pe.exports()
    except (OSError, PeError, struct.error, ValueError) as e:
        print(f"::error::{args.file}: {e}", file=sys.stderr)
        return 1

    errors = []
    for dll, symbols in imports:
        low = dll.lower()
        if low in FORBIDDEN_DLLS:
            errors.append(f"forbidden DLL {dll}")
        for s in symbols:
            if s in FORBIDDEN_SYMBOLS:
                errors.append(f"forbidden symbol {dll}!{s}")
            if low == "msvcrt.dll" and s not in MSVCRT_ALLOWED and not s.startswith("#"):
                errors.append(
                    f"msvcrt.dll imports {s} (static /MT must not pull the dynamic CRT)"
                )

    if args.require_exports:
        missing = REQUIRED_EXPORTS - exports
        if missing:
            errors.append("missing exports: " + ", ".join(sorted(missing)))

    for dll, symbols in sorted(imports, key=lambda x: x[0].lower()):
        print(f"{dll}: {len(symbols)} imports")
        for s in sorted(symbols):
            print(f"  {s}")
    print(f"exports: {len(exports)}")
    if errors:
        for e in errors:
            print(f"::error::{args.file}: {e}", file=sys.stderr)
        return 1
    print(f"OK: {args.file} has no Win8+ imports")
    return 0


if __name__ == "__main__":
    sys.exit(main())
