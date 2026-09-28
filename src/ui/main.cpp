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

#include "common/constants.h"
#include "MainDlg.h"

// RESTYPE_PE：从 exe 自身 PE 资源加载 UI（Release 默认；与 demo 一致）
#define RESTYPE_PE 1
#define RES_TYPE RESTYPE_PE

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
    // CLI 自检模式优先（不启动 GUI / SOUI）
    if (TryCliScan(lpCmdLine)) return 0;

    // SOUI 要求 OLE 初始化（拖放/剪贴板等）
    HRESULT hRes = OleInitialize(nullptr);
    if (FAILED(hRes)) return -1;

    int nRet = 0;
    {
        SOUI::SApplication theApp(hInstance);
        SOUI::SAppCfg cfg;

        // 渲染：GDI（XP 兼容，CPU 渲染，零外部依赖）；图片解码：stb（免 GDI+/WIC）
        cfg.SetRender(SOUI::Render_Gdi)
            .SetImgDecoder(SOUI::ImgDecoder_Stb)
            .SetLog(TRUE, 2, "vpid")
            .EnableScript(FALSE);

        // 系统资源：静态库模式（LIB_CORE && LIB_SOUI_COM）下全部编入 exe
        cfg.SetSysResPeHandle(hInstance);
        // 应用资源也编入 exe（RESTYPE_PE）
        cfg.SetAppResPeHandle(hInstance);

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
            // 深色标题栏（DWM，Vista+ 动态探测；XP 无 dwmapi 自动跳过 → 经典标题栏本就可见）
            // 让系统边框/按钮在 Win10 浅色主题下也清晰可见（系统原生绘制，非自定义控件）
            {
                HMODULE hDwm = ::LoadLibraryW(L"dwmapi.dll");
                if (hDwm) {
                    typedef HRESULT(WINAPI *FDwmSetAttr)(HWND, DWORD, LPCVOID, DWORD);
                    FDwmSetAttr fn = (FDwmSetAttr)::GetProcAddress(hDwm, "DwmSetWindowAttribute");
                    if (fn) {
                        BOOL dark = TRUE;
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
        }
    }
    OleUninitialize();
    return nRet;
}
