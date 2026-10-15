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
#include "common/trace.h"
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

// RESTYPE_PE：从 exe 自身 PE 资源加载 UI（Windows Release 默认；与 demo 一致）。
// Linux：ELF 无 PE 资源段，故编译期把 .rc 编成 COFF 资源对象内嵌进 ELF（见
// tools/gen_linux_coff_res.py），运行期仍按 PE 资源路径解析；仅当内嵌符号缺失时
// 才回退到 exe 同目录的 uires/ 文件提供者（见 _tWinMain 内的 fallback 分支）。
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
    // 只走 stdout（Windows 下已 AttachConsole 到父控制台），不落地任何文件
    auto devs = scanner->scan();
    printf("vpid scan: %zu device(s)\n", devs.size());
    for (const auto &d : devs) {
        printf("  VID:%s PID:%s serial:%s name:%s\n",
               d.getFormattedVid().c_str(), d.getFormattedPid().c_str(),
               d.serial.c_str(), d.getDisplayName().c_str());
    }
    fflush(stdout);

    return true;
}

int WINAPI _tWinMain(HINSTANCE hInstance, HINSTANCE /*hPrev*/, LPTSTR lpCmdLine, int /*nCmdShow*/) {
    // CLI 自检模式优先（不启动 GUI / SOUI）
    if (TryCliScan(lpCmdLine)) return 0;

#ifdef _WIN32
    // 退出诊断日志（XP 退出残留定位）：记录 OS 版本便于区分环境。
    // 打原始值十六进制，避免 HIWORD/LOWORD 解码在不同 SDK 下出现歧义：
    // 0x00050003=XP SP3, 0x000601xx=Win7, 0x000a00xx=Win10/11
    {
        DWORD v = ::GetVersion();
        vpidtrace::Trace(L"app start: GetVersion=" + vpidtrace::Hex(v)
                         + L" (5.1=XP 6.1=Win7 10.0=Win10/11)");
    }
#endif

    // SOUI 要求 OLE 初始化（拖放/剪贴板等）
    HRESULT hRes = OleInitialize(nullptr);
    if (FAILED(hRes)) {
        return -1;
    }

    int nRet = 0;
    {
        SOUI::SApplication theApp(hInstance);
        SOUI::SAppCfg cfg;

        // 渲染：GDI（XP 兼容，CPU 渲染，零外部依赖）；图片解码：stb（免 GDI+/WIC）
        cfg.SetRender(SOUI::Render_Gdi)
            .SetImgDecoder(SOUI::ImgDecoder_Stb)
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
            cfg.SetSysResPeHandle(hInstance);
            cfg.SetAppResPeHandle(hInstance);
        } else {
            std::wstring resRoot = GetExeDir() + L"/uires";
            cfg.SetSysResFile((resRoot + L"/soui-sys-resource").c_str());
            cfg.SetAppResFile(resRoot.c_str());
        }
#endif

        if (!cfg.DoConfig(&theApp)) {
            nRet = -1;
        } else {
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
                    nRet = -1;
                    return nRet;
                }
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
            nRet = theApp.Run(dlg.m_hWnd);
            vpidtrace::Trace(L"SApplication::Run returned (" + std::to_wstring(nRet)
                             + L") -- 消息循环已退出");

#ifdef _WIN32
            // [XP 退出残留 · 最终解] 直接 ExitProcess，跳过剩下的一切析构与 CRT 退出清理。
            //
            // XP 实测两次日志已把范围锁定：
            //   第 3 轮缺 "_tWinMain returning"，中间只有 OleUninitialize → 移除后复测，
            //   卡点不变，且日志里 clipboard 出现 0 次（进程全程未复制，剪贴板理论排除）。
            //   于是真凶是上面那个 } 触发的 ~MainDlg/~SAppCfg/~SApplication 析构：
            //   ~MainDlg 已证实完成（"join: no scan thread handle" 已打印），
            //   ~SAppCfg 只是 4 个 delete，故卡在 ~SApplication 里那串 SOUI 收尾：
            //     SResProviderMgr::RemoveAll / _DestroySingletons(SNotifyCenter/SHostMgr/
            //     SUiDef/STimerGenerator/SWindowMgr) / SRichEdit::UninitTextService /
            //     SNativeWndHelper::Uninit —— 这些都在 vendored 的 SOUI 里，只能盲猜。
            //
            // 为什么不"修好它"而直接跳过：进程反正马上终止，**没有任何资源需要我们归还**——
            //   窗口已在 OnClose 里 DestroyWindow，扫描线程已在 ShutdownScanThread 里 join，
            //   消息循环已正常退出；内存/GDI/句柄/剪贴板所有权/COM 与 OLE 状态
            //   全部会在进程终止时由操作系统回收。
            // ExitProcess 还额外跳过 C++ 静态析构与 atexit 回调，那是 XP 时代
            //   msvcrt 堆回收/TLS 清理的高风险区（exit() 会跑它们，ExitProcess 不会）。
            // ExitProcess 仍会执行各模块的 DLL_PROCESS_DETACH(fDllUnloading=TRUE)，
            //   那是所有从不调用 OleUninitialize 的 Win32 程序走的正常安全路径。
            // ExitProcess 自 NT 3.1 起就在 kernel32，XP 原生导出，非 Vista+ API。
            vpidtrace::Trace(L"calling ExitProcess(" + std::to_wstring(nRet)
                             + L") -- 跳过 ~MainDlg/~SAppCfg/~SApplication 与 CRT 退出清理");
            ::ExitProcess((UINT)nRet);
#endif
        }
    }
    vpidtrace::Trace(L"app objects destroyed (~MainDlg/~SAppCfg/~SApplication done)");

    // [XP 退出残留根因] 这里**故意不**调用 OleUninitialize()。
    //
    // XP 实测日志显示进程卡在 "SApplication::Run returned" 与 "_tWinMain returning"
    // 之间——那之间原本只有 OleUninitialize()。机制：
    //   CopySelected 走过 EmptyClipboard + SetClipboardData，本进程因此成为
    //   **剪贴板所有者**。OleUninitialize 收尾时会调用 ReleaseThreadClipboard
    //   释放本线程的剪贴板锁；而本线程拥有剪贴板，于是它要给剪贴板所有者窗口
    //   投递/发送 WM_DESTROYCLIPBOARD，并**等待该消息被处理**。
    //   但此刻消息循环早已退出（SApplication::Run 已返回）、窗口也已销毁，
    //   永远没人处理这条消息 → 死锁，表现为"窗口关了、进程残留、内存逐渐变小"。
    //   Win10/11 的 OLE/剪贴板实现不走这条等待路径，所以只在 XP 复现。
    //
    // 正解：进程马上就退出了，OLE 初始化引用计数无需平衡；OLE 状态与本线程的
    //   剪贴板所有权都会在进程终止时由操作系统直接回收，**不需要**本线程收尾。
    //   绝大多数 Win32 程序本来就不调用 OleUninitialize。
    //   （想"正确"地调用，它必须放在消息循环仍活着、窗口仍存在之时，即
    //   MainDlg::OnClose 里 DestroyWindow 之前——但那会引入"自投递消息自等待"
    //   的风险，收益为 0，故直接不调。）
    vpidtrace::Trace(L"_tWinMain returning rc=" + std::to_wstring(nRet)
                     + L" -- 此后若进程不消失，则是 CRT/static 析构或 DLL_PROCESS_DETACH 阶段");

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
