#pragma once
#include <vector>
#include "core/device_info.h"

namespace vpid {

// 一次比对的结果
struct DeviceDiff {
    std::vector<USBDevice> added;    // 较基准新增
    std::vector<USBDevice> removed;  // 较基准移除
    bool changed = false;            // 唯一键集合是否发生变化
};

// 设备比对工具（新增/移除基准差值）
class DeviceComparer {
public:
    // 单次遍历完成 新增/移除 计算并给出是否有变化（避免对两份列表重复建集合）
    static DeviceDiff diff(const std::vector<USBDevice>& oldDev,
                           const std::vector<USBDevice>& newDev);

    // 兼容旧 UI（legacy GTK/Win32）：以输出参数方式返回新增/移除
    static void compare(const std::vector<USBDevice>& oldDev,
                        const std::vector<USBDevice>& newDev,
                        std::vector<USBDevice>& added,
                        std::vector<USBDevice>& removed);

    // 唯一键集合是否不一致
    static bool hasChanged(const std::vector<USBDevice>& a, const std::vector<USBDevice>& b);
};

} // namespace vpid