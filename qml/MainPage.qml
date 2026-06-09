import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Page {
    id: root
    required property QtObject usbManager
    required property QtObject theme

    property var currentDevices: []
    property var addedDevices: []
    property var removedDevices: []

    /// 从 UsbManager 同步加载数据
    function loadDevices() {
        currentDevices = theme.parseDevices(usbManager.getDevicesJson());
        addedDevices = theme.parseDevices(usbManager.getAddedDevicesJson());
        removedDevices = theme.parseDevices(usbManager.getRemovedDevicesJson());

        // 检查错误状态
        var err = usbManager.get_error();
        if (err) {
            errorLabel.text = err;
            errorBanner.visible = true;
            errorTimer.restart();
        }
    }

    Component.onCompleted: {
        // 连接 UsbManager 信号（QML 会自动将 Rust devices_changed 映射为 devicesChanged）
        usbManager.devicesChanged.connect(loadDevices);
        // 初始化时加载一次
        if (usbManager) {
            loadDevices();
        }
    }

    // 自动刷新 Timer（每 3 秒轮询，作为热插拔的兜底方案）
    Timer {
        interval: 3000
        running: true
        repeat: true
        onTriggered: {
            if (usbManager) {
                usbManager.refresh();
                // loadDevices 由 devicesChanged 信号触发
            }
        }
    }

    // 错误提示条（5 秒后自动消失）
    Timer {
        id: errorTimer
        interval: 5000
        onTriggered: errorBanner.visible = false
    }

    Rectangle {
        id: errorBanner
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: 32
        visible: false
        color: "#f8d7da"
        z: 10

        Label {
            id: errorLabel
            anchors.centerIn: parent
            color: "#721c24"
            font.pixelSize: theme.fontSizeBody
        }
    }

    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            spacing: theme.spacingMedium

            Label {
                text: "USB 设备查看器"
                font.pixelSize: theme.fontSizeTitle
                font.weight: Font.Medium
                color: theme.primaryColor
            }

            Item { Layout.fillWidth: true }

            ToolButton {
                text: "刷新"
                onClicked: {
                    if (!usbManager) return;
                    usbManager.refresh();
                }
            }

            ToolButton {
                text: "设为基准"
                onClicked: {
                    if (!usbManager) return;
                    usbManager.setBaseline();
                    root.loadDevices();
                }
            }

            ToolButton {
                text: "退出"
                onClicked: Qt.quit()
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: theme.spacingMedium
        spacing: theme.spacingMedium

        DeviceTable {
            id: currentTable
            title: "当前设备"
            model: root.currentDevices
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredHeight: root.height / 2 - theme.spacingMedium
            theme: root.theme
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: theme.spacingMedium

            DeviceTable {
                id: addTable
                title: "新增设备"
                model: root.addedDevices
                Layout.fillWidth: true
                Layout.fillHeight: true
                theme: root.theme
            }

            DeviceTable {
                id: removeTable
                title: "移除设备"
                model: root.removedDevices
                Layout.fillWidth: true
                Layout.fillHeight: true
                theme: root.theme
            }
        }
    }
}
