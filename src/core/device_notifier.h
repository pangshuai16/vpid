#pragma once
#include <memory>
#include <functional>

namespace vpid {

// 设备插拔通知抽象：UI 层不直接依赖平台 API
// Windows: RegisterDeviceNotification + WM_DEVICECHANGE（消息循环驱动）
// Linux:   udev monitor（libudev 线程轮询）
// 回调触发时机：设备树发生变化时（由平台实现按需去抖/合并）
class DeviceNotifier {
public:
    // 通知回调（UI 线程调用，保持轻量：仅触发一次重新扫描）
    using Callback = std::function<void()>;

    virtual ~DeviceNotifier() = default;

    // 启动监听。hwnd 仅 Windows 需要（注册设备通知的目标窗口）。
    // 返回 false 表示平台不支持（调用方应退化为定时轮询）。
    virtual bool Start(void *hwnd) = 0;
    virtual void Stop() = 0;

    // 跨平台工厂（windows_notifier.cpp / linux_notifier.cpp 按平台实现）
    static std::unique_ptr<DeviceNotifier> Create(Callback cb);
};

} // namespace vpid
