// Linux 设备插拔通知：udev monitor（libudev 线程轮询）
// 依赖：libudev（pkg-config: libudev）。若未链接（如精简构建），
// Start 返回 false，UI 层退化为定时轮询（本项目默认 100ms 刷新，已足够）。
#include "core/device_notifier.h"

#include <thread>
#include <atomic>
#include <chrono>
#include <memory>

namespace vpid {

namespace {

class LinuxNotifier : public DeviceNotifier {
public:
    explicit LinuxNotifier(Callback cb) : cb_(std::move(cb)) {}
    ~LinuxNotifier() override { Stop(); }

    bool Start(void *) override {
        if (running_) return false;
        // 尝试打开 udev 监控；库缺失时放行 → 返回 false 由调用方降级轮询
        if (!tryInit()) return false;
        running_ = true;
        thread_ = std::thread([this] { run(); });
        return true;
    }

    void Stop() override {
        running_ = false;
        if (thread_.joinable()) thread_.join();
        cleanup();
    }

private:
    bool tryInit();
    void run();
    void cleanup();

    Callback cb_;
    std::atomic<bool> running_{false};
    std::thread thread_;
    void *mon_ = nullptr;   // udev_monitor*
    void *udev_ = nullptr;  // udev*
};

// ---- libudev 动态加载（fi compatible with systems without libudev at link time）----
// 用 dlopen 避免 CMake 必须强制链接 libudev；缺失时降级轮询。
#include <dlfcn.h>
#include <poll.h>

using UdevNewFn            = void *(*)(void);
using UdevMonitorNewFn     = void *(*)(void);
using UdevMonitorEnableFn  = int (*)(void *);
using UdevMonitorGetFdFn   = int (*)(void *);
using UdevMonitorReceiveFn = void *(*)(void *);
using UdevMonitorFilterAddFn = int (*)(void *, const char *, const char *);

static UdevNewFn g_udev_new = nullptr;
static UdevMonitorNewFn g_mon_new = nullptr;
static UdevMonitorEnableFn g_mon_enable = nullptr;
static UdevMonitorGetFdFn g_mon_fd = nullptr;
static UdevMonitorReceiveFn g_mon_recv = nullptr;
static UdevMonitorFilterAddFn g_mon_filter = nullptr;
static void *g_lib = nullptr;

bool loadUdev() {
    if (g_lib) return true;
    g_lib = dlopen("libudev.so.1", RTLD_LAZY | RTLD_LOCAL);
    if (!g_lib) g_lib = dlopen("libudev.so.0", RTLD_LAZY | RTLD_LOCAL);
    if (!g_lib) return false;
    g_udev_new = (UdevNewFn)dlsym(g_lib, "udev_new");
    g_mon_new = (UdevMonitorNewFn)dlsym(g_lib, "udev_monitor_new_from_netlink");
    g_mon_enable = (UdevMonitorEnableFn)dlsym(g_lib, "udev_monitor_enable_receiving");
    g_mon_fd = (UdevMonitorGetFdFn)dlsym(g_lib, "udev_monitor_get_fd");
    g_mon_recv = (UdevMonitorReceiveFn)dlsym(g_lib, "udev_monitor_receive_device");
    g_mon_filter = (UdevMonitorFilterAddFn)dlsym(g_lib, "udev_monitor_filter_add_match_subsystem_devtype");
    return g_udev_new && g_mon_new && g_mon_enable && g_mon_fd && g_mon_recv;
}

bool LinuxNotifier::tryInit() {
    if (!loadUdev()) return false;
    udev_ = g_udev_new();
    if (!udev_) return false;
    // 新式 API：udev_monitor_new_from_netlink(udev, "udev")
    mon_ = ((void *(*)(void *, const char *))g_mon_new)(udev_, "udev");
    if (!mon_) { cleanup(); return false; }
    if (g_mon_filter) g_mon_filter(mon_, "usb", "usb_device");
    if (g_mon_enable(mon_) < 0) { cleanup(); return false; }
    return true;
}

void LinuxNotifier::run() {
    int fd = g_mon_fd(mon_);
    struct pollfd pfd{fd, POLLIN, 0};
    while (running_) {
        int r = poll(&pfd, 1, 200);   // 200ms 去抖
        if (r > 0 && (pfd.revents & POLLIN)) {
            g_mon_recv(mon_);          // 消费事件
            if (cb_) cb_();
        }
    }
}

void LinuxNotifier::cleanup() {
    if (mon_ && g_lib) {
        void (*free_mon)(void *) = (void (*)(void *))dlsym(g_lib, "udev_monitor_unref");
        if (free_mon) free_mon(mon_);
    }
    if (udev_ && g_lib) {
        void (*free_udev)(void *) = (void (*)(void *))dlsym(g_lib, "udev_unref");
        if (free_udev) free_udev(udev_);
    }
    mon_ = nullptr;
    udev_ = nullptr;
}

} // namespace

std::unique_ptr<DeviceNotifier> DeviceNotifier::Create(Callback cb) {
    return std::make_unique<LinuxNotifier>(std::move(cb));
}

} // namespace vpid