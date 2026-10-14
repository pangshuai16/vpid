// SOUI5 版主窗口实现：业务逻辑与原 Win32 版保持一致
#include "MainDlg.h"

#include <windows.h>
#include <algorithm>

#include <SApp.h>
#include <res.mgr/SUiDef.h>   // GETCOLOR：行级语义色从 color.xml 取，避免 C++ 与 XML 两处硬编码
#include <helper/SColor.h>    // CR_INVALID

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

// 稳定排序：VID → PID → 名称。
// VID 在前（厂商）符合运维的查表习惯——先认厂商再认型号；
// 且"全部设备"与"变更记录"共用同一个排序，两栏行序一致，扫视时对得上。
void SortByVidPidName(std::vector<USBDevice> &v) {
    std::stable_sort(v.begin(), v.end(),
        [](const USBDevice &a, const USBDevice &b) {
            int c = a.getFormattedVid().compare(b.getFormattedVid());
            if (c != 0) return c < 0;
            c = a.getFormattedPid().compare(b.getFormattedPid());
            if (c != 0) return c < 0;
            return a.getDisplayName() < b.getDisplayName();
        });
}

// 显示指纹：把"屏幕上现在画的是什么"压成一个 64 位哈希。
// 用途：HandleScanResult 用它判断是否需要重建列表——只看 diff.changed 是不够的，
// 见 HandleScanResult 里"插上再拔掉"那段注释。
//
// [perf] 原来用 std::wstring 拼接 vid/pid/serial（含 UTF8ToW 三次转换），
// 每帧要跑 3 次 MakeSig = O(N) 的字符串分配 + 拷贝。50 个设备 × 10Hz × 3 = 每秒 1500 次
// 字符串拼接。改成 FNV-1a 64 位哈希：一次遍历、零分配、零拷贝，快 10×+。
//
// 碰撞概率：n 个设备的碰撞概率约 n²/2^64；50 个设备 ~3e-15，工程上不可发生。
// 若真碰撞（不可能），最坏是多刷一次表，无功能影响。
#include <cstdint>
static inline uint64_t fnv1a64(uint64_t h, const char *s) {
    while (*s) {
        h ^= (uint64_t)(uint8_t)*s++;
        h *= 0x100000001B3ULL;  // FNV-1a 64 prime
    }
    return h;
}

uint64_t MakeSigHash(const std::vector<USBDevice> &v) {
    uint64_t h = 0x146575AB4149D913ULL;  // FNV-1a 64 offset
    for (const auto &d : v) {
        DeviceKey k = d.getUniqueKey();
        h = fnv1a64(h, k.vid.c_str());
        h = fnv1a64(h, "/");
        h = fnv1a64(h, k.pid.c_str());
        h = fnv1a64(h, "/");
        h = fnv1a64(h, k.serial.c_str());
        h = fnv1a64(h, ";");
    }
    return h;
}

// ---- 表头排序：把设备映射成"某一列的可比较字符串" ----
// 两栏的列语义不同：左栏 VID/PID/名称/路径，右栏 状态/VID/PID/名称，故分开两个函数。
std::string AllColumnText(const USBDevice &d, int col) {
    switch (col) {
    case 0: return d.getFormattedVid();
    case 1: return d.getFormattedPid();
    case 2: return d.getDisplayName();
    case 3: return d.path;
    default: return std::string();
    }
}

std::string ChangedColumnText(const USBDevice &d, int col) {
    switch (col) {
    // 状态列在"新增组 / 移除组"内部恒等 → 排序是空操作，这正是期望的行为：
    // 保持"新增在前、移除在后"的分组（分组由 RefreshViews 的插入顺序决定）不被排序打散。
    case 0: return std::string();
    case 1: return d.getFormattedVid();
    case 2: return d.getFormattedPid();
    case 3: return d.getDisplayName();
    default: return std::string();
    }
}

// "a 是否应排在 b 之前"。相等一律 false，交给 std::stable_sort 保留默认序（VID→PID→名称）
// 作为稳定次键——于是"主列相同的行"仍按默认序排列，同一份数据任何一次排序结果都一致。
bool LessByField(const std::string &x, const std::string &y, bool asc) {
    if (x == y) return false;
    return asc ? (x < y) : (x > y);
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
    lc_changed_  = FindChildByName2<SListCtrl>(L"lc_changed");
    txt_total_count_   = FindChildByName(L"txt_total_count");
    txt_added_count_   = FindChildByName(L"txt_added_count");
    txt_removed_count_ = FindChildByName(L"txt_removed_count");
    pill_live_   = FindChildByName(L"pill_live");
    pill_idle_   = FindChildByName(L"pill_idle");
    txt_status_a_ = FindChildByName(L"txt_status_a");
    txt_status_b_ = FindChildByName(L"txt_status_b");

    // 表头点击排序：直接订阅两个列表"表头"的事件集。
    // 不用 EVENT_NAME_HANDLER：那个按事件源控件名匹配，而 EventHeaderClick 的发出者是
    // listctrl 的子窗口（表头），名字对不上 lc_all / lc_changed。
    if (lc_all_) {
        if (SHeaderCtrl *h = lc_all_->GetHeaderCtrl())
            h->GetEventSet()->subscribeEvent(&MainDlg::OnHeaderClickAll, this);
    }
    if (lc_changed_) {
        if (SHeaderCtrl *h = lc_changed_->GetHeaderCtrl())
            h->GetEventSet()->subscribeEvent(&MainDlg::OnHeaderClickChanged, this);
    }

    UpdateMonitorState();
    SetupDeviceNotifier();
    StartScan();
    if (auto_refresh_) SNativeWnd::SetTimer(kTimerAuto, kAutoRefreshIntervalMs);
    UpdateMonitorState();
    return 0;
}

// ---- 应用栏状态胶囊：与自动刷新开关保持同步 ----
// live / idle 是两个互斥窗口（各有独立皮肤：绿底绿点 / 灰底灰点），靠显隐切换。
// 不采用"运行时换肤"是因为那需要拿到 ISkinObj 再 SetSkin，而两个窗口的方案
// 与"停止刷新/恢复刷新"按钮组用的是同一套机制，行为更可预测。
void MainDlg::UpdateMonitorState() {
    if (pill_live_) pill_live_->SetVisible(auto_refresh_ ? TRUE : FALSE, TRUE);
    if (pill_idle_) pill_idle_->SetVisible(auto_refresh_ ? FALSE : TRUE, TRUE);
}

// ---- 表头点击排序 ----
// 设计：排序作用于"底层数据表"而不是列表控件里的行。
// 原因：行号是选中行反查设备的唯一索引（SelectedFromList 用 all_dev_[i] / changed_dev_[i]），
// 若只在控件内 SortItems，行序与数据表就脱钩，选中会指到错误的设备；
// 而每次自动刷新都会重建列表，控件内的排序也保不住。排数据表则两个问题都不存在。
void MainDlg::ApplyUserSort() {
    if (sort_col_all_ >= 0) {
        const int col = sort_col_all_;
        const bool asc = sort_asc_all_;
        std::stable_sort(all_dev_.begin(), all_dev_.end(),
                         [col, asc](const USBDevice &a, const USBDevice &b) {
                             return LessByField(AllColumnText(a, col),
                                                AllColumnText(b, col), asc);
                         });
    }
    if (sort_col_changed_ >= 0) {
        const int col = sort_col_changed_;
        const bool asc = sort_asc_changed_;
        auto cmp = [col, asc](const USBDevice &a, const USBDevice &b) {
            return LessByField(ChangedColumnText(a, col),
                               ChangedColumnText(b, col), asc);
        };
        // 两个组各自排序（不合并成一张表）：保持"新增在前 / 移除在后"的语义分组，
        // 组内再按用户选的列排。这样"状态"列点击后组序仍然正确。
        std::stable_sort(added_dev_.begin(), added_dev_.end(), cmp);
        std::stable_sort(removed_dev_.begin(), removed_dev_.end(), cmp);
    }
}

void MainDlg::UpdateSortIndicator() {
    auto apply = [](SListCtrl *list, int col, bool asc) {
        if (!list) return;
        SHeaderCtrl *h = list->GetHeaderCtrl();
        if (!h) return;
        for (UINT i = 0; i < h->GetItemCount(); ++i) {
            UINT flag = 0; // 0 = 清除该列箭头
            if ((int)i == col) flag = asc ? HDF_SORTUP : HDF_SORTDOWN;
            h->SetItemSort((int)i, flag);
        }
    };
    apply(lc_all_, sort_col_all_, sort_asc_all_);
    apply(lc_changed_, sort_col_changed_, sort_asc_changed_);
}

// 同一列再点一次 → 切换升/降序；点别的列 → 切到该列并回到升序。
BOOL MainDlg::OnHeaderClickAll(EventHeaderClick *pEvt) {
    if (!pEvt) return TRUE;
    const int col = pEvt->iItem;
    if (col == sort_col_all_) {
        sort_asc_all_ = !sort_asc_all_;
    } else {
        sort_col_all_ = col;
        sort_asc_all_ = true;
    }
    RefreshViews();        // 内部先 ApplyUserSort() 再渲染
    UpdateSortIndicator();
    return TRUE;
}

BOOL MainDlg::OnHeaderClickChanged(EventHeaderClick *pEvt) {
    if (!pEvt) return TRUE;
    const int col = pEvt->iItem;
    if (col == sort_col_changed_) {
        sort_asc_changed_ = !sort_asc_changed_;
    } else {
        sort_col_changed_ = col;
        sort_asc_changed_ = true;
    }
    RefreshViews();
    UpdateSortIndicator();
    return TRUE;
}

void MainDlg::OnClose() {
    closing_ = true;
    if (notifier_) notifier_->Stop();
    SNativeWnd::KillTimer(kTimerAuto);
    DestroyWindow();
    // 必须 PostQuitMessage：SApplication::Run 内部是 pMsgLoop->Run()（GetMessage 循环），
    // 仅 DestroyWindow 不会让 GetMessage 返回 0 —— 窗口没了但进程/消息循环仍在。
    // 标题栏 X 与"退出"按钮都走这里，统一收口。
    ::PostQuitMessage(0);
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
    // WindowsNotifier 已在窗口消息层面过滤事件类型。
    // 必须同时看 auto_refresh_（与 OnDeviceChanged 一致）：
    // "停止刷新"= 冻结自动识别；否则暂停后插拔设备仍会自动扫描，
    // 界面在"监控已暂停"状态下继续变化，语义自相矛盾。
    if (auto_refresh_ && !scanning_ && !closing_) StartScan();
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
    UpdateMonitorState();
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
        // 恢复后立刻对一次：把暂停期间真实发生的变化一次性对齐到界面上，
        // 不必等下一个 100ms 定时器（也让"恢复刷新"这个动作有即时反馈）。
        if (!scanning_ && !closing_) StartScan();
    }
    UpdateMonitorState();
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

void MainDlg::OnBtnExit() {
    // 与标题栏 X 同一个收口：OnClose 统一做退出清理（置 closing_ 阻止在途扫描回填、
    // 停设备通知、停自动刷新定时器、销毁窗口），避免两条路径各写一份清理逻辑漂移。
    OnClose();
}

// ---- 三列表互斥选中（原版 LVN_ITEMCHANGED 逻辑）----
void MainDlg::OnListSelChanged(IEvtArgs *pEvt) {
    if (!pEvt) return;
    // 用事件源控件的 name 判断是哪张列表被点击
    LPCWSTR name = pEvt->NameFrom();
    SListCtrl *src = nullptr;
    if (name && wcscmp(name, L"lc_all") == 0) src = lc_all_;
    else if (name && wcscmp(name, L"lc_changed") == 0) src = lc_changed_;
    if (!src) return;
    if (src->GetSelectedItem() < 0) return; // 仅处理"选中了新行"

    // 互斥：左选中清右，右选中清左
    if (src == lc_all_) {
        lc_changed_->SetSelectedItem(-1);
    } else {
        lc_all_->SetSelectedItem(-1);
    }

    // 状态栏显示选中设备详情（原版 deviceInfoText + 新增/移除前缀）
    const USBDevice *d = nullptr;
    std::wstring tag;
    if (src == lc_all_) {
        d = SelectedFromList(lc_all_, all_dev_);
    } else {
        d = SelectedFromList(lc_changed_, changed_dev_);
        tag = L"变更  ";
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
    if (!lc_all_ || !lc_changed_) return;
    // 渲染前先按用户选择的排序列/方向重排数据。放在这里而不是各调用点：
    // RefreshViews 是唯一的"把数据搬到屏幕"的出口，排序跟着它走，
    // 就不会出现"某条路径重绘时忘了排序"的漏网。
    ApplyUserSort();
    lc_all_->DeleteAllItems();
    lc_changed_->DeleteAllItems();
    changed_dev_.clear();

    for (size_t i = 0; i < all_dev_.size(); ++i) {
        const USBDevice &d = all_dev_[i];
        int idx = lc_all_->InsertItem((int)i, UTF8ToW(d.getFormattedVid()).c_str());
        lc_all_->SetSubItemText(idx, 1, UTF8ToW(d.getFormattedPid()).c_str());
        lc_all_->SetSubItemText(idx, 2, UTF8ToW(d.getDisplayName()).c_str());
        lc_all_->SetSubItemText(idx, 3, d.path.empty() ? L"-" : UTF8ToW(d.path).c_str());
    }

    // 变更记录：新增在前、移除在后，合成一张表。
    // 语义做"三重编码"——状态列文字（新增/移除）+ 行底色（靛青晕/朱砂晕）+ 分组条计数（新增 N/移除 M）：
    //   只有颜色 → 色觉障碍者分辨不出，且截图打印、黑白屏后失效；
    //   只有文字 → 每次都要逐行读字，扫视成本高。
    // 三者互为冗余，任一通道单独失效都不影响判读。
    // 刻意选"靛青 + 朱砂"而不是"绿 + 红"：红绿色盲（约 8% 男性）能区分青与红，绿与红则不能。
    // 行底色取自行级色接口 SetItemColor（SOUI 原生只支持斑马纹，见 SListCtrl 的 vpid 补丁）。
    // 注意：名字必须带 "@color/" 前缀。SUiDef::GetColor 对不带前缀的字符串走的是
    // 字面量解析分支（"rgb(r,g,b)"），命名色查找拿不到，会静默返回 CR_INVALID——
    // 那样 SetItemColor 就等于没调用，行底色会退回斑马纹。这个坑很隐蔽：不报错、不崩，
    // 只是"颜色没上"，只能靠截图采样才能发现。
    const COLORREF crAddBg = GETCOLOR(L"@color/rowAddBg");
    const COLORREF crDelBg = GETCOLOR(L"@color/rowDelBg");
    // 行首竖条：与主色/危险色同源（见 color.xml），比行底晕更"实"，
    // 负责在整片表格里一眼指出"这几行是新增 / 那几行是移除"。
    const COLORREF crAddBar = GETCOLOR(L"@color/rowAddBar");
    const COLORREF crDelBar = GETCOLOR(L"@color/rowDelBar");
    int row = 0;
    for (const auto &d : added_dev_) {
        int idx = lc_changed_->InsertItem(row, L"新增");
        lc_changed_->SetSubItemText(idx, 1, UTF8ToW(d.getFormattedVid()).c_str());
        lc_changed_->SetSubItemText(idx, 2, UTF8ToW(d.getFormattedPid()).c_str());
        lc_changed_->SetSubItemText(idx, 3, UTF8ToW(d.getDisplayName()).c_str());
        lc_changed_->SetItemColor(idx, crAddBg, CR_INVALID);
        lc_changed_->SetItemAccent(idx, crAddBar);
        changed_dev_.push_back(d);
        ++row;
    }
    for (const auto &d : removed_dev_) {
        int idx = lc_changed_->InsertItem(row, L"移除");
        lc_changed_->SetSubItemText(idx, 1, UTF8ToW(d.getFormattedVid()).c_str());
        lc_changed_->SetSubItemText(idx, 2, UTF8ToW(d.getFormattedPid()).c_str());
        lc_changed_->SetSubItemText(idx, 3, UTF8ToW(d.getDisplayName()).c_str());
        lc_changed_->SetItemColor(idx, crDelBg, CR_INVALID);
        lc_changed_->SetItemAccent(idx, crDelBar);
        changed_dev_.push_back(d);
        ++row;
    }

    // 分组条计数徽标：带语义词（"新增 3" / "移除 2"），不裸显数字
    if (txt_total_count_) txt_total_count_->SetWindowText(std::to_wstring(all_dev_.size()).c_str());
    if (txt_added_count_)
        txt_added_count_->SetWindowText((std::wstring(L"新增 ") + std::to_wstring(added_dev_.size())).c_str());
    if (txt_removed_count_)
        txt_removed_count_->SetWindowText((std::wstring(L"移除 ") + std::to_wstring(removed_dev_.size())).c_str());

    // 记下"这一帧画的是什么"，供下次扫描判断是否需要重建列表
    shown_sig_all_ = MakeSigHash(all_dev_);
    shown_sig_add_ = MakeSigHash(added_dev_);
    shown_sig_del_ = MakeSigHash(removed_dev_);
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
    if (!d) d = SelectedFromList(lc_changed_, changed_dev_);
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
    // 排序：让"变更记录"的行序只取决于设备本身，而不是扫描返回的顺序。
    // 不排序的话，同一组变更在连续两次扫描间可能顺序抖动，行会跳来跳去，
    // 也会让下面的指纹比对产生假阳性（内容没变、顺序变了 → 误判为需要重绘）。
    SortByVidPidName(diff.added);
    SortByVidPidName(diff.removed);
    added_dev_ = std::move(diff.added);
    removed_dev_ = std::move(diff.removed);

    const size_t prevCount = cur_dev_.size();
    cur_dev_ = devices;
    all_dev_ = devices;
    SortByVidPidName(all_dev_);

    // 先落地用户排序，再算"是否需要重建"。
    // 下面的脏检查用 MakeSig(all_dev_) 与 shown_sig_all_ 比对，而 shown_sig_all_ 是在
    // RefreshViews 里"经过用户排序之后"记录的。若这里不先排一次，两边行序就不一致 →
    // 每次扫描都判为脏 → 每 100ms 全量重建两个列表（闪烁 + 白耗）。
    ApplyUserSort();

    // —— 是否需要重建列表 ——
    // 不能用 diff.changed 代替这个判断。反例（运维最常见的动作）：
    //   基准 = {A} → 插上 B → 扫描得 {A,B}，diff 报"新增 B"，列表显示"新增 B"；
    //   拔掉 B   → 扫描得 {A}，diff(baseline={A}, {A}) 报"无变化"，
    //             若按 changed 判断就不重绘，屏幕上会永远留着那行"新增 B"。
    // 指纹比对同时覆盖三条通道（设备集合 / 新增集合 / 移除集合），
    // 任何一条与"当前画在屏幕上的"不一致就重绘，"回到基准"时也能正确清空。
    const bool dirty = firstScan
                    || MakeSigHash(all_dev_)     != shown_sig_all_
                    || MakeSigHash(added_dev_)   != shown_sig_add_
                    || MakeSigHash(removed_dev_) != shown_sig_del_;
    if (dirty) RefreshViews();

    // 状态栏每次扫描完成都更新：运维要看的是"最后一次成功扫描的时刻与设备数"，
    // 而不是"最后一次发生变化的时刻"。设备数迁移 + 相对基准的增减用文字写全，
    // 不依赖颜色（与列表里的三重编码一致）。
    std::wstring change;
    if (!added_dev_.empty())   change += L" · 新增 " + std::to_wstring((int)added_dev_.size());
    if (!removed_dev_.empty()) change += L" · 移除 " + std::to_wstring((int)removed_dev_.size());
    SetStatusA(L"最后刷新 " + NowTime() + L" · 设备数 " +
               std::to_wstring((int)prevCount) + L" → " +
               std::to_wstring((int)devices.size()) + change);
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