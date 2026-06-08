import QtQuick

/// 全局主题与设计 Token。作为普通 QtObject 由 main.qml 实例化并向下传递。
QtObject {
    // 颜色
    readonly property color primaryColor: "#0d6efd"
    readonly property color primaryHover: Qt.rgba(0.05, 0.42, 0.98, 0.1)
    readonly property color surfaceColor: "#f0f0f0"
    readonly property color textPrimary: "#333"
    readonly property color textSecondary: "#666"

    // 间距
    readonly property int spacingSmall: 4
    readonly property int spacingMedium: 10
    readonly property int radiusSmall: 4
    readonly property int radiusMedium: 5

    // 字号
    readonly property int fontSizeTitle: 18
    readonly property int fontSizeBody: 14

    // 表格列宽
    readonly property int colWidthProduct: 200
    readonly property int colWidthVendor: 200
    readonly property int colWidthId: 80
    readonly property int rowHeight: 32
    readonly property int headerHeight: 36

    /// 将整数格式化为带前导零的十六进制字符串（"0x" 前缀）。
    function formatHex(value, digits) {
        var d = digits === undefined ? 4 : digits;
        return "0x" + ((value >>> 0).toString(16)).padStart(d, "0");
    }

    /// 解析 USB 设备 JSON 字符串为 JS 数组（无设备时返回空数组）。
    function parseDevices(json) {
        if (!json) {
            return [];
        }
        try {
            return JSON.parse(json);
        } catch (e) {
            console.warn("parseDevices failed:", e);
            return [];
        }
    }
}
