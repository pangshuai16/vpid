// SOUI5 版应用入口（MSVC + XP 兼容）
// 资源加载全部走 PE 资源（编入 exe）：系统皮肤 + 应用 uires，保持单文件自包含
// 注意：souistd.h 必须最先包含（提供 config.h / _T / SOUI_EXP 等基础）
#include <souistd.h>

#include <windows.h>
#include <ole2.h>

#include <SApp.h>
#include <SAppCfg.h>

#include <res.mgr/SUiDef.h>

#include <cstdio>
#include <string>

#ifndef _WIN32
// 内嵌资源符号（由生成的 soui_coff_embed.c 定义；未内嵌的构建里为 0/弱符号）
// 注意：swinx 的 resapi.cpp 也以同样的弱符号方式引用它们。
extern "C" const unsigned char _binary_soui_coff_o_start[] __attribute__((weak));
#endif

#include "common/constants.h"
#include "MainDlg.h"

// 高分屏清晰度：清单已声明 DPI 感知（避免 Windows 把整窗位图拉伸放虚），
// 但 SOUI 默认缩放固定为 100（SHostWnd 内部 UM_SETSCALE 100），不会随 DPI 自动放大。
// 这里把 SOUI scale 设为 系统DPI/96×100，使窗口在高分屏上"同尺寸但更清晰"。
// XP 仅 96 DPI → 返回 100（不缩放），行为与旧版一致。
static int GetDpiScale() {
    int dpi = 96;
    // 注意：swinx 跨平台 HWND 是整数类型（UINT_PTR），用 0 表示 NULL 窗口；
    // nullptr 无法隐式转整数（C++17 报错），MSVC 原生构建则是指针类型可直接传。
    HDC hdc = ::GetDC((HWND)0);
    if (hdc) {
        int x = ::GetDeviceCaps(hdc, LOGPIXELSX);
        if (x > 0) dpi = x;
        ::ReleaseDC((HWND)0, hdc);
    }
    return dpi * 100 / 96;
}

// RESTYPE_PE：从 exe 自身 PE 资源加载 UI（Windows Release 默认；与 demo 一致）
// Linux：ELF 无 PE 资源段，swinx 的 FindResource 需要 _binary_soui_coff_o_* 符号
// （windres 机制），本项目未启用 → PE 资源加载必然失败 → SOUI 静默退出。
// 所以 Linux 改用文件资源提供者（SetSysResFile/SetAppResFile），从 exe 同目录
// 的 uires/ 加载（与 README 承诺的"Linux 从本地资源目录加载"一致）。
#define RESTYPE_PE 1
#define RES_TYPE RESTYPE_PE

// Linux：解析 exe 自身路径（swinx 实现了 GetModuleFileName，基于 /proc/self/exe），
// 返回 exe 所在目录。资源目录约定：<exe_dir>/uires/
// 注意：SOUI 在 Linux 下编译定义 _UNICODE（TCHAR=wchar_t），
// SetSysResFile/SetAppResFile 接收宽字符路径（LPCWSTR）。
static std::wstring GetExeDir() {
    wchar_t buf[MAX_PATH + 1] = {0};
    ::GetModuleFileNameW(NULL, buf, MAX_PATH);
    std::wstring path(buf);
    size_t pos = path.find_last_of(L"/\\");
    if (pos != std::wstring::npos) path = path.substr(0, pos);
    return path;
}

// Linux 诊断用窄字符版（stderr/日志文件输出）
static std::string GetExeDirA() {
    char buf[MAX_PATH + 1] = {0};
    ::GetModuleFileNameA(NULL, buf, MAX_PATH);
    std::string path(buf);
    size_t pos = path.find_last_of("/\\");
    if (pos != std::string::npos) path = path.substr(0, pos);
    return path;
}

// CLI 自检模式：vpid_viewer --scan（或 /scan）
// 跳过 GUI，直接执行 USB 扫描并打印设备列表到控制台后退出。
// 用途：① CI 构建后自动自检（验证扫描链路可用）；② 对比 CI 产物与本地
// 产物在相同机器上的扫描行为（设备数/列表）。
// GUI 程序默认无控制台，用 AttachConsole 挂到父进程控制台以便捕获输出。
static bool TryCliScan(LPTSTR lpCmdLine) {
    if (!lpCmdLine || !*lpCmdLine) return false;
    const std::wstring cmd(lpCmdLine);
    if (cmd.find(L"--scan") == std::wstring::npos &&
        cmd.find(L"/scan") == std::wstring::npos)
        return false;
#ifdef _WIN32
    // GUI 子系统的 stdout 在无控制台环境不可见，Windows 下挂父进程控制台
    if (::AttachConsole(ATTACH_PARENT_PROCESS)) {
        FILE *f = nullptr;
        freopen_s(&f, "CONOUT$", "w", stdout);
    }
#endif
    auto scanner = vpid::createScanner();
    auto devs = scanner->scan();
    // GUI 子系统的 stdout 在无控制台环境不可见，写结果文件兜底（exe 同目录）
    FILE *out = nullptr;
#ifdef _WIN32
    errno_t e = fopen_s(&out, "vpid_scan_result.txt", "w");
    if (e != 0) out = nullptr;
#else
    out = fopen("vpid_scan_result.txt", "w");
#endif
    if (out) {
        fprintf(out, "vpid scan: %zu device(s)\n", devs.size());
        for (const auto &d : devs) {
            fprintf(out, "  VID:%s PID:%s serial:%s name:%s\n",
                    d.getFormattedVid().c_str(), d.getFormattedPid().c_str(),
                    d.serial.c_str(), d.getDisplayName().c_str());
        }
        fclose(out);
    }
    printf("vpid scan: %zu device(s)\n", devs.size());
    fflush(stdout);
    return true;
}

int WINAPI _tWinMain(HINSTANCE hInstance, HINSTANCE /*hPrev*/, LPTSTR lpCmdLine, int /*nCmdShow*/) {
    // Linux 诊断：所有失败路径都写 stderr + vpid_linux_diag.txt（GUI 静默退出排查用）
    auto diag = [](const char *msg) {
        fprintf(stderr, "[vpid] %s\n", msg);
        fflush(stderr);
        FILE *f = fopen("vpid_linux_diag.txt", "a");
        if (f) { fprintf(f, "[vpid] %s\n", msg); fclose(f); }
    };
#ifndef _WIN32
    diag("start _tWinMain");
    diag(("exe dir: " + GetExeDirA()).c_str());
    diag(("uires dir: " + (GetExeDirA() + "/uires")).c_str());
    // [诊断] 宽字符路径还原 UTF-8，与窄字符路径对比（排查中文目录乱码）
    {
        std::wstring wdir = GetExeDir();
        std::string wdirUtf8;
        // 宽→窄：用 swinx 的 tostring（CP_UTF8）——这里是简单逐字节透传演示
        // 直接打印宽字符的 UTF-8 编码（每个 wchar_t 高字节为 0 则取低字节）
        std::string raw;
        for (wchar_t wc : wdir) raw += (char)(wc & 0xFF);
        diag(("wide-dir raw: " + raw).c_str());
        // 检查窄路径是否与宽路径逐字节一致
        std::string adir = GetExeDirA();
        diag(("wide==ansi: " + std::string(adir == raw ? "YES" : "NO")).c_str());
        // 检查 uires.idx 文件是否存在（窄路径）
        std::string sysIdx = adir + "/uires/soui-sys-resource/uires.idx";
        FILE *fchk = fopen(sysIdx.c_str(), "rb");
        if (fchk) { diag(("sys uires.idx EXISTS: " + sysIdx).c_str()); fclose(fchk); }
        else diag(("sys uires.idx MISSING: " + sysIdx).c_str());
        std::string appIdx = adir + "/uires/uires.idx";
        fchk = fopen(appIdx.c_str(), "rb");
        if (fchk) { diag(("app uires.idx EXISTS: " + appIdx).c_str()); fclose(fchk); }
        else diag(("app uires.idx MISSING: " + appIdx).c_str());
    }
#endif

    // CLI 自检模式优先（不启动 GUI / SOUI）
    if (TryCliScan(lpCmdLine)) return 0;

    // SOUI 要求 OLE 初始化（拖放/剪贴板等）
    HRESULT hRes = OleInitialize(nullptr);
    if (FAILED(hRes)) {
        diag("OleInitialize FAILED");
        return -1;
    }

    int nRet = 0;
    {
        SOUI::SApplication theApp(hInstance);
        SOUI::SAppCfg cfg;

        // 渲染：GDI（XP 兼容，CPU 渲染，零外部依赖）；图片解码：stb（免 GDI+/WIC）
        cfg.SetRender(SOUI::Render_Gdi)
            .SetImgDecoder(SOUI::ImgDecoder_Stb)
            .SetLog(TRUE, 2, "vpid")
            .EnableScript(FALSE);

#ifdef _WIN32
        // Windows：系统资源 + 应用资源全部编入 exe PE 资源（单文件自包含）
        cfg.SetSysResPeHandle(hInstance);
        cfg.SetAppResPeHandle(hInstance);
#else
        // Linux：优先使用"内嵌进 ELF"的 PE 资源（编译期把 .rc 编成 COFF 资源
        // 对象，作为数据嵌入 ELF；swinx resapi 通过 _binary_soui_coff_o_start/
        // _end 符号解析，二者以弱符号静态链接）→ 单二进制自包含，无需 uires/ 目录。
        // 若该符号不存在（未启用内嵌的构建），回退到 exe 同目录的 uires/ 文件提供者。
        bool embedded = (&_binary_soui_coff_o_start != NULL);
        if (embedded) {
            diag("resource mode: embedded PE (COFF blob in ELF)");
            cfg.SetSysResPeHandle(hInstance);
            cfg.SetAppResPeHandle(hInstance);
        } else {
            diag(("resource mode: file provider " + GetExeDirA() + "/uires").c_str());
            std::wstring resRoot = GetExeDir() + L"/uires";
            cfg.SetSysResFile((resRoot + L"/soui-sys-resource").c_str());
            cfg.SetAppResFile(resRoot.c_str());
        }
#endif

        if (!cfg.DoConfig(&theApp)) {
            diag("DoConfig FAILED (资源加载失败：内嵌资源缺失且 uires/ 目录无效)");
            nRet = -1;
        } else {
            diag("DoConfig OK");
            vpid::MainDlg dlg;
            // 系统边框窗口：XML wndStyle 含 WS_CAPTION（系统标题栏 47120384=0x02CF0000）
            // 客户区 = 内容区 1280x720，窗口总尺寸 = 客户区 + 系统标题栏/边框（AdjustWindowRect）
            {
                RECT rcWnd{0, 0, vpid::kDefaultWindowWidth,
                           vpid::kDefaultWindowHeight};
#ifdef _WIN32
                // AdjustWindowRect 仅 Windows 有（swinx/Linux 无此 Win32 API）
                ::AdjustWindowRect(&rcWnd, WS_OVERLAPPEDWINDOW, FALSE);
#endif
                dlg.Create(GetActiveWindow(), 0, 0,
                           rcWnd.right - rcWnd.left, rcWnd.bottom - rcWnd.top);
                if (!dlg.m_hWnd) {
                    diag("MainDlg Create FAILED (窗口创建失败)");
                    nRet = -1;
                    return nRet;
                }
                diag("MainDlg Create OK");
            }
            // [高分屏清晰] 按系统 DPI 放大 SOUI 缩放（清单已声明 DPI 感知，窗口不再被位图拉伸）。
            // 不缩放时窗口在高分屏会缩成"一半大小"。scale=100（96 DPI）时跳过，保持原尺寸。
            {
                int scale = GetDpiScale();
                if (scale != 100) {
                    RECT rcScaled{0, 0,
                        vpid::kDefaultWindowWidth * scale / 100,
                        vpid::kDefaultWindowHeight * scale / 100};
#ifdef _WIN32
                    ::AdjustWindowRect(&rcScaled, WS_OVERLAPPEDWINDOW, FALSE);
#endif
                    dlg.SetScale(scale, &rcScaled);
                }
            }
            // 系统标题栏文字（SOUI 创建时窗口名为 HOSTWND，覆盖为产品名）
            ::SetWindowTextW(dlg.m_hWnd, L"USB设备ID查看");
            // 统一窗口图标：SOUI 只管理 icon 句柄（XML bigIcon/smallIcon）但从不
            // WM_SETICON 设置到窗口，导致系统标题栏/任务栏无图标。
            // 这里显式设置：16px=标题栏/Alt+Tab 小图标，32px=任务栏大图标，
            // 均取自已完善的 app.ico（与桌面/资源管理器图标同一设计，三处一致）。
            {
                HICON hIconSm = (HICON)::LoadImageW(hInstance, MAKEINTRESOURCE(1),
                    IMAGE_ICON, 16, 16, LR_DEFAULTCOLOR);
                HICON hIconBg = (HICON)::LoadImageW(hInstance, MAKEINTRESOURCE(1),
                    IMAGE_ICON, 32, 32, LR_DEFAULTCOLOR);
                if (hIconSm) ::SendMessageW(dlg.m_hWnd, WM_SETICON, ICON_SMALL, (LPARAM)hIconSm);
                if (hIconBg) ::SendMessageW(dlg.m_hWnd, WM_SETICON, ICON_BIG, (LPARAM)hIconBg);
                // 进程退出时句柄自动释放，无需显式 DestroyIcon
            }
            // [关键] SOUI 的 ModifyStyle(0,dwStyle) 只改样式位、不触发 SWP_FRAMECHANGED，
            // 补一次让系统按 WS_CAPTION 重算非客户区（ncpainter system=true 后已走系统路径）。
            ::SetWindowPos(dlg.m_hWnd, NULL, 0, 0, 0, 0,
                           SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER);
            // 浅色标题栏：显式关掉"沉浸式深色模式"。
            // 内容区是浅色主题，若系统处于深色模式而标题栏跟着变深，
            // 会出现"深色标题栏 + 浅色内容"的割裂（系统原生绘制，非自定义控件）。
            // XP 无 dwmapi → LoadLibrary 失败自动跳过（经典标题栏本就是浅色）。
            {
                HMODULE hDwm = ::LoadLibraryW(L"dwmapi.dll");
                if (hDwm) {
                    typedef HRESULT(WINAPI *FDwmSetAttr)(HWND, DWORD, LPCVOID, DWORD);
                    FDwmSetAttr fn = (FDwmSetAttr)::GetProcAddress(hDwm, "DwmSetWindowAttribute");
                    if (fn) {
                        BOOL dark = FALSE;
                        // DWMWA_USE_IMMERSIVE_DARK_MODE：Win10 20H1+ 用 20，旧版用 19
                        if (FAILED(fn(dlg.m_hWnd, 20, &dark, sizeof(dark))))
                            fn(dlg.m_hWnd, 19, &dark, sizeof(dark));
                    }
                    ::FreeLibrary(hDwm);
                }
            }
            dlg.GetNative()->SendMessage(WM_INITDIALOG);
            dlg.CenterWindow();
            dlg.ShowWindow(SW_SHOWNORMAL);
            diag("ShowWindow OK, entering message loop");
            nRet = theApp.Run(dlg.m_hWnd);
            diag("message loop exited");
        }
    }
    OleUninitialize();
    diag("exit _tWinMain");
    return nRet;
}

#ifndef _WIN32
// ===================== Linux 标准入口 =====================
// 关键：Linux 链接器以标准 C 的 main 为入口。若只有 Win32 风格的
// _tWinMain 而无 main 调用它，链接器 GC（-ffunction-sections + LTO）
// 会把整个应用代码（含资源字符串）当死代码删除——产物启动后
// 只剩 swinx 库代码，表现为"无报错、无窗口、无进程"（用户实测现象）。
// 这里显式提供 main → _tWinMain 调用链，确保应用代码被保留。
extern "C" int main(int argc, char **argv) {
    // 组装 Win32 风格命令行（swinx 用窄/宽字符串，这里按 _tWinMain 期望）
    // _tWinMain(hInstance, hPrev, lpCmdLine, nCmdShow)
    // hInstance：Linux 下 swinx 的 GetModuleHandle(NULL) 返回有效值（dlopen 基址）
    // lpCmdLine：argv[1..] 拼接（用于 --scan 自检）
    std::string cmdline;
    for (int i = 1; i < argc; ++i) {
        if (i > 1) cmdline += " ";
        cmdline += argv[i];
    }
    HINSTANCE hInstance = ::GetModuleHandle(nullptr);
    int ret = _tWinMain(hInstance, nullptr,
#ifdef _UNICODE
        (LPTSTR)std::wstring(cmdline.begin(), cmdline.end()).c_str(),
#else
        (LPTSTR)cmdline.c_str(),
#endif
        SW_SHOWNORMAL);
    return ret;
}
#endif // !_WIN32
