// 退出/扫描诊断日志（定位"XP 上点退出后进程残留"用）
//
// 为什么需要它：XP 上点退出后进程残留、而内存占用逐渐变小 —— 说明窗口已销毁、
// 主线程已退出消息循环，但进程仍然活着。Windows 的 ExitProcess 会等**所有线程**
// 结束，所以"还有活线程"和"主线程卡在 CRT 退出清理"都会表现为残留，两者症状完全
// 相同，只有把退出序列打出来才能区分。
//
// 写到 %TEMP%\vpid_exit.log（追加）。默认开启；设环境变量 VPID_NOTRACE=1 关闭。
// 全程只用最古老的 Win32 API（GetTempPathW / CreateFileW / WriteFile /
// SetFilePointer / QueryPerformanceCounter / GetEnvironmentVariableW），
// 均为 NT 3.1 / Win95 时代就有，无任何 Vista+ 导入，XP 安全。
// 线程安全：每次调用独立 open/write/close（本程序整个生命周期只写几十行，开销可忽略）。
//
// 跨平台：实现只在 _WIN32 下生效（该日志专为 XP 退出残留设计）；
// 其它平台退化为 no-op，但**保持同一套 API 签名**，使调用方无需 #ifdef。
// 注意：调用方使用的 DWORD/DWORD64/HANDLE 等类型在此文件内自行 typedef，
// 不依赖 windows.h 在其它平台可见。
#ifndef VPID_COMMON_TRACE_H
#define VPID_COMMON_TRACE_H

#include <cstdint>
#include <string>

#ifdef _WIN32
#include <windows.h>
#endif

namespace vpidtrace {

// 调用方会用到的 Windows 类型别名（非 Windows 平台给出等宽替代，保证编译通过）
#ifdef _WIN32
typedef DWORD VpidDword;
typedef DWORD64 VpidQword;
#else
typedef unsigned int VpidDword;
typedef uint64_t VpidQword;
#endif

// 进程启动以来的毫秒数
#ifdef _WIN32
inline VpidQword NowMs() {
    static LARGE_INTEGER freq = {0};
    if (freq.QuadPart == 0) ::QueryPerformanceFrequency(&freq);
    LARGE_INTEGER cnt = {0};
    ::QueryPerformanceCounter(&cnt);
    return (freq.QuadPart != 0) ? (VpidQword)cnt.QuadPart * 1000 / freq.QuadPart : 0;
}
#else
inline VpidQword NowMs() { return 0; }
#endif

// 无 CRT 格式化的十六进制（避免依赖 swprintf 之类，纯位运算）
inline std::wstring Hex(VpidDword v) {
    static const wchar_t digits[] = L"0123456789abcdef";
    std::wstring s = L"0x";
    for (int i = 28; i >= 0; i -= 4) s += digits[(v >> i) & 0xF];
    return s;
}

#ifdef _WIN32

inline bool Enabled() {
    // 用 volatile LONG + 检查即可：本程序只有主线程会首次进入，竞态无害。
    static volatile LONG cached = -1;
    LONG c = cached;
    if (c != -1) return (c == 1);
    wchar_t buf[8] = {0};
    DWORD n = ::GetEnvironmentVariableW(L"VPID_NOTRACE", buf, 8);
    bool on = (n == 0 || n >= 8 || buf[0] != L'1');
    cached = on ? 1 : 0;
    return on;
}

inline void Trace(const std::wstring& line) {
    if (!Enabled()) return;
    wchar_t tmp[MAX_PATH];
    DWORD n = ::GetTempPathW(MAX_PATH, tmp);
    if (n == 0 || n >= MAX_PATH) return;
    std::wstring path(tmp, tmp + n);
    path += L"vpid_exit.log";

    HANDLE h = ::CreateFileW(path.c_str(), GENERIC_WRITE,
                             FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr,
                             OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (h == INVALID_HANDLE_VALUE) return;
    ::SetFilePointer(h, 0, nullptr, FILE_END);

    std::wstring out = L"[";
    out += std::to_wstring(NowMs());
    out += L"ms] ";
    out += line;
    out += L"\r\n";

    DWORD wrote = 0;
    ::WriteFile(h, out.c_str(), (DWORD)(out.size() * sizeof(wchar_t)), &wrote, nullptr);
    ::CloseHandle(h);
}

#else  // !_WIN32：非 Windows 平台为 no-op，保留签名以免调用方加 #ifdef

inline bool Enabled() { return false; }
inline void Trace(const std::wstring& /*line*/) {}

#endif  // _WIN32

}  // namespace vpidtrace

#endif  // VPID_COMMON_TRACE_H
