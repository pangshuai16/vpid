#include "core/device_scanner.h"
#include "common/constants.h"
#include <algorithm>
#include <atomic>
#include <cctype>
#include <regex>

namespace vpid {

namespace {
// 全局扫描取消标志。同一时刻至多一个扫描线程（MainDlg::scanning_ 互斥），
// 故用全局量即可；静态存储期 → 生命周期长于任何 MainDlg 实例，无线程持它导致的 UAF。
std::atomic<bool> g_scanCancel(false);
}  // namespace

void RequestScanCancel() { g_scanCancel = true; }
bool ScanCancelRequested() { return g_scanCancel; }

void Scanner::extractVidPid(const std::string& device_id, std::string& vid, std::string& pid) {
    static const std::regex vidRe(kVidPattern, std::regex::icase);
    static const std::regex pidRe(kPidPattern, std::regex::icase);
    vid.clear();
    pid.clear();
    std::smatch m;
    if (std::regex_search(device_id, m, vidRe) && m[1].matched) {
        std::string hex = m[1].str();
        std::transform(hex.begin(), hex.end(), hex.begin(),
                       [](unsigned char c) { return (char)std::toupper((int)c); });
        vid = "0x" + hex;
    }
    if (std::regex_search(device_id, m, pidRe) && m[1].matched) {
        std::string hex = m[1].str();
        std::transform(hex.begin(), hex.end(), hex.begin(),
                       [](unsigned char c) { return (char)std::toupper((int)c); });
        pid = "0x" + hex;
    }
}

std::string Scanner::extractSerialFromDeviceId(const std::string& device_id) {
    if (device_id.empty()) return std::string();
    auto pos = device_id.find('\\');
    if (pos == std::string::npos) return std::string();
    auto pos2 = device_id.find('\\', pos + 1);
    if (pos2 == std::string::npos) return std::string();
    return device_id.substr(pos2 + 1);
}

std::vector<USBDevice> scanUsbDevices() {
    g_scanCancel = false;  // 新一轮扫描，复位取消标志
    auto s = createScanner();
    return s->scan();
}

} // namespace vpid