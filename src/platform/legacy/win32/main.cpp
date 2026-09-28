// Windows 原生 Win32 版本（无 UI 框架）
// 兼容 Windows XP (x86)：仅 Win32 API + Common Controls，静态链接。
// 复用 core/（USB 枚举、比对、剪贴板文本生成）。
//
// 布局：
//   [顶栏 50px] 设备数 label + [停止/自动刷新][重置][手动刷新][复制]
//   [主区]      左(60%) 全部设备列表 | 右(40%) 上=+新增 / 下=-移除
//   [底栏 22px] 状态(字段0) + 基准信息(字段1)

#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0501
#endif
#ifndef WINVER
#define WINVER 0x0501
#endif
#define _WIN32_IE 0x0501   // Common Controls v5，XP 兼容
#ifndef UNICODE
#define UNICODE
#endif
#ifndef _UNICODE
#define _UNICODE
#endif

#include <windows.h>
#include <commctrl.h>
#include <dbt.h>
#include <string>
#include <vector>
#include <thread>
#include <atomic>
#include <algorithm>
#include <cstring>
#include <cstdio>
#include <cwchar>

#include "common/constants.h"
#include "core/device_scanner.h"
#include "core/device_comparer.h"

#if defined(USB_DEVICE_INTERFACE_CLASS_GUID) && defined(__MINGW32__)
  // (保留空的兼容分支，避免未使用告警)
#endif

// ---- 控件/消息 ID ----
namespace ids {
const int kMessageScanDone = WM_APP + 1;
const int kTimerAuto = 1;

const int kBtnStopRefresh = 2001;
const int kBtnAutoRefresh = 2002;
const int kBtnManualRefresh = 2003;
const int kBtnBaseline = 2004;
const int kBtnCopy = 2005;

const int kHeaderDeviceCount = 2100;
const int kHeaderAdded = 2101;
const int kHeaderRemoved = 2102;
const int kStatusA = 2103;
const int kStatusB = 2104;
}

static HINSTANCE g_hInst = nullptr;

namespace theme {
// Apple HIG 语义色（浅色外观）
COLORREF primary(){ return RGB(0x00,0x7A,0xFF); }        // systemBlue
COLORREF primaryHover(){ return RGB(0x0A,0x84,0xFF); }   // systemBlue 高亮
COLORREF success(){ return RGB(0x34,0xC7,0x59); }        // systemGreen
COLORREF successBg(){ return RGB(0xE9,0xF9,0xEF); }
COLORREF danger(){ return RGB(0xFF,0x3B,0x30); }         // systemRed
COLORREF dangerBg(){ return RGB(0xFD,0xEC,0xEC); }
COLORREF text(){ return RGB(0x1D,0x1D,0x1F); }
COLORREF textSecondary(){ return RGB(0x86,0x86,0x8B); }
COLORREF border(){ return RGB(0xD1,0xD1,0xD6); }         // separator
COLORREF bg(){ return RGB(0xF5,0xF5,0xF7); }             // window backgroundColor
COLORREF white(){ return RGB(0xFF,0xFF,0xFF); }
COLORREF rowEven(){ return RGB(0xFA,0xFA,0xFC); }        // 极浅斑马纹
COLORREF primaryDark(){ return RGB(0x00,0x59,0xD6); }    // systemBlue 深
COLORREF controlFill(){ return RGB(0xF7,0xF7,0xF7); }          // 浅灰按钮 rest
COLORREF controlBorder(){ return RGB(0x8A,0x88,0x86); }        // 按钮描边
COLORREF controlHover(){ return RGB(0xF0,0xF0,0xF0); }
}

namespace vpid {

// 向白/黑方向偏移颜色（按钮 hover/press）
static COLORREF shade(COLORREF c, int delta) {
    int r = GetRValue(c) + delta, g = GetGValue(c) + delta, b = GetBValue(c) + delta;
    r = r < 0 ? 0 : (r > 255 ? 255 : r);
    g = g < 0 ? 0 : (g > 255 ? 255 : g);
    b = b < 0 ? 0 : (b > 255 ? 255 : b);
    return RGB((BYTE)r, (BYTE)g, (BYTE)b);
}

// Vista+ 高分屏锐化（XP 无此函数，探测失败则静默跳过）
static void enableHighDPI() {
    typedef BOOL (WINAPI *Fn)(void);
    HMODULE m = GetModuleHandleW(L"user32.dll");
    if (!m) return;
    Fn f = (Fn)GetProcAddress(m, "SetProcessDPIAware");
    if (f) f();
}

// 等宽字体：数据列（VID/PID 固定 4 位）用等宽更对齐。优先 Consolas，XP 不内置则回退 Courier New。
static int CALLBACK monoEnumProc(const LOGFONTW*, const TEXTMETRICW*, DWORD, LPARAM lp) {
    *(BOOL*)lp = TRUE; return 0; // 找到即停止
}
static bool fontExists(const wchar_t* name) {
    LOGFONTW lf; memset(&lf, 0, sizeof(lf));
    lf.lfCharSet = DEFAULT_CHARSET;
    wcscpy(lf.lfFaceName, name);
    BOOL found = FALSE;
    HDC dc = GetDC(nullptr);
    if (dc) { EnumFontFamiliesExW(dc, &lf, monoEnumProc, (LPARAM)&found, 0); ReleaseDC(nullptr, dc); }
    return found != FALSE;
}
static HFONT makeMonoFont(int height) {
    const wchar_t* face = fontExists(L"Consolas") ? L"Consolas" : L"Courier New";
    return CreateFontW(height, 0, 0, 0, FW_NORMAL, 0, 0, 0,
                       DEFAULT_CHARSET, 0, 0, CLEARTYPE_QUALITY, 0, face);
}

struct App;

// ---------- 小工具 ----------
static std::wstring toW(const std::string& s) {
    if (s.empty()) return std::wstring();
    int need = MultiByteToWideChar(CP_UTF8, 0, s.c_str(), (int)s.size(), nullptr, 0);
    std::wstring out((size_t)need, L'\0');
    MultiByteToWideChar(CP_UTF8, 0, s.c_str(), (int)s.size(), &out[0], need);
    return out;
}
static void setWndText(HWND h, const std::wstring& s) { SetWindowTextW(h, s.c_str()); }
static std::wstring nowTime() {
    SYSTEMTIME st; GetLocalTime(&st);
    wchar_t b[32];
    swprintf(b, 32, L"%02d:%02d:%02d", st.wHour, st.wMinute, st.wSecond);
    return b;
}
static void sortByPidVidName(std::vector<USBDevice>& v) {
    std::stable_sort(v.begin(), v.end(),
        [](const USBDevice& a, const USBDevice& b) {
            int c = a.getFormattedPid().compare(b.getFormattedPid());
            if (c != 0) return c < 0;
            c = a.getFormattedVid().compare(b.getFormattedVid());
            if (c != 0) return c < 0;
            return a.getDisplayName() < b.getDisplayName();
        });
}
static COLORREF btnFill(int id) {
    switch (id) {
        case ids::kBtnStopRefresh: return RGB(0xD6,0x45,0x41); // 深红（白字可读）
        case ids::kBtnManualRefresh:
        case ids::kBtnCopy:        return theme::primary();
        default:                   return RGB(0x1E,0x9E,0x4B); // 深绿（白字可读）
    }
}
static COLORREF btnBorder(int id) {
    switch (id) {
        case ids::kBtnStopRefresh: return RGB(0xB0,0x3A,0x35);
        case ids::kBtnManualRefresh:
        case ids::kBtnCopy:        return theme::primaryDark();
        default:                   return RGB(0x17,0x83,0x37);
    }
}

// ---------- App 状态 ----------
struct App {
    HWND hwnd = 0;
    HWND headerCount = 0;
    HWND cmdButtons[5] = {0};
    HFONT hFontBtns = 0;
    HFONT hFontList = 0;
    HFONT hFontTitle = 0;
    bool btnHover[5] = {false};
    HWND listAll = 0;
    HWND headerAdded = 0, headerRemoved = 0;
    HWND listAdded = 0, listRemoved = 0;
    HWND statusA = 0, statusB = 0;
    HBRUSH brAddedBg = 0, brRemovedBg = 0;

    std::vector<USBDevice> allDev;
    std::vector<USBDevice> curDev;
    std::vector<USBDevice> addedDev, removedDev;
    std::vector<USBDevice> baseline;

    std::atomic<bool> scanning{false};
    std::atomic<bool> closing{false};
    bool autoRefresh = true;
    HDEVNOTIFY hDevNotify = 0;
    bool ignoreSelChange = false;
};

static App* g = nullptr;

// 按钮子类化：实现悬停(WM_MOUSEMOVE/TrackMouseEvent)与离开(WM_MOUSELEAVE)
static WNDPROC g_btnOldProc = nullptr;
static void trackButtonEnter(HWND btn) {
    TRACKMOUSEEVENT tme;
    memset(&tme, 0, sizeof(tme));
    tme.cbSize = sizeof(tme);
    tme.dwFlags = TME_LEAVE;
    tme.hwndTrack = btn;
    _TrackMouseEvent(&tme);
}
static LRESULT CALLBACK ButtonProc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
    App& a = *g;
    int idx = (int)GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if (idx >= 0 && idx < 5) {
        switch (msg) {
        case WM_MOUSEMOVE:
            if (!a.btnHover[idx]) {
                a.btnHover[idx] = true;
                InvalidateRect(hwnd, nullptr, FALSE);
            }
            trackButtonEnter(hwnd);
            break;
        case WM_MOUSELEAVE:
            if (a.btnHover[idx]) {
                a.btnHover[idx] = false;
                InvalidateRect(hwnd, nullptr, FALSE);
            }
            break;
        }
    }
    return CallWindowProcW(g_btnOldProc, hwnd, msg, wp, lp);
}

// ---------- 列表 ----------
static void initListColumns(HWND list, const wchar_t* cols[], int widths[], int n) {
    LVCOLUMNW lc; memset(&lc, 0, sizeof(lc));
    lc.mask = LVCF_TEXT | LVCF_WIDTH | LVCF_FMT;
    for (int i = 0; i < n; ++i) {
        lc.iSubItem = i;
        lc.pszText = (LPWSTR)cols[i];
        lc.cx = widths[i];
        lc.fmt = (i == 0 || i == 1) ? LVCFMT_CENTER : LVCFMT_LEFT;
        ListView_InsertColumn(list, i, &lc);
    }
}

static void repopulateList(HWND list, const std::vector<USBDevice>& devs, bool withPath) {
    ListView_DeleteAllItems(list);
    for (size_t i = 0; i < devs.size(); ++i) {
        const USBDevice& d = devs[i];
        std::wstring v0 = toW(d.getFormattedVid());
        std::wstring v1 = toW(d.getFormattedPid());
        std::wstring v2 = toW(d.getDisplayName());
        LVITEMW it; memset(&it, 0, sizeof(it));
        it.mask = LVIF_TEXT | LVIF_PARAM;
        it.pszText = (LPWSTR)v0.c_str();
        it.iItem = (int)i;
        it.lParam = (LPARAM)i;
        int idx = ListView_InsertItem(list, &it);
        ListView_SetItemText(list, idx, 1, (LPWSTR)v1.c_str());
        ListView_SetItemText(list, idx, 2, (LPWSTR)v2.c_str());
        if (withPath) {
            std::wstring v3 = d.path.empty() ? L"-" : toW(d.path);
            ListView_SetItemText(list, idx, 3, (LPWSTR)v3.c_str());
        }
    }
    // 刷新后清除所有残留选中/焦点，避免“默认全选中”
    ListView_SetItemState(list, -1, 0, LVIS_SELECTED | LVIS_FOCUSED);
}

static int selectedRow(HWND list) { return ListView_GetNextItem(list, -1, LVNI_SELECTED); }

static const USBDevice* selFrom(const std::vector<USBDevice>& devs, HWND list) {
    int i = selectedRow(list);
    if (i < 0 || (size_t)i >= devs.size()) return nullptr;
    return &devs[(size_t)i];
}

static void clearSelection(HWND list) {
    int i = selectedRow(list);
    while (i >= 0) {
        ListView_SetItemState(list, i, 0, LVIS_SELECTED);
        i = selectedRow(list);
    }
}

// ---------- 状态 ----------
static void setStatusA(const std::wstring& m) { setWndText(g->statusA, m); }
static void setStatusB(const std::wstring& m) { setWndText(g->statusB, m); }

static std::wstring deviceInfoText(const USBDevice& d) {
    std::wstring s = toW(d.getDisplayName());
    s += L" | VID: " + toW(d.getFormattedVid());
    s += L" | PID: " + toW(d.getFormattedPid());
    s += L" | 序列号: ";
    s += d.serial.empty() ? L"N/A" : toW(d.serial);
    return s;
}

static void refreshViews() {
    repopulateList(g->listAll, g->allDev, true);
    repopulateList(g->listAdded, g->addedDev, false);
    repopulateList(g->listRemoved, g->removedDev, false);
    setWndText(g->headerCount, std::to_wstring(g->allDev.size()) + L" 个设备已连接");
    setWndText(g->headerAdded, L"+ 新增设备  " + std::to_wstring(g->addedDev.size()));
    setWndText(g->headerRemoved, L"- 移除设备  " + std::to_wstring(g->removedDev.size()));
}

// ---------- 逻辑 ----------
static void startScan();

static void setBaseline() {
    if (g->curDev.empty()) {
        MessageBoxW(g->hwnd, L"当前没有设备列表，请先刷新", L"提示", MB_OK | MB_ICONINFORMATION);
        return;
    }
    g->baseline = g->curDev;
    g->addedDev.clear();
    g->removedDev.clear();
    setStatusA(L"已重置基准为当前设备列表");
    refreshViews();
    setStatusB(L"基准: " + std::to_wstring((int)g->baseline.size()) + L" 个设备 (" + nowTime() + L")");
}

static void copySelected() {
    const USBDevice* d = selFrom(g->allDev, g->listAll);
    if (!d) d = selFrom(g->addedDev, g->listAdded);
    if (!d) d = selFrom(g->removedDev, g->listRemoved);
    if (!d) { MessageBoxW(g->hwnd, L"请先选择一个设备", L"提示", MB_OK | MB_ICONINFORMATION); return; }

    std::wstring text = toW(d->toClipboardText());
    if (OpenClipboard(g->hwnd)) {
        EmptyClipboard();
        size_t bytes = (text.size() + 1) * sizeof(wchar_t);
        HGLOBAL hg = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if (hg) {
            void* p = GlobalLock(hg);
            if (p) {
                memcpy(p, text.c_str(), bytes);
                GlobalUnlock(hg);
                if (SetClipboardData(CF_UNICODETEXT, hg)) hg = nullptr;
            }
            if (hg) GlobalFree(hg);
        }
        CloseClipboard();
    }
    setStatusA(L"已复制: " + toW(d->getDisplayName()));
}

static void handleScanResult(std::vector<USBDevice>& devices) {
    if (g->baseline.empty()) {
        g->baseline = devices;
        setStatusB(L"基准: " + std::to_wstring((int)devices.size()) + L" 个设备 (" + nowTime() + L")");
    }

    DeviceComparer::compare(g->baseline, devices, g->addedDev, g->removedDev);
    bool changed = DeviceComparer::hasChanged(g->curDev, devices);
    g->curDev = devices;

    if (changed) {
        g->allDev = devices;
        sortByPidVidName(g->allDev);
        refreshViews();

        std::wstring change;
        if (!g->addedDev.empty()) change += L" (+" + std::to_wstring((int)g->addedDev.size()) + L")";
        if (!g->removedDev.empty()) change += L" (-" + std::to_wstring((int)g->removedDev.size()) + L")";
        setWndText(g->headerCount, std::to_wstring((int)devices.size()) + L" 个设备已连接" + change);

        setStatusA(L"最后刷新: " + nowTime() + L" | 设备数: " +
                   std::to_wstring((int)g->curDev.size()) + L" -> " +
                   std::to_wstring((int)devices.size()));
    }
}

static void startScan() {
    if (g->scanning.exchange(true)) return;
    if (g->closing) { g->scanning = false; return; }
    std::thread([&]() {
        auto* devs = new std::vector<USBDevice>(scanUsbDevices());
        if (!g->closing) PostMessageW(g->hwnd, ids::kMessageScanDone, 0, (LPARAM)devs);
        else { delete devs; g->scanning = false; }
    }).detach();
}

// ---------- 布局 ----------
static void layoutChildren(HWND hwnd) {
    RECT rc; GetClientRect(hwnd, &rc);
    int W = rc.right, H = rc.bottom;
    const int pad = 16;            // 窗口外边距（Fluent 大留白）
    const int gap = 14;            // 面板间间距
    const int toolbarH = 60, statusH = 24;
    const int ctlH = 38;           // 工具栏控件高度（Fluent 高按钮）
    int topY = (toolbarH - ctlH) / 2;

    // 左：标题（设备数）
    int x = pad;
    MoveWindow(g->headerCount, x, topY, 260, ctlH, TRUE);

    // 右：按钮右对齐（同一垂直中心线）；重置为常用按钮放第二位
    const int order[5] = { 0, 1, 3, 2, 4 };       // 显示顺序
    bool showStop = g->autoRefresh;
    bool show[5] = { showStop, !showStop, true, true, true };
    ShowWindow(g->cmdButtons[0], show[0] ? SW_SHOW : SW_HIDE);
    ShowWindow(g->cmdButtons[1], show[1] ? SW_SHOW : SW_HIDE);
    int gapb = 10;                                 // 按钮间距

    // 实例化按钮文本度量：宽度 = 文本宽 + 双侧留白，确保任何文字完整显示且与边框留有空隙
    const int btnPadx = 24;                        // 每侧文本内边距
    int btnW[5] = { 60, 60, 60, 60, 60 };
    HDC dc = GetDC(g->hwnd);
    HFONT oldFont = (HFONT)SelectObject(dc, g->hFontBtns);
    for (int k = 0; k < 5; ++k) {
        if (!show[k]) continue;
        wchar_t lab[64];
        GetWindowTextW(g->cmdButtons[k], lab, 64);
        SIZE s{};
        GetTextExtentPoint32W(dc, lab, (int)wcslen(lab), &s);
        btnW[k] = s.cx + btnPadx * 2;
        if (btnW[k] < 60) btnW[k] = 60;
    }
    SelectObject(dc, oldFont);
    ReleaseDC(g->hwnd, dc);

    int total = 0, ngap = 0;
    for (int q = 0; q < 5; ++q) { int k = order[q]; if (show[k]) { total += btnW[k]; ++ngap; } }
    total += (ngap - 1) * gapb;
    x = W - pad - total;
    for (int q = 0; q < 5; ++q) {
        int k = order[q];
        if (!show[k]) continue;
        MoveWindow(g->cmdButtons[k], x, topY, btnW[k], ctlH, TRUE);
        x += btnW[k] + gapb;
    }

    // 主内容区：左列表 | 右两面板，顶部/底部对齐、边框对齐
    int cTop = toolbarH + pad;
    int cBot = H - statusH - pad;
    int leftW = (int)((W - 2 * pad - gap) * 0.60);
    int midX  = pad + leftW + gap;
    int rightW = W - pad - midX;
    int half = (cBot - cTop) / 2;               // 右侧上下两面板等高
    int chipH = 28;                              // chip 告示条高度（更饱满）
    int listH = half - chipH;

    MoveWindow(g->listAll, pad, cTop, leftW, cBot - cTop, TRUE);

    MoveWindow(g->headerAdded, midX, cTop, rightW, chipH, TRUE);
    MoveWindow(g->listAdded, midX, cTop + chipH, rightW, listH, TRUE);
    MoveWindow(g->headerRemoved, midX, cTop + half, rightW, chipH, TRUE);
    MoveWindow(g->listRemoved, midX, cTop + half + chipH, rightW, listH, TRUE);

    MoveWindow(g->statusA, pad, H - statusH, W / 2 - pad, statusH, TRUE);
    MoveWindow(g->statusB, W / 2, H - statusH, W / 2 - pad, statusH, TRUE);
}

// ---------- 控件创建 ----------
static HWND makeStatic(HWND parent, int id, const wchar_t* text, DWORD extra) {
    return CreateWindowExW(0, L"STATIC", text,
        WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | extra,
        0, 0, 0, 0, parent, (HMENU)(INT_PTR)id, g_hInst, nullptr);
}
static HWND makeCommand(HWND parent, int id, const wchar_t* text) {
    return CreateWindowExW(0, L"BUTTON", text,
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_CLIPSIBLINGS | BS_OWNERDRAW,
        0, 0, 0, 0, parent, (HMENU)(INT_PTR)id, g_hInst, nullptr);
}
static HWND makeList(HWND parent, int id) {
    HWND h = CreateWindowExW(0, WC_LISTVIEWW, L"",
        WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_BORDER |
        LVS_REPORT | LVS_SINGLESEL | LVS_SHOWSELALWAYS,
        0, 0, 0, 0, parent, (HMENU)(INT_PTR)id, g_hInst, nullptr);
    ListView_SetExtendedListViewStyle(h, LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER);
    ListView_SetBkColor(h, theme::white());
    ListView_SetTextBkColor(h, theme::white());
    // 计入自定义绘制的交替行，背景色统一为画布色
    return h;
}

static void createControls(HWND hwnd) {
    App& a = *g;

    // 字体（XP 回退 Tahoma，现代系统用 Segoe UI 更清晰）
    a.hFontBtns = CreateFontW(-16, 0, 0, 0, FW_BOLD, 0, 0, 0,
                              DEFAULT_CHARSET, 0, 0, CLEARTYPE_QUALITY, 0, L"Segoe UI");
    a.hFontTitle = CreateFontW(-19, 0, 0, 0, FW_BOLD, 0, 0, 0,
                               DEFAULT_CHARSET, 0, 0, CLEARTYPE_QUALITY, 0, L"Segoe UI");
    a.hFontList = makeMonoFont(-14); // 数据列等宽，字号更大，行更高更舒适

    a.headerCount = makeStatic(hwnd, ids::kHeaderDeviceCount, L"0 个设备已连接", 0);
    SendMessageW(a.headerCount, WM_SETFONT, (WPARAM)a.hFontTitle, TRUE);

    const wchar_t* labels[5] = { L"停止刷新", L"自动刷新", L"手动刷新", L"重置", L"复制" };
    for (int i = 0; i < 5; ++i) {
        HWND b = makeCommand(hwnd, ids::kBtnStopRefresh + i, labels[i]);
        a.cmdButtons[i] = b;
        g_btnOldProc = (WNDPROC)SetWindowLongPtrW(b, GWLP_WNDPROC, (LONG_PTR)&ButtonProc);
        SetWindowLongPtrW(b, GWLP_USERDATA, (LONG_PTR)i);
    }

    // 主列表列（VID/PID 固定 4 位 hex + 等宽，极小列宽 + 居中）
    {
        const wchar_t* cols[4] = { L"VID", L"PID", L"设备名称", L"路径" };
        int ws[4] = { 46, 46, 230, 400 };
        a.listAll = makeList(hwnd, 3000);
        initListColumns(a.listAll, cols, ws, 4);
    }
    a.headerAdded = makeStatic(hwnd, ids::kHeaderAdded, L"+ 新增设备  0", SS_LEFT);
    a.headerRemoved = makeStatic(hwnd, ids::kHeaderRemoved, L"- 移除设备  0", SS_LEFT);
    SendMessageW(a.headerAdded, WM_SETFONT, (WPARAM)a.hFontBtns, TRUE);
    SendMessageW(a.headerRemoved, WM_SETFONT, (WPARAM)a.hFontBtns, TRUE);
    {
        const wchar_t* cols[3] = { L"VID", L"PID", L"设备名称" };
        int ws[3] = { 46, 46, 240 };
        a.listAdded = makeList(hwnd, 3100);
        initListColumns(a.listAdded, cols, ws, 3);
        a.listRemoved = makeList(hwnd, 3200);
        initListColumns(a.listRemoved, cols, ws, 3);
    }

    for (HWND lst : { a.listAll, a.listAdded, a.listRemoved })
        SendMessageW(lst, WM_SETFONT, (WPARAM)a.hFontList, TRUE);

    a.statusA = makeStatic(hwnd, ids::kStatusA, L"", 0);
    a.statusB = makeStatic(hwnd, ids::kStatusB, L"", SS_RIGHT);
    SendMessageW(a.statusA, WM_SETFONT, (WPARAM)a.hFontList, TRUE);
    SendMessageW(a.statusB, WM_SETFONT, (WPARAM)a.hFontList, TRUE);

    a.brAddedBg = CreateSolidBrush(theme::successBg());
    a.brRemovedBg = CreateSolidBrush(theme::dangerBg());

    layoutChildren(hwnd);
}

// ---------- 设备插拔通知 ----------
static void setupDeviceNotifier(HWND hwnd) {
    // 监听 USB 设备接口插拔
    DEV_BROADCAST_DEVICEINTERFACE_W ifc{};
    ifc.dbcc_size = sizeof(ifc);
    ifc.dbcc_devicetype = DBT_DEVTYP_DEVICEINTERFACE;
    ifc.dbcc_classguid = {0xA5DCBF10, 0x6530, 0x11D2,
                          {0x90, 0x1F, 0x00, 0xC0, 0x4F, 0xB9, 0x51, 0xED}};
    g->hDevNotify = RegisterDeviceNotificationW(hwnd, &ifc, DEVICE_NOTIFY_WINDOW_HANDLE);
}

// ---------- 窗口过程 ----------
static LRESULT CALLBACK WndProc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
    App& a = *g;

    switch (msg) {
    case WM_CREATE: {
        g->hwnd = hwnd;
        g = &a; // no-op keep
        createControls(hwnd);
        setupDeviceNotifier(hwnd);
        startScan();
        if (a.autoRefresh) SetTimer(hwnd, ids::kTimerAuto, kAutoRefreshIntervalMs, nullptr);
        return 0;
    }
    case WM_SIZE:
        layoutChildren(hwnd);
        return 0;

    case ids::kMessageScanDone: {
        std::vector<USBDevice>* devs = (std::vector<USBDevice>*)lp;
        std::unique_ptr<std::vector<USBDevice>> ptr(devs);
        a.scanning = false;
        if (!a.closing) handleScanResult(*devs);
        return 0;
    }

    case WM_TIMER:
        if (wp == ids::kTimerAuto && a.autoRefresh && !a.scanning && !a.closing) startScan();
        return 0;

    case WM_DEVICECHANGE: {
        DWORD e = (DWORD)wp;
        if (e == DBT_DEVICEARRIVAL || e == DBT_DEVICEREMOVECOMPLETE || e == DBT_DEVNODES_CHANGED) {
            if (a.scanning) { /* 轮询 timer 会衔接 */ } else startScan();
        }
        return 0;
    }

    case WM_COMMAND: {
        int id = LOWORD(wp);
        switch (id) {
        case ids::kBtnStopRefresh:
            a.autoRefresh = false;
            KillTimer(hwnd, ids::kTimerAuto);
            layoutChildren(hwnd);
            setStatusA(L"自动刷新已停止");
            return 0;
        case ids::kBtnAutoRefresh:
            if (!a.autoRefresh) {
                a.autoRefresh = true;
                SetTimer(hwnd, ids::kTimerAuto, kAutoRefreshIntervalMs, nullptr);
                layoutChildren(hwnd);
                setStatusA(L"自动刷新已开启（间隔 100 ms）");
            }
            return 0;
        case ids::kBtnManualRefresh:
            if (!a.scanning) startScan();
            return 0;
        case ids::kBtnBaseline:
            setBaseline();
            return 0;
        case ids::kBtnCopy:
            copySelected();
            return 0;
        }
        return 0;
    }

    case WM_NOTIFY: {
        LPNMHDR nm = (LPNMHDR)lp;

        // 列表行自定义绘制：交替行 + 主色选中态
        if (nm->code == NM_CUSTOMDRAW) {
            LPNMLVCUSTOMDRAW lvcd = (LPNMLVCUSTOMDRAW)lp;
            if (lvcd->nmcd.dwDrawStage == CDDS_PREPAINT)
                return CDRF_NOTIFYITEMDRAW;
            if (lvcd->nmcd.dwDrawStage == CDDS_ITEMPREPAINT) {
                // 用权威 API 判定真实选中，避免 nmcd.uItemState 在聚焦/无焦点状态下误报，
                // 导致未选中行也显示选中背景（“默认全选中”）
                int idx = (int)lvcd->nmcd.dwItemSpec;
                bool sel = (ListView_GetItemState((HWND)lvcd->nmcd.hdr.hwndFrom,
                                                  idx, LVIS_SELECTED) & LVIS_SELECTED) != 0;
                if (sel) {
                    lvcd->clrTextBk = theme::primary();
                    lvcd->clrText = theme::white();
                } else {
                    lvcd->clrText = theme::text();
                    lvcd->clrTextBk = (idx % 2) == 0 ? theme::rowEven() : theme::white();
                }
                return CDRF_NEWFONT;
            }
            return CDRF_DODEFAULT;
        }

        if (nm->code == LVN_ITEMCHANGED) {
            LPNMLISTVIEW lv = (LPNMLISTVIEW)lp;
            if (lv->uNewState & LVIS_SELECTED && !a.ignoreSelChange) {
                a.ignoreSelChange = true;
                // 互斥：左选中清右，右选中清左
                if (nm->idFrom == 3000) { clearSelection(a.listAdded); clearSelection(a.listRemoved); }
                else { clearSelection(a.listAll); }
                const USBDevice* d = nullptr;
                HWND src = nullptr;
                if (nm->idFrom == 3000) { d = selFrom(a.allDev, a.listAll); src = a.listAll; }
                else if (nm->idFrom == 3100) { d = selFrom(a.addedDev, a.listAdded); src = a.listAdded; }
                else if (nm->idFrom == 3200) { d = selFrom(a.removedDev, a.listRemoved); src = a.listRemoved; }
                if (d) {
                    std::wstring tag;
                    if (src == a.listAdded) tag = L"新增  ";
                    else if (src == a.listRemoved) tag = L"移除  ";
                    setStatusA(tag + deviceInfoText(*d));
                }
                a.ignoreSelChange = false;
            }
        }
        // C++ 代码在所有 case 之后有 return 0
        return 0;
    }

    case WM_DRAWITEM: {
        LPDRAWITEMSTRUCT dis = (LPDRAWITEMSTRUCT)lp;
        if (dis->CtlType == ODT_BUTTON) {
            wchar_t buf[64];
            GetWindowTextW(dis->hwndItem, buf, 64);
            int idx = dis->CtlID - ids::kBtnStopRefresh;
            bool disabled = (dis->itemState & ODS_DISABLED) != 0;
            bool pressed  = (dis->itemState & ODS_SELECTED) != 0;
            COLORREF base = disabled ? theme::controlFill() : btnFill(dis->CtlID);
            COLORREF fill = base;
            if (!disabled) {
                if (idx >= 0 && idx < 5 && a.btnHover[idx]) fill = shade(base, +22);
                if (pressed) fill = shade(base, -36);
            }
            COLORREF textCol = disabled ? RGB(0xA5,0xA5,0xA5) : theme::white();
            COLORREF borderCol = disabled ? theme::border() : btnBorder(dis->CtlID);

            RECT rc = dis->rcItem;
            // 圆角填充边界（收小，避免与窗口边缘贴死）
            InflateRect(&rc, -4, -3);
            HPEN pen = CreatePen(PS_SOLID, 1, borderCol);
            HGDIOBJ oldPen = SelectObject(dis->hDC, pen);
            HBRUSH br = CreateSolidBrush(fill);
            HGDIOBJ oldBr = SelectObject(dis->hDC, br);
            RoundRect(dis->hDC, rc.left, rc.top, rc.right, rc.bottom, 6, 6);
            SelectObject(dis->hDC, oldPen); DeleteObject(pen);
            SelectObject(dis->hDC, oldBr); DeleteObject(br);

            // 文本再额外内收，保证文字与圆角/描边留有足够间距
            RECT tr = rc;
            InflateRect(&tr, -6, -4);
            SetBkMode(dis->hDC, TRANSPARENT);
            SetTextColor(dis->hDC, textCol);
            HFONT old = (HFONT)SelectObject(dis->hDC, a.hFontBtns);
            DrawTextW(dis->hDC, buf, -1, &tr, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            SelectObject(dis->hDC, old);
            return TRUE;
        }
        return 0;
    }

    case WM_CTLCOLORSTATIC: {
        HDC hdc = (HDC)wp;
        HWND hctl = (HWND)lp;
        if (hctl == a.headerAdded) {
            SetBkColor(hdc, theme::successBg());
            SetTextColor(hdc, theme::success());
            return (LRESULT)a.brAddedBg;
        }
        if (hctl == a.headerRemoved) {
            SetBkColor(hdc, theme::dangerBg());
            SetTextColor(hdc, theme::danger());
            return (LRESULT)a.brRemovedBg;
        }
        if (hctl == a.headerCount) {
            SetBkColor(hdc, theme::white());
            SetTextColor(hdc, theme::text());
            return (LRESULT)GetStockObject(WHITE_BRUSH);
        }
        SetBkColor(hdc, theme::bg());
        SetTextColor(hdc, theme::textSecondary());
        return (LRESULT)GetStockObject(NULL_BRUSH);
    }

    case WM_ERASEBKGND:
        return 1; // WM_PAINT 绘制背景

    case WM_PAINT: {
        PAINTSTRUCT ps;
        HDC hdc = BeginPaint(hwnd, &ps);
        RECT rc; GetClientRect(hwnd, &rc);
        int W = rc.right;
        // 背景
        FillRect(hdc, &rc, (HBRUSH)GetStockObject(NULL_BRUSH));
        HBRUSH bgBr = CreateSolidBrush(theme::bg());
        FillRect(hdc, &rc, bgBr);
        DeleteObject(bgBr);
        // 顶部工具条（白）+ 分隔线
        RECT tb{0, 0, W, 60};
        FillRect(hdc, &tb, (HBRUSH)GetStockObject(WHITE_BRUSH));
        HPEN pen = CreatePen(PS_SOLID, 1, theme::border());
        HPEN oldPen = (HPEN)SelectObject(hdc, pen);
        MoveToEx(hdc, 0, 60, nullptr);
        LineTo(hdc, W, 60);
        SelectObject(hdc, oldPen);
        DeleteObject(pen);
        EndPaint(hwnd, &ps);
        return 0;
    }

    case WM_CLOSE:
        a.closing = true;
        KillTimer(hwnd, ids::kTimerAuto);
        if (a.hDevNotify) { UnregisterDeviceNotification(a.hDevNotify); a.hDevNotify = 0; }
        DestroyWindow(hwnd);
        return 0;

    case WM_DESTROY:
        if (a.hFontBtns) DeleteObject(a.hFontBtns);
        if (a.brAddedBg) DeleteObject(a.brAddedBg);
        if (a.brRemovedBg) DeleteObject(a.brRemovedBg);
        PostQuitMessage(0);
        return 0;
    }
    return DefWindowProcW(hwnd, msg, wp, lp);
}

} // namespace vpid

// ---------- 入口 ----------
int WINAPI wWinMain(HINSTANCE hInstance, HINSTANCE, PWSTR, int nCmdShow) {
    g_hInst = hInstance;

    INITCOMMONCONTROLSEX icc;
    icc.dwSize = sizeof(icc);
    icc.dwICC = ICC_LISTVIEW_CLASSES | ICC_STANDARD_CLASSES;
    InitCommonControlsEx(&icc);

    vpid::App app;
    vpid::g = &app;

    WNDCLASSW wc;
    memset(&wc, 0, sizeof(wc));
    wc.lpfnWndProc = vpid::WndProc;
    wc.hInstance = hInstance;
    wc.hCursor = LoadCursor(nullptr, IDC_ARROW);
    wc.hbrBackground = nullptr;
    // 程序图标（USB，资源 ID = 1）：窗口/任务栏/Alt-Tab 均显示
    wc.hIcon = LoadIconW(hInstance, MAKEINTRESOURCEW(1));
    if (!wc.hIcon) wc.hIcon = (HICON)LoadImageW(nullptr, IDI_APPLICATION, IMAGE_ICON, 0, 0, LR_SHARED);
    wc.lpszClassName = L"VPidViewerClass";
    wc.style = CS_HREDRAW | CS_VREDRAW;
    RegisterClassW(&wc);

    HWND hwnd = CreateWindowExW(0, wc.lpszClassName,
        vpid::toW(vpid::kAppName).c_str(),
        WS_OVERLAPPEDWINDOW,
        CW_USEDEFAULT, CW_USEDEFAULT,
        vpid::kDefaultWindowWidth, vpid::kDefaultWindowHeight,
        nullptr, nullptr, hInstance, nullptr);
    if (!hwnd) return 1;

    ShowWindow(hwnd, nCmdShow);
    UpdateWindow(hwnd);

    MSG msg;
    while (GetMessageW(&msg, nullptr, 0, 0) > 0) {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    return (int)msg.wParam;
}