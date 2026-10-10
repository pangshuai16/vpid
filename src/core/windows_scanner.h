#pragma once
#include "core/device_scanner.h"

namespace vpid {

// Windows：SetupAPI（主）+ 注册表（兜底）
class WindowsScanner : public Scanner {
public:
    std::vector<USBDevice> scan() override;
private:
    std::vector<USBDevice> scanViaSetupApi();
    std::vector<USBDevice> scanViaRegistry();
};

} // namespace vpid
