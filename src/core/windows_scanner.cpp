#include "core/windows_scanner.h"

#include "common/constants.h"

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <setupapi.h>
#include <initguid.h>

#include <algorithm>
#include <functional>
#include <optional>
#include <set>
#include <string>
#include <vector>

namespace vpid {

namespace {
// UTF-16 -> UTF-8（设备名等含非 ASCII，须保真）；定义见文件末尾，使用前需前向声明
std::string w2utf8(const std::wstring& w);

// 按唯一键去重（保留顺序）
std::vector<USBDevice> dedup(const std::vector<USBDevice>& in) {
    std::vector<USBDevice> out;
    std::set<DeviceKey> seen;
    for (const auto& d : in) {
        auto k = d.getUniqueKey();
        if (seen.insert(k).second) out.push_back(d);
    }
    return out;
}

int regDword(HKEY key, const wchar_t* name, int def) {
    DWORD v = 0, sz = sizeof(v), type = 0;
    if (RegQueryValueExW(key, name, nullptr, &type, (BYTE*)&v, &sz) == ERROR_SUCCESS && type == REG_DWORD)
        return (int)v;
    return def;
}

std::string regString(HKEY key, const wchar_t* name) {
    DWORD type = 0, sz = 0;
    if (RegQueryValueExW(key, name, nullptr, &type, nullptr, &sz) != ERROR_SUCCESS || sz == 0)
        return std::string();
    std::wstring buf(sz / sizeof(wchar_t) + 4, L'\0');
    DWORD ok = sz;
    if (RegQueryValueExW(key, name, nullptr, &type, (BYTE*)&buf[0], &ok) != ERROR_SUCCESS)
        return std::string();
    return w2utf8(buf);
}

std::string cleanRegistryString(std::string v) {
    if (v.empty()) return v;
    auto pos = v.find_last_of('\\');
    if (pos != std::string::npos) return v.substr(pos + 1);
    return v;
}

USBDevice buildDevice(const std::string& vid, const std::string& pid, const std::string& serial,
                      const std::string& name, const std::string& manufacturer,
                      const std::string& location, const std::string& driver,
                      const std::string& device_id, const std::string& status) {
    USBDevice d;
    d.vid = vid; d.pid = pid; d.serial = serial;
    d.name = name.empty() ? std::string("USB Device") : name;
    d.manufacturer = manufacturer;
    d.location = location; d.driver = driver;
    d.device_id = device_id; d.pnp_device_id = device_id;
    d.status = status; d.path = device_id;
    return d;
}

// UTF-16 -> UTF-8（设备名等含非 ASCII，须保真）
std::string w2utf8(const std::wstring& w) {
    if (w.empty()) return std::string();
    int n = WideCharToMultiByte(CP_UTF8, 0, w.data(), (int)w.size(), nullptr, 0, nullptr, nullptr);
    if (n <= 0) return std::string();
    std::string out(n, '\0');
    WideCharToMultiByte(CP_UTF8, 0, w.data(), (int)w.size(), &out[0], n, nullptr, nullptr);
    return out;
}
} // namespace

std::vector<USBDevice> WindowsScanner::scan() {
    auto setup = scanViaSetupApi();
    if (!setup.empty()) return setup;
    auto reg = scanViaRegistry();
    if (!reg.empty()) return reg;
    return {};
}

// ---------------- SetupAPI ----------------
std::vector<USBDevice> WindowsScanner::scanViaSetupApi() {
    std::vector<USBDevice> devices;
    HDEVINFO devs = SetupDiGetClassDevsW(nullptr, nullptr, nullptr,
                                         DIGCF_PRESENT | DIGCF_ALLCLASSES);
    if (devs == INVALID_HANDLE_VALUE) return devices;

    std::set<DeviceKey> seen;
    for (DWORD idx = 0;; ++idx) {
        SP_DEVINFO_DATA di{};
        di.cbSize = sizeof(SP_DEVINFO_DATA);
        if (!SetupDiEnumDeviceInfo(devs, idx, &di)) break;

        wchar_t instBuf[1024];
        if (!SetupDiGetDeviceInstanceIdW(devs, &di, instBuf, 1024, nullptr)) continue;
        std::wstring ws(instBuf);
        std::string instance_id = w2utf8(ws);

        std::string vid, pid;
        Scanner::extractVidPid(instance_id, vid, pid);
        if (vid.empty() || pid.empty()) continue;

        std::string serial = Scanner::extractSerialFromDeviceId(instance_id);
        DeviceKey key{vid, pid, serial};
        if (!seen.insert(key).second) continue;

        auto propStr = [&](DWORD which) -> std::string {
            DWORD type = 0, need = 0;
            if (!SetupDiGetDeviceRegistryPropertyW(devs, &di, which, &type,
                                                   nullptr, 0, &need)) {
                if (need == 0) return std::string();
            }
            std::vector<wchar_t> buf(need / sizeof(wchar_t) + 4, L'\0');
            if (!SetupDiGetDeviceRegistryPropertyW(devs, &di, which, &type,
                                                   (PBYTE)buf.data(), (DWORD)(buf.size()*sizeof(wchar_t)), &need))
                return std::string();
            return w2utf8(std::wstring(buf.data()));
        };

        std::string name = propStr(SPDRP_FRIENDLYNAME);
        if (name.empty()) name = propStr(SPDRP_DEVICEDESC);
        std::string manufacturer = propStr(SPDRP_MFG);
        std::string driver = propStr(SPDRP_DRIVER);
        std::string location = propStr(SPDRP_LOCATION_INFORMATION);

        devices.push_back(buildDevice(vid, pid, serial, name, manufacturer,
                                      location, driver, instance_id, kStatusConnected));
    }

    SetupDiDestroyDeviceInfoList(devs);
    return dedup(devices);
}

// ---------------- 注册表兜底 ----------------
std::vector<USBDevice> WindowsScanner::scanViaRegistry() {
    std::vector<USBDevice> devices;
    HKEY base = nullptr;
    if (RegOpenKeyExW(HKEY_LOCAL_MACHINE, L"SYSTEM\\CurrentControlSet\\Enum\\USB",
                      0, KEY_READ, &base) != ERROR_SUCCESS)
        return devices;

    for (DWORD i = 0;; ++i) {
        wchar_t name[256];
        DWORD nameLen = 256;
        if (RegEnumKeyExW(base, i, name, &nameLen, nullptr, nullptr, nullptr, nullptr) != ERROR_SUCCESS)
            break;
        std::wstring vs(name);
        std::string vidPidKeyName(vs.begin(), vs.end());

        std::string vid, pid;
        Scanner::extractVidPid(vidPidKeyName, vid, pid);
        if (vid.empty() || pid.empty()) continue;

        std::wstring subPath = L"SYSTEM\\CurrentControlSet\\Enum\\USB\\" + vs;
        HKEY vpKey = nullptr;
        if (RegOpenKeyExW(HKEY_LOCAL_MACHINE, subPath.c_str(), 0, KEY_READ, &vpKey) != ERROR_SUCCESS)
            continue;

        for (DWORD j = 0;; ++j) {
            wchar_t instName[256];
            DWORD instLen = 256;
            if (RegEnumKeyExW(vpKey, j, instName, &instLen, nullptr, nullptr, nullptr, nullptr) != ERROR_SUCCESS)
                break;
            std::wstring wis(instName);
            std::string serialPart(wis.begin(), wis.end());

            std::wstring instPath = subPath + L"\\" + wis;
            HKEY instKey = nullptr;
            if (RegOpenKeyExW(HKEY_LOCAL_MACHINE, instPath.c_str(), 0, KEY_READ, &instKey) != ERROR_SUCCESS)
                continue;

            // 连接判定：ConfigManagerErrorCode==0 且未标记 ConfigFlags bit2
            int errCode = regDword(instKey, L"ConfigManagerErrorCode", 0);
            int configFlags = regDword(instKey, L"ConfigFlags", 0);
            bool connected = (errCode == 0) && ((configFlags & 0x4) == 0);

            std::string deviceId = "USB\\VID_" + vid.substr(2) + "&PID_" + pid.substr(2) + "\\" + serialPart;

            if (connected) {
                std::string name = cleanRegistryString(regString(instKey, L"FriendlyName"));
                if (name.empty()) name = cleanRegistryString(regString(instKey, L"DeviceDesc"));
                std::string manufacturer = cleanRegistryString(regString(instKey, L"Mfg"));
                std::string driver = regString(instKey, L"Driver");
                std::string location = regString(instKey, L"LocationInformation");
                devices.push_back(buildDevice(vid, pid, serialPart, name, manufacturer,
                                              location, driver, deviceId, kStatusConnected));
            }
            RegCloseKey(instKey);
        }
        RegCloseKey(vpKey);
    }
    RegCloseKey(base);
    return dedup(devices);
}

std::unique_ptr<Scanner> createScanner() { return std::make_unique<WindowsScanner>(); }

} // namespace vpid