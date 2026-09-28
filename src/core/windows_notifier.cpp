// Windows 设备插拔通知：RegisterDeviceNotification
// 事件派发：WM_DEVICECHANGE 消息由宿主窗口消息映射处理（MainDlg::OnDeviceChange），
//           notifier 仅负责注册/注销监听（设备 GUID 过滤在消息层完成）。
#include "core/device_notifier.h"

#include <windows.h>
#include <dbt.h>

namespace vpid {

namespace {

class WindowsNotifier : public DeviceNotifier {
public:
    explicit WindowsNotifier(Callback) {}   // Windows 走消息循环，无需回调线程
    ~WindowsNotifier() override { Stop(); }

    bool Start(void *hwnd) override {
        if (!hwnd || notify_) return false;
        hwnd_ = (HWND)hwnd;
        DEV_BROADCAST_DEVICEINTERFACE_W ifc{};
        ifc.dbcc_size = sizeof(ifc);
        ifc.dbcc_devicetype = DBT_DEVTYP_DEVICEINTERFACE;
        ifc.dbcc_classguid = {0xA5DCBF10, 0x6530, 0x11D2,
                              {0x90, 0x1F, 0x00, 0xC0, 0x4F, 0xB9, 0x51, 0xED}};
        notify_ = RegisterDeviceNotificationW(hwnd_, &ifc, DEVICE_NOTIFY_WINDOW_HANDLE);
        return notify_ != nullptr;
    }

    void Stop() override {
        if (notify_) {
            UnregisterDeviceNotification(notify_);
            notify_ = nullptr;
        }
        hwnd_ = nullptr;
    }

private:
    HWND hwnd_ = nullptr;
    HDEVNOTIFY notify_ = nullptr;
};

} // namespace

std::unique_ptr<DeviceNotifier> DeviceNotifier::Create(Callback cb) {
    return std::make_unique<WindowsNotifier>(std::move(cb));
}

} // namespace vpid
