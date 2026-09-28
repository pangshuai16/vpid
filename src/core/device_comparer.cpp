#include "core/device_comparer.h"
#include <set>

namespace vpid {

DeviceDiff DeviceComparer::diff(const std::vector<USBDevice>& oldDev,
                                const std::vector<USBDevice>& newDev) {
    DeviceDiff result;
    std::set<DeviceKey> oldKeys, newKeys;
    for (const auto& d : oldDev) oldKeys.insert(d.getUniqueKey());
    for (const auto& d : newDev) newKeys.insert(d.getUniqueKey());

    // 集合相等即无变化（std::set 的 operator!= 做集合语义比较）
    result.changed = (oldKeys != newKeys);
    for (const auto& d : newDev)
        if (!oldKeys.count(d.getUniqueKey())) result.added.push_back(d);
    for (const auto& d : oldDev)
        if (!newKeys.count(d.getUniqueKey())) result.removed.push_back(d);
    return result;
}

void DeviceComparer::compare(const std::vector<USBDevice>& oldDev,
                             const std::vector<USBDevice>& newDev,
                             std::vector<USBDevice>& added,
                             std::vector<USBDevice>& removed) {
    DeviceDiff d = diff(oldDev, newDev);
    added = std::move(d.added);
    removed = std::move(d.removed);
}

bool DeviceComparer::hasChanged(const std::vector<USBDevice>& a, const std::vector<USBDevice>& b) {
    std::set<DeviceKey> ka, kb;
    for (const auto& d : a) ka.insert(d.getUniqueKey());
    for (const auto& d : b) kb.insert(d.getUniqueKey());
    if (ka.size() != kb.size()) return true;
    for (const auto& k : ka) if (!kb.count(k)) return true;
    return false;
}

} // namespace vpid