// SOUI5 版主窗口：SHostWnd 派生（跨平台 UI 层，平台差异收敛到 core 的 Scanner/DeviceNotifier）
// 布局: src/ui/uires/xml/dlg_main.xml
// 事件: EVENT_MAP（SOUI 控件事件）+ MSG_MAP（原生消息：扫描完成/定时器/设备插拔）
// 注意: EVENT_MAP / MSG_MAP 系列是全局宏（WTL 风格），不带 SOUI:: 前缀
#ifndef VPID_SOUI_MAINDLG_H
#define VPID_SOUI_MAINDLG_H

// 必须先包含 souistd.h（SOUI 全库基础：config.h / SOUI_EXP / windows.h 等）
#include <souistd.h>

#include <core/SHostWnd.h>
#include <control/SListCtrl.h>
#include <string>
#include <vector>
#include <atomic>
#include <memory>
#include <cstdint>

#include "common/constants.h"
#include "core/device_info.h"       // USBDevice
#include "core/device_scanner.h"    // scanUsbDevices
#include "core/device_comparer.h"   // DeviceComparer
#include "core/device_notifier.h"   // DeviceNotifier（跨平台设备插拔通知抽象）

using namespace SOUI;

namespace vpid {

class MainDlg : public SHostWnd {
public:
    MainDlg();
    ~MainDlg() override;

    // 设备插拔事件入口（Windows 由 WM_DEVICECHANGE 触发；Linux 由 udev 线程回调）
    // 注意：必须同时看 auto_refresh_。"停止刷新"的语义是"冻结自动识别"，
    // 若这里不看 auto_refresh_，暂停后插拔设备仍会自动扫描 → 界面在"暂停"下继续变化，
    // 用户就会看到"已暂停却仍识别到新增"这种自相矛盾的状态。
    void OnDeviceChanged() { if (auto_refresh_ && !scanning_ && !closing_) StartScan(); }

protected:
    // ---- 原生消息（由 MSG_MAP 消息映射调度，非虚函数，不加 override）----
    int OnCreate(LPCREATESTRUCT lpCreateStruct);
    void OnClose();
    void OnSize(UINT nType, CSize size);
    void OnTimer(UINT_PTR idEvent);
    void OnCommand(UINT uNotifyCode, int nID, HWND wndCtl);
    // 系统标题栏：拦截 WM_NCCALCSIZE，绕开 SOUI SNcPainter 的"客户区=整窗"返回 0，
    // 强制 DefWindowProc 按 WS_CAPTION 压缩出系统标题栏/边框区域
    LRESULT OnNcCalcSize(UINT uMsg, WPARAM wParam, LPARAM lParam, BOOL &bHandled);
    // 系统标题栏：拦截 WM_NCHITTEST，交给 DefWindowProc 识别标题栏(拖拽)/系统按钮/边框缩放
    LRESULT OnNcHitTestSys(UINT uMsg, WPARAM wParam, LPARAM lParam, BOOL &bHandled);

    LRESULT OnScanDone(UINT uMsg, WPARAM wParam, LPARAM lParam, BOOL &bHandled);
    LRESULT OnDeviceChange(UINT uMsg, WPARAM wParam, LPARAM lParam, BOOL &bHandled);

    // ---- SOUI 控件事件 ----
    void OnBtnStopRefresh();
    void OnBtnAutoRefresh();
    void OnBtnManualRefresh();
    void OnBtnBaseline();
    void OnBtnCopy();
    void OnBtnExit();
    // 列表选择变化：三列表互斥（原版 LVN_ITEMCHANGED 逻辑）
    void OnListSelChanged(IEvtArgs *pEvt);

    // ---- 表头点击排序（用户自定义排序）----
    // 两个列表各自独立维护"按哪列 / 什么方向"。SHeaderCtrl 的 EventHeaderClick 由
    // SListCtrl 内的表头直接发出，这里在 OnCreate 里订阅其事件集（不走 EVENT_NAME_HANDLER：
    // 那个按"事件源控件名"匹配，而事件源是 listctrl 的子窗口表头，名字对不上）。
    BOOL OnHeaderClickAll(EventHeaderClick *pEvt);
    BOOL OnHeaderClickChanged(EventHeaderClick *pEvt);

    // ---- 内部工具 ----
    void InitListColumns();
    void RefreshViews();
    void SetStatusA(const std::wstring &m);
    void SetStatusB(const std::wstring &m);
    void StartScan();
    // 阻塞到在途扫描线程返回。必须在 DestroyWindow 之后、PostQuitMessage 之前调用：
    // 线程上下文只装 HWND 所以不怕窗口先销毁；而 ExitProcess 会等所有线程结束，
    // 所以这里必须真的等到线程返回，超时"甩掉"线程只会让进程残留。
    void ShutdownScanThread();
    void SetBaseline();
    void CopySelected();
    const USBDevice *SelectedFromList(SListCtrl *list,
                                      const std::vector<USBDevice> &devs);
    void HandleScanResult(std::vector<USBDevice> &devices);
    void SetupDeviceNotifier();
    std::wstring DeviceInfoText(const USBDevice &d);
    // 应用栏监控状态胶囊：同步"实时监控中 / 监控已暂停"文案与圆点颜色
    void UpdateMonitorState();
    // 按当前排序列/方向重排三个数据表（RefreshViews 渲染前调用）
    void ApplyUserSort();
    // 把排序列/方向同步到表头的排序指示三角
    void UpdateSortIndicator();

    // ---- 控件缓存（FindChildByName2 每次查找有开销，缓存指针）----
    SListCtrl *lc_all_       = nullptr;
    SListCtrl *lc_changed_   = nullptr;  // 变更记录（新增在前 / 移除在后，合并自原"新增+移除"两栏）
    SWindow   *txt_total_count_   = nullptr;  // 分组条计数徽标（全部设备）
    SWindow   *txt_added_count_   = nullptr;  // 分组条计数徽标（新增）
    SWindow   *txt_removed_count_ = nullptr;  // 分组条计数徽标（移除）
    // 应用栏监控状态胶囊：live / idle 是两个互斥窗口，靠显隐切换（不运行时换肤）
    SWindow   *pill_live_         = nullptr;
    SWindow   *pill_idle_         = nullptr;
    SWindow   *txt_status_a_ = nullptr;
    SWindow   *txt_status_b_ = nullptr;

    // ---- 业务状态（与原 Win32 版一致）----
    std::vector<USBDevice> all_dev_;
    std::vector<USBDevice> cur_dev_;
    std::vector<USBDevice> added_dev_;
    std::vector<USBDevice> removed_dev_;
    std::vector<USBDevice> baseline_;
    // 与 lc_changed_ 的行序严格一一对应（新增在前、移除在后），供选中行反查设备
    std::vector<USBDevice> changed_dev_;
    // "当前屏幕上画的是什么"的指纹（设备集合 / 新增集合 / 移除集合三条通道）。
    // RefreshViews() 渲染后写入，HandleScanResult() 比对后决定要不要重建列表。
    // 没有它，"插上再拔掉"回到基准时残留的变更行不会被清掉（diff 会报无变化）。
    // [perf] 64 位 FNV-1a 哈希，替代原来的 std::wstring 拼接（每帧 O(N) 字符串分配）。
    // 三个 0 是"从未渲染"的哨兵值，首帧必 dirty（firstScan 分支也兜底）。
    uint64_t shown_sig_all_ = 0;
    uint64_t shown_sig_add_ = 0;
    uint64_t shown_sig_del_ = 0;

    std::atomic<bool> scanning_{false};
    std::atomic<bool> closing_{false};
    bool auto_refresh_ = true;
    std::unique_ptr<DeviceNotifier> notifier_; // 跨平台设备插拔通知

    // 在途扫描线程句柄（scanning_ 互斥 → 同一时刻至多一个）。
    // [XP 退出残留根因] 扫描线程原先是 fire-and-forget 且持有 this：
    //   OnClose 里 DestroyWindow 之后，MainDlg 要等消息循环退出、_tWinMain 栈展开
    //   才析构。若线程此刻仍在跑并读写 this->closing_/this->scanning_，就是 UAF。
    //   Win10/11 的堆实现容忍这种越界写，XP 的 msvcrt 会破坏堆元数据 →
    //   _tWinMain 返回后 CRT 退出清理（static 析构 / 堆锁）挂死，
    //   表现为"窗口关了、进程还挂在任务管理器"。
    //   修法三重保险：① 线程上下文只装 HWND（不装 this）；② 退出前先请求取消扫描
    //   （RequestScanCancel，让扫描在一个 SetupAPI 往返内自行返回）；
    //   ③ DestroyWindow 之后 join 线程——Windows 的 ExitProcess 会等所有线程结束，
    //   线程还在跑 → 进程永远退不出，这才是"窗口关了但进程残留"的直接原因。
    std::atomic<HANDLE> scanThread_{nullptr};

    // 表头点击排序状态（-1 = 未启用，沿用 SortByVidPidName 的 VID→PID→名称 默认序）。
    // 两栏各自独立：列号即 headerStyle 里 <item> 的顺序（本项目已关闭拖拽换列，列号稳定）。
    int  sort_col_all_ = -1;
    bool sort_asc_all_ = true;
    int  sort_col_changed_ = -1;
    bool sort_asc_changed_ = true;

    // SOUI 控件事件映射（按钮 name）
    EVENT_MAP_BEGIN()
        EVENT_NAME_COMMAND(L"btn_stop_refresh", OnBtnStopRefresh)
        EVENT_NAME_COMMAND(L"btn_auto_refresh", OnBtnAutoRefresh)
        EVENT_NAME_COMMAND(L"btn_manual_refresh", OnBtnManualRefresh)
        EVENT_NAME_COMMAND(L"btn_baseline", OnBtnBaseline)
        EVENT_NAME_COMMAND(L"btn_copy", OnBtnCopy)
        EVENT_NAME_COMMAND(L"btn_exit", OnBtnExit)
        // 列表选择变化（互斥选中：左清右 / 右清左）
        EVENT_NAME_HANDLER(L"lc_all", EVT_LC_SELCHANGED, OnListSelChanged)
        EVENT_NAME_HANDLER(L"lc_changed", EVT_LC_SELCHANGED, OnListSelChanged)
    EVENT_MAP_END2(SHostWnd)

    // 原生消息映射
    BEGIN_MSG_MAP_EX(MainDlg)
        MSG_WM_CREATE(OnCreate)
        MSG_WM_CLOSE(OnClose)
        MSG_WM_SIZE(OnSize)
        MSG_WM_TIMER(OnTimer)
        MESSAGE_HANDLER(WM_NCCALCSIZE, OnNcCalcSize)
        MESSAGE_HANDLER(WM_NCHITTEST, OnNcHitTestSys)
        MESSAGE_HANDLER(WM_APP + 1, OnScanDone)
#ifdef WM_DEVICECHANGE
        // 仅 Windows：设备插拔经系统消息通知（swinx/Linux 无 WM_DEVICECHANGE，
        // Linux 走 udev 回调 → OnDeviceChanged）
        MESSAGE_HANDLER(WM_DEVICECHANGE, OnDeviceChange)
#endif
        CHAIN_MSG_MAP(SHostWnd)
    END_MSG_MAP()
};

} // namespace vpid

#endif // VPID_SOUI_MAINDLG_H