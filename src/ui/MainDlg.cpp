// SOUI5 版主窗口实现：业务逻辑与原 Win32 版保持一致
#include "MainDlg.h"

#include <windows.h>
#include <algorithm>

#include <SApp.h>

#ifdef _WIN32
// ---- [XP 兼容] 覆盖 MSVC UCRT 静态库引入的 Vista+ API 导入 ----
// GetNumaHighestNodeNumber 是 Vista+ 导出，XP 的 kernel32 没有。
// UCRT 通过 __imp_GetNumaHighestNodeNumber 引用它（NUMA 拓扑探测）。
// 方案：定义本地实现 + 链接器别名指令，把该导入无条件重定向到我们自己的符号。
// XP 是单 NUMA 节点系统：返回 0（节点编号 0）即语义正确。
namespace vpid_xpcompat {
    BOOL WINAPI FakeGetNumaHighestNodeNumber(PULONG HighestNodeNumber) {
        if (HighestNodeNumber) *HighestNodeNumber = 0;
        return TRUE;
    }
    BOOL WINAPI FakeGetNumaProcessorNumber(PULONG ProcessorNumber) {
        if (ProcessorNumber) *ProcessorNumber = 0;
        return TRUE;
    }
}
// extern "C" 变量在 x86 下符号名 = 名字 + 前导下划线：__imp_My... => ___imp_My...
// 链接期由 CMake 注入 /ALTERNATENAME 把 UCRT 对 GetNumaHighestNodeNumber/GetNumaProcessorNumber
// 的导入引用重定向到上面两个本地符号（Vista+ API，XP 的 kernel32 无此导出）。
extern "C" {
    __declspec(selectany) BOOL (WINAPI *__imp_MyGetNumaHighestNodeNumber)(PULONG) =
        vpid_xpcompat::FakeGetNumaHighestNodeNumber;
    __declspec(selectany) BOOL (WINAPI *__imp_MyGetNumaProcessorNumber)(PULONG) =
        vpid_xpcompat::FakeGetNumaProcessorNumber;
}
#endif // _WIN32（NUMA stub 仅 Windows/MSVC 需要）

namespace vpid {

namespace {
const UINT kMessageScanDone = WM_APP + 1;
const UINT_PTR kTimerAuto = 1;

// 与 Win32 原版等宽字体不同，SOUI 用 XML font 属性；这里仅做转换工具
std::wstring UTF8ToW(const std::string &s) {
    if (s.empty()) return std::wstring();
    int need = MultiByteToWideChar(CP_UTF8, 0, s.c_str(), (int)s.size(), nullptr, 0);
    std::wstring out((size_t)need, L'\0');
    MultiByteToWideChar(CP_UTF8, 0, s.c_str(), (int)s.size(), &out[0], need);
    return out;
}

std::wstring NowTime() {
    SYSTEMTIME st;
    GetLocalTime(&st);
    wchar_t b[32];
    swprintf(b, 32, L"%02d:%02d:%02d", st.wHour, st.wMinute, st.wSecond);
    return b;
}

void SortByPidVidName(std::vector<USBDevice> &v) {
    std::stable_sort(v.begin(), v.end(),
        [](const USBDevice &a, const USBDevice &b) {
            int c = a.getFormattedPid().compare(b.getFormattedPid());
            if (c != 0) return c < 0;
            c = a.getFormattedVid().compare(b.getFormattedVid());
            if (c != 0) return c < 0;
            return a.getDisplayName() < b.getDisplayName();
        });
}
} // namespace

MainDlg::MainDlg() : SHostWnd(L"layout:dlg_main") {
}

MainDlg::~MainDlg() {
    closing_ = true;
    if (notifier_) {
        notifier_->Stop();
        notifier_.reset();
    }
}

int MainDlg::OnCreate(LPCREATESTRUCT lpCreateStruct) {
    // 先让 SHostWnd 完成布局
    SetMsgHandled(FALSE);
    int ret = SHostWnd::OnCreate(lpCreateStruct);
    SetMsgHandled(TRUE);

    // 缓存控件指针
    lc_all_      = FindChildByName2<SListCtrl>(L"lc_all");
    lc_added_    = FindChildByName2<SListCtrl>(L"lc_added");
    lc_removed_  = FindChildByName2<SListCtrl>(L"lc_removed");
    txt_count_   = FindChildByName(L"txt_device_count");
    txt_added_   = FindChildByName(L"txt_added_title");
    txt_removed_ = FindChildByName(L"txt_removed_title");
    txt_status_a_ = FindChildByName(L"txt_status_a");
    txt_status_b_ = FindChildByName(L"txt_status_b");

    SetupDeviceNotifier();
    StartScan();
    if (auto_refresh_) SNativeWnd::SetTimer(kTimerAuto, kAutoRefreshIntervalMs);
    return 0;
}

void MainDlg::OnClose() {
    closing_ = true;
    if (notifier_) notifier_->Stop();
    SNativeWnd::KillTimer(kTimerAuto);
    DestroyWindow();
}

void MainDlg::OnSize(UINT nType, CSize size) {
    SetMsgHandled(FALSE); // 继续交给 SHostWnd 布局
}

// ---- 系统标题栏：NCCALCSIZE 强制走 DefWindowProc ----
// SOUI 的 SNcPainter 常驻消息链（CHAIN_MSG_MAP_MEMBER(*m_pNcPainter)），其 OnNcCalcSize
// 在未启用自绘非客户区时返回 0（"客户区=整窗"），导致 WS_CAPTION 加了也不压缩标题栏。
// MainDlg 的映射先于 CHAIN_MSG_MAP(SHostWnd) 执行，这里显式接管并交给系统默认处理。
LRESULT MainDlg::OnNcCalcSize(UINT, WPARAM wParam, LPARAM lParam, BOOL &bHandled) {
    bHandled = TRUE;
    return ::DefWindowProc(m_hWnd, WM_NCCALCSIZE, wParam, lParam);
}

// ---- 系统标题栏：NCHITTEST 强制走 DefWindowProc ----
// SNcPainter::OnNcHitTest 会把标题栏区域返回 HTCLIENT（吞掉系统按钮/拖拽/边框缩放）。
// 这里交给 DefWindowProc：系统自动识别 HTCAPTION(拖拽/双击最大化)/HTMINBUTTON/HTCLOSE/
// HTMAXBUTTON/HTLEFT 等（WS_CAPTION + WS_THICKFRAME 已由 wndStyle 提供）。
LRESULT MainDlg::OnNcHitTestSys(UINT, WPARAM, LPARAM lParam, BOOL &bHandled) {
    bHandled = TRUE;
    return ::DefWindowProc(m_hWnd, WM_NCHITTEST, 0, lParam);
}

void MainDlg::OnTimer(UINT_PTR idEvent) {
    if (idEvent == kTimerAuto && auto_refresh_ && !scanning_ && !closing_) {
        StartScan();
    }
    SetMsgHandled(FALSE); // 原版在此不停止消息传递
}

void MainDlg::OnCommand(UINT uNotifyCode, int nID, HWND wndCtl) {
    SetMsgHandled(FALSE);
}

// ---- 扫描线程完成通知 ----
LRESULT MainDlg::OnScanDone(UINT, WPARAM, LPARAM lParam, BOOL &bHandled) {
    auto *devs = reinterpret_cast<std::vector<USBDevice> *>(lParam);
    std::unique_ptr<std::vector<USBDevice>> ptr(devs);
    scanning_ = false;
    if (!closing_) HandleScanResult(*devs);
    return 0;
}

// ---- 设备插拔通知（Windows: WM_DEVICECHANGE 消息；Linux: udev 回调 → OnDeviceChanged）----
LRESULT MainDlg::OnDeviceChange(UINT, WPARAM, LPARAM, BOOL &bHandled) {
    // WindowsNotifier 已在窗口消息层面过滤事件类型，直接触发扫描
    if (!scanning_ && !closing_) StartScan();
    return 0;
}

// ---- 按钮事件 ----
void MainDlg::OnBtnStopRefresh() {
    auto_refresh_ = false;
    SNativeWnd::KillTimer(kTimerAuto);
    SetStatusA(L"自动刷新已停止");
    // 同步切换两按钮可见性（自动刷新按钮恢复显示；display=0 让隐藏按钮不占位）
    SWindow *btnAuto = FindChildByName(L"btn_auto_refresh");
    SWindow *btnStop = FindChildByName(L"btn_stop_refresh");
    if (btnAuto) btnAuto->SetVisible(TRUE, TRUE);
    if (btnStop) btnStop->SetVisible(FALSE, TRUE);
}

void MainDlg::OnBtnAutoRefresh() {
    if (!auto_refresh_) {
        auto_refresh_ = true;
        SNativeWnd::SetTimer(kTimerAuto, kAutoRefreshIntervalMs);
        SWindow *btnAuto = FindChildByName(L"btn_auto_refresh");
        SWindow *btnStop = FindChildByName(L"btn_stop_refresh");
        if (btnAuto) btnAuto->SetVisible(FALSE, TRUE);
        if (btnStop) btnStop->SetVisible(TRUE, TRUE);
        SetStatusA(L"自动刷新已开启（间隔 100 ms）");
    }
}

void MainDlg::OnBtnManualRefresh() {
    if (!scanning_) StartScan();
}

void MainDlg::OnBtnBaseline() {
    SetBaseline();
}

void MainDlg::OnBtnCopy() {
    CopySelected();
}

// ---- 三列表互斥选中（原版 LVN_ITEMCHANGED 逻辑）----
void MainDlg::OnListSelChanged(IEvtArgs *pEvt) {
    if (!pEvt) return;
    // 用事件源控件的 name 判断是哪张列表被点击
    LPCWSTR name = pEvt->NameFrom();
    SListCtrl *src = nullptr;
    if (name && wcscmp(name, L"lc_all") == 0) src = lc_all_;
    else if (name && wcscmp(name, L"lc_added") == 0) src = lc_added_;
    else if (name && wcscmp(name, L"lc_removed") == 0) src = lc_removed_;
    if (!src) return;
    if (src->GetSelectedItem() < 0) return; // 仅处理"选中了新行"

    // 互斥：左选中清右，右选中清左
    if (src == lc_all_) {
        lc_added_->SetSelectedItem(-1);
        lc_removed_->SetSelectedItem(-1);
    } else {
        lc_all_->SetSelectedItem(-1);
    }

    // 状态栏显示选中设备详情（原版 deviceInfoText + 新增/移除前缀）
    const USBDevice *d = nullptr;
    std::wstring tag;
    if (src == lc_all_) {
        d = SelectedFromList(lc_all_, all_dev_);
    } else if (src == lc_added_) {
        d = SelectedFromList(lc_added_, added_dev_);
        tag = L"新增  ";
    } else if (src == lc_removed_) {
        d = SelectedFromList(lc_removed_, removed_dev_);
        tag = L"移除  ";
    }
    if (d) SetStatusA(tag + DeviceInfoText(*d));
}

// ---- 控件工具 ----
void MainDlg::SetStatusA(const std::wstring &m) {
    if (txt_status_a_) txt_status_a_->SetWindowText(m.c_str());
}

void MainDlg::SetStatusB(const std::wstring &m) {
    if (txt_status_b_) txt_status_b_->SetWindowText(m.c_str());
}

std::wstring MainDlg::DeviceInfoText(const USBDevice &d) {
    std::wstring s = UTF8ToW(d.getDisplayName());
    s += L" | VID: " + UTF8ToW(d.getFormattedVid());
    s += L" | PID: " + UTF8ToW(d.getFormattedPid());
    s += L" | 序列号: ";
    s += d.serial.empty() ? L"N/A" : UTF8ToW(d.serial);
    return s;
}

void MainDlg::RefreshViews() {
    if (!lc_all_) return;
    lc_all_->DeleteAllItems();
    lc_added_->DeleteAllItems();
    lc_removed_->DeleteAllItems();

    for (size_t i = 0; i < all_dev_.size(); ++i) {
        const USBDevice &d = all_dev_[i];
        int idx = lc_all_->InsertItem((int)i, UTF8ToW(d.getFormattedVid()).c_str());
        lc_all_->SetSubItemText(idx, 1, UTF8ToW(d.getFormattedPid()).c_str());
        lc_all_->SetSubItemText(idx, 2, UTF8ToW(d.getDisplayName()).c_str());
        lc_all_->SetSubItemText(idx, 3, d.path.empty() ? L"-" : UTF8ToW(d.path).c_str());
    }
    for (size_t i = 0; i < added_dev_.size(); ++i) {
        const USBDevice &d = added_dev_[i];
        int idx = lc_added_->InsertItem((int)i, UTF8ToW(d.getFormattedVid()).c_str());
        lc_added_->SetSubItemText(idx, 1, UTF8ToW(d.getFormattedPid()).c_str());
        lc_added_->SetSubItemText(idx, 2, UTF8ToW(d.getDisplayName()).c_str());
    }
    for (size_t i = 0; i < removed_dev_.size(); ++i) {
        const USBDevice &d = removed_dev_[i];
        int idx = lc_removed_->InsertItem((int)i, UTF8ToW(d.getFormattedVid()).c_str());
        lc_removed_->SetSubItemText(idx, 1, UTF8ToW(d.getFormattedPid()).c_str());
        lc_removed_->SetSubItemText(idx, 2, UTF8ToW(d.getDisplayName()).c_str());
    }

    if (txt_count_) {
        std::wstring t = std::to_wstring(all_dev_.size()) + L" 个设备已连接";
        txt_count_->SetWindowText(t.c_str());
    }
    if (txt_added_) {
        std::wstring t = L"+ 新增设备  " + std::to_wstring(added_dev_.size());
        txt_added_->SetWindowText(t.c_str());
    }
    if (txt_removed_) {
        std::wstring t = L"- 移除设备  " + std::to_wstring(removed_dev_.size());
        txt_removed_->SetWindowText(t.c_str());
    }
}

const USBDevice *MainDlg::SelectedFromList(SListCtrl *list,
                                           const std::vector<USBDevice> &devs) {
    if (!list) return nullptr;
    int i = list->GetSelectedItem();
    if (i < 0 || (size_t)i >= devs.size()) return nullptr;
    return &devs[(size_t)i];
}

void MainDlg::SetBaseline() {
    if (cur_dev_.empty()) {
        SMessageBox(NULL, L"当前没有设备列表，请先刷新", L"提示", MB_OK | MB_ICONINFORMATION);
        return;
    }
    baseline_ = cur_dev_;
    added_dev_.clear();
    removed_dev_.clear();
    SetStatusA(L"已重置基准为当前设备列表");
    RefreshViews();
    SetStatusB(L"基准: " + std::to_wstring((int)baseline_.size()) + L" 个设备 (" + NowTime() + L")");
}

void MainDlg::CopySelected() {
    const USBDevice *d = SelectedFromList(lc_all_, all_dev_);
    if (!d) d = SelectedFromList(lc_added_, added_dev_);
    if (!d) d = SelectedFromList(lc_removed_, removed_dev_);
    if (!d) {
        SMessageBox(NULL, L"请先选择一个设备", L"提示", MB_OK | MB_ICONINFORMATION);
        return;
    }
    std::wstring text = UTF8ToW(d->toClipboardText());
    if (OpenClipboard(m_hWnd)) {
        EmptyClipboard();
        size_t bytes = (text.size() + 1) * sizeof(wchar_t);
        HGLOBAL hg = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if (hg) {
            void *p = GlobalLock(hg);
            if (p) {
                memcpy(p, text.c_str(), bytes);
                GlobalUnlock(hg);
                if (SetClipboardData(CF_UNICODETEXT, hg)) hg = nullptr;
            }
            if (hg) GlobalFree(hg);
        }
        CloseClipboard();
    }
    SetStatusA(L"已复制: " + UTF8ToW(d->getDisplayName()));
}

void MainDlg::HandleScanResult(std::vector<USBDevice> &devices) {
    // 首次扫描：baseline 为空，先落基准；此时 diff(baseline, devices) 必然
    // 无变化（oldKeys==newKeys），若仅按 changed 判断将永不刷新列表/计数，
    // 导致"状态栏 0 个设备、表格全空"。故首次扫描无条件刷新。
    const bool firstScan = baseline_.empty();
    if (firstScan) {
        baseline_ = devices;
        SetStatusB(L"基准: " + std::to_wstring((int)devices.size()) + L" 个设备 (" + NowTime() + L")");
    }
    DeviceDiff diff = DeviceComparer::diff(baseline_, devices);
    added_dev_ = std::move(diff.added);
    removed_dev_ = std::move(diff.removed);
    bool changed = diff.changed;
    cur_dev_ = devices;
    if (firstScan || changed) {
        all_dev_ = devices;
        SortByPidVidName(all_dev_);
        RefreshViews();
        std::wstring change;
        if (!added_dev_.empty()) change += L" (+" + std::to_wstring((int)added_dev_.size()) + L")";
        if (!removed_dev_.empty()) change += L" (-" + std::to_wstring((int)removed_dev_.size()) + L")";
        if (txt_count_) {
            std::wstring t = std::to_wstring((int)devices.size()) + L" 个设备已连接" + change;
            txt_count_->SetWindowText(t.c_str());
        }
        SetStatusA(L"最后刷新: " + NowTime() + L" | 设备数: " +
                   std::to_wstring((int)cur_dev_.size()) + L" -> " +
                   std::to_wstring((int)devices.size()));
    }
}

void MainDlg::StartScan() {
    if (scanning_.exchange(true)) return;
    if (closing_) {
        scanning_ = false;
        return;
    }
    // 用原生线程而非 std::thread（std::thread 经 libcpmt 拉入 libconcrt，
    // 而 concrt 引用 GetNumaHighestNodeNumber 等 Vista+ API，XP 上无法加载）
    struct ScanCtx { MainDlg *self; };
    auto *ctx = new ScanCtx{this};
    HANDLE h = CreateThread(nullptr, 0, [](LPVOID p) -> DWORD {
        std::unique_ptr<ScanCtx> c((ScanCtx *)p);
        MainDlg *self = c->self;
        auto *devs = new std::vector<USBDevice>(scanUsbDevices());
        if (!self->closing_) self->PostMessage(kMessageScanDone, 0, (LPARAM)devs);
        else {
            delete devs;
            self->scanning_ = false;
        }
        return 0;
    }, ctx, 0, nullptr);
    if (h) CloseHandle(h);
    else {
        delete ctx;
        scanning_ = false;
    }
}

void MainDlg::SetupDeviceNotifier() {
    // 跨平台设备插拔通知：Windows=RegisterDeviceNotification；Linux=udev。
    // 平台不支持时 Start 返回 false，UI 仍由 100ms 自动刷新定时器兜底（与原版一致）。
    notifier_ = DeviceNotifier::Create([this]() { OnDeviceChanged(); });
    // Start 参数为 void*；Windows 下 HWND 即 void*，Linux(swinx) 下 HWND 为
    // unsigned long，统一 reinterpret_cast 兼容两平台。
    if (notifier_) notifier_->Start(reinterpret_cast<void*>(m_hWnd));
}

} // namespace vpid