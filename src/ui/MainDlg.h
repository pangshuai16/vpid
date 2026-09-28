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
    void OnDeviceChanged() { if (!scanning_ && !closing_) StartScan(); }

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
    // 列表选择变化：三列表互斥（原版 LVN_ITEMCHANGED 逻辑）
    void OnListSelChanged(IEvtArgs *pEvt);

    // ---- 内部工具 ----
    void InitListColumns();
    void RefreshViews();
    void SetStatusA(const std::wstring &m);
    void SetStatusB(const std::wstring &m);
    void StartScan();
    void SetBaseline();
    void CopySelected();
    const USBDevice *SelectedFromList(SListCtrl *list,
                                      const std::vector<USBDevice> &devs);
    void HandleScanResult(std::vector<USBDevice> &devices);
    void SetupDeviceNotifier();
    std::wstring DeviceInfoText(const USBDevice &d);

    // ---- 控件缓存（FindChildByName2 每次查找有开销，缓存指针）----
    SListCtrl *lc_all_       = nullptr;
    SListCtrl *lc_added_     = nullptr;
    SListCtrl *lc_removed_   = nullptr;
    SWindow   *txt_count_    = nullptr;
    SWindow   *txt_added_    = nullptr;
    SWindow   *txt_removed_  = nullptr;
    SWindow   *txt_status_a_ = nullptr;
    SWindow   *txt_status_b_ = nullptr;

    // ---- 业务状态（与原 Win32 版一致）----
    std::vector<USBDevice> all_dev_;
    std::vector<USBDevice> cur_dev_;
    std::vector<USBDevice> added_dev_;
    std::vector<USBDevice> removed_dev_;
    std::vector<USBDevice> baseline_;

    std::atomic<bool> scanning_{false};
    std::atomic<bool> closing_{false};
    bool auto_refresh_ = true;
    std::unique_ptr<DeviceNotifier> notifier_; // 跨平台设备插拔通知

    // SOUI 控件事件映射（按钮 name）
    EVENT_MAP_BEGIN()
        EVENT_NAME_COMMAND(L"btn_stop_refresh", OnBtnStopRefresh)
        EVENT_NAME_COMMAND(L"btn_auto_refresh", OnBtnAutoRefresh)
        EVENT_NAME_COMMAND(L"btn_manual_refresh", OnBtnManualRefresh)
        EVENT_NAME_COMMAND(L"btn_baseline", OnBtnBaseline)
        EVENT_NAME_COMMAND(L"btn_copy", OnBtnCopy)
        // 列表选择变化（互斥选中：左清右 / 右清左）
        EVENT_NAME_HANDLER(L"lc_all", EVT_LC_SELCHANGED, OnListSelChanged)
        EVENT_NAME_HANDLER(L"lc_added", EVT_LC_SELCHANGED, OnListSelChanged)
        EVENT_NAME_HANDLER(L"lc_removed", EVT_LC_SELCHANGED, OnListSelChanged)
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