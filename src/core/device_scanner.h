#pragma once
#include <memory>
#include <string>
#include <vector>
#include "core/device_info.h"

namespace vpid {

// USB 扫描器抽象基类
class Scanner {
public:
    virtual ~Scanner() = default;
    virtual std::vector<USBDevice> scan() = 0;

    // 从设备 ID 提取 VID/PID："USB\VID_8087&PID_0024\.." -> vid="0x8087", pid="0x0024"
    static void extractVidPid(const std::string& device_id, std::string& vid, std::string& pid);

    // 提取反斜杠分隔的第三段作为序列号
    static std::string extractSerialFromDeviceId(const std::string& device_id);
};

// 跨平台工厂（在 windows_scanner.cpp / linux_scanner.cpp 中按平台实现）
std::unique_ptr<Scanner> createScanner();

// 统一入口
std::vector<USBDevice> scanUsbDevices();

// ---- 扫描取消 ----
// 扫描跑在工作线程里。退出时调用 RequestScanCancel() 让在途扫描尽快自行返回。
// 这不是优化而是**必需**：Windows 的 ExitProcess 会等所有线程结束，所以
// "等不到就放弃"的超时（WaitForSingleObject 超时）只会把问题换个地方——
// 线程还在跑 → 进程永远退不出（表现为窗口已关、任务管理器里进程残留）。
// 同一时刻至多一个扫描线程（MainDlg::scanning_ 互斥），故全局标志即可，无需传参。
// scanUsbDevices() 每次调用会先复位标志。
void RequestScanCancel();
bool ScanCancelRequested();

} // namespace vpid