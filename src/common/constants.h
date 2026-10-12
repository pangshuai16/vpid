#pragma once
#include <string>

namespace vpid {

// 纯常量配置，无 UI 框架依赖
constexpr const char* kAppName    = "USB设备ID查看";
// 当前版本。发行版版本号以 platform/resources/app.rc 的 VS_VERSION_INFO 为准
// （资源管理器"属性→详细信息"读的就是那里），改版本时两处需同步。
// 目前未在 UI 展示，预留给将来的"关于"对话框。
constexpr const char* kAppVersion = "1.1.0";
constexpr const char* kAppAuthor  = "USB Manager";

constexpr int kAutoRefreshIntervalMs = 100;
constexpr int kDefaultWindowWidth    = 1280;
constexpr int kDefaultWindowHeight   = 720;
constexpr int kMinWindowWidth        = 960;
constexpr int kMinWindowHeight       = 600;

constexpr const char* kStatusConnected = "Connected";

// 注册表 USB 枚举路径（Windows 兜底扫描）
constexpr const char* kRegistryUsbBasePath = R"(SYSTEM\CurrentControlSet\Enum\USB)";

// VID/PID 正则模式
constexpr const char* kVidPattern = R"(VID_([0-9A-Fa-f]{4}))";
constexpr const char* kPidPattern = R"(PID_([0-9A-Fa-f]{4}))";

} // namespace vpid