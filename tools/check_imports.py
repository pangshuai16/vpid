#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Check PE import table for Vista+ APIs."""
import struct
import sys

def read_pe_imports(filepath):
    with open(filepath, 'rb') as f:
        data = f.read()

    # DOS header
    if data[:2] != b'MZ':
        print("Not a valid PE file")
        return

    e_lfanew = struct.unpack_from('<I', data, 0x3C)[0]

    # PE signature
    if data[e_lfanew:e_lfanew+4] != b'PE\x00\x00':
        print("Invalid PE signature")
        return

    # COFF header
    machine = struct.unpack_from('<H', data, e_lfanew + 4)[0]
    num_sections = struct.unpack_from('<H', data, e_lfanew + 6)[0]
    opt_hdr_size = struct.unpack_from('<H', data, e_lfanew + 20)[0]

    opt_hdr_offset = e_lfanew + 24

    # Magic (PE32 = 0x10b, PE32+ = 0x20b)
    magic = struct.unpack_from('<H', data, opt_hdr_offset)[0]

    if magic == 0x10b:  # PE32
        import_dir_rva = struct.unpack_from('<I', data, opt_hdr_offset + 104)[0]
        import_dir_size = struct.unpack_from('<I', data, opt_hdr_offset + 108)[0]
    elif magic == 0x20b:  # PE32+
        import_dir_rva = struct.unpack_from('<I', data, opt_hdr_offset + 120)[0]
        import_dir_size = struct.unpack_from('<I', data, opt_hdr_offset + 128)[0]
    else:
        print(f"Unknown PE magic: {magic:#x}")
        return

    # Section headers
    sections = []
    sec_offset = opt_hdr_offset + opt_hdr_size
    for i in range(num_sections):
        name = data[sec_offset:sec_offset+8].rstrip(b'\x00').decode('ascii', errors='replace')
        vsize = struct.unpack_from('<I', data, sec_offset + 8)[0]
        vaddr = struct.unpack_from('<I', data, sec_offset + 12)[0]
        raw_size = struct.unpack_from('<I', data, sec_offset + 16)[0]
        raw_ptr = struct.unpack_from('<I', data, sec_offset + 20)[0]
        sections.append({'name': name, 'vaddr': vaddr, 'vsize': vsize, 'raw_ptr': raw_ptr, 'raw_size': raw_size})
        sec_offset += 40

    def rva_to_offset(rva):
        for s in sections:
            if s['vaddr'] <= rva < s['vaddr'] + max(s['vsize'], s['raw_size']):
                return rva - s['vaddr'] + s['raw_ptr']
        return None

    # Read import directory
    all_imports = {}
    offset = rva_to_offset(import_dir_rva)
    if offset is None:
        print("Cannot resolve import directory RVA")
        return

    while True:
        ilt_rva = struct.unpack_from('<I', data, offset)[0]
        time_stamp = struct.unpack_from('<I', data, offset + 4)[0]
        forwarder = struct.unpack_from('<I', data, offset + 8)[0]
        name_rva = struct.unpack_from('<I', data, offset + 12)[0]
        iat_rva = struct.unpack_from('<I', data, offset + 16)[0]

        if ilt_rva == 0 and name_rva == 0:
            break

        # Read DLL name
        name_offset = rva_to_offset(name_rva)
        if name_offset is not None:
            dll_name = b''
            p = name_offset
            while data[p] != 0:
                dll_name += bytes([data[p]])
                p += 1
            dll_name = dll_name.decode('ascii', errors='replace')
        else:
            dll_name = "???"

        # Read import names from ILT
        funcs = []
        ilt_offset = rva_to_offset(ilt_rva)
        if ilt_offset is not None:
            p = ilt_offset
            while True:
                entry = struct.unpack_from('<I', data, p)[0]
                if entry == 0:
                    break
                if entry & 0x80000000:  # Import by ordinal
                    funcs.append(f"ordinal_{entry & 0xFFFF}")
                else:
                    hint_name_rva = entry & 0x7FFFFFFF
                    hn_offset = rva_to_offset(hint_name_rva)
                    if hn_offset is not None:
                        hint = struct.unpack_from('<H', data, hn_offset)[0]
                        fname = b''
                        fp = hn_offset + 2
                        while data[fp] != 0:
                            fname += bytes([data[fp]])
                            fp += 1
                        funcs.append(fname.decode('ascii', errors='replace'))
                    else:
                        funcs.append("???")
                p += 4

        all_imports[dll_name] = funcs
        offset += 20

    return all_imports

# Vista+ APIs that are problematic for XP
VISTA_PLUS_APIS = {
    'GetFileSizeEx',
    'SetFilePointerEx',
    'CompareStringEx',
    'GetLocaleInfoEx',
    'GetThreadUILanguage',
    'InitializeCriticalSectionEx',
    'CreateFileExW',
    'GetTempPathExW',
    'FindFirstFileExW',
    'FlsAlloc',
    'FlsFree',
    'FlsGetValue',
    'FlsSetValue',
    'EncodePointer',
    'DecodePointer',
    'GetTickCount64',
    'GetTickCount32',
    ' RaiseException',
    'InitializeSListHead',
    'InterlockedExchange',
    'InterlockedIncrement',
    'InterlockedDecrement',
    'InterlockedPushEntrySList',
    'InterlockedPopEntrySList',
    'IsDebuggerPresent',
    'TerminateProcess',
    'GetCurrentProcess',
    'GetCurrentThread',
    'GetLastError',
    'SetLastError',
    'GetProcAddress',
    'LoadLibraryW',
    'GetModuleHandleW',
    'FreeLibrary',
    'ExitProcess',
    'ExitThread',
    'GetExitCodeProcess',
    'WaitForSingleObject',
    'CloseHandle',
    'GetSystemTimeAsFileTime',
    'GetCurrentProcessId',
    'GetCurrentThreadId',
    'GetSystemInfo',
    'HeapAlloc',
    'HeapFree',
    'HeapCreate',
    'HeapDestroy',
    'HeapSize',
    'HeapReAlloc',
    'HeapValidate',
    'GetProcessHeap',
    'RtlUnwind',
    'QueryPerformanceCounter',
    'UnhandledExceptionFilter',
    'SetUnhandledExceptionFilter',
    'TerminateProcess',
    'IsProcessorFeaturePresent',
}

# Only the truly fatal ones for XP
FATAL_VISTA_APIS = {
    'GetFileSizeEx',
    'SetFilePointerEx',
    'CompareStringEx',
    'GetLocaleInfoEx',
    'GetThreadUILanguage',
    'InitializeCriticalSectionEx',
    'FlsAlloc',
    'FlsFree',
    'FlsGetValue',
    'FlsSetValue',
    'GetTickCount64',
    'GetTempPathExW',
    'FindFirstFileExW',
    'CreateFileExW',
}

def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <pe_file>")
        sys.exit(1)

    filepath = sys.argv[1]
    imports = read_pe_imports(filepath)

    if not imports:
        print("No imports found or failed to read PE")
        sys.exit(1)

    print(f"\n=== Import Table for {filepath} ===\n")

    all_func_names = set()
    for dll, funcs in sorted(imports.items()):
        print(f"{dll} ({len(funcs)} imports):")
        for f in funcs:
            print(f"  {f}")
            # Extract function name (strip ordinal_ prefix if any)
            if not f.startswith("ordinal_"):
                all_func_names.add(f)
        print()

    # Check for Vista+ APIs
    print("=== XP Compatibility Analysis ===\n")

    fatal_found = []
    for api in sorted(FATAL_VISTA_APIS):
        if api in all_func_names:
            fatal_found.append(api)

    if fatal_found:
        print(f"FATAL Vista+ APIs found ({len(fatal_found)}):")
        for api in fatal_found:
            # Find which DLL
            for dll, funcs in imports.items():
                if api in funcs:
                    print(f"  {api} (from {dll})")
                    break
    else:
        print("No fatal Vista+ APIs found! PE should be XP-compatible.")

    # Check all Vista+ APIs (non-fatal too)
    all_vista = []
    for api in sorted(all_func_names):
        if api in FATAL_VISTA_APIS or api in VISTA_PLUS_APIS:
            if api not in fatal_found:
                all_vista.append(api)

    if all_vista:
        print(f"\nOther potentially Vista+ APIs ({len(all_vista)}):")
        for api in all_vista:
            for dll, funcs in imports.items():
                if api in funcs:
                    print(f"  {api} (from {dll})")
                    break

    # Summary
    print(f"\n=== Summary ===")
    print(f"Total DLLs: {len(imports)}")
    print(f"Total imports: {sum(len(v) for v in imports.values())}")
    print(f"Fatal Vista+ APIs: {len(fatal_found)}")
    if fatal_found:
        print(f"  -> {', '.join(fatal_found)}")

if __name__ == '__main__':
    main()
