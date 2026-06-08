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
    }

    Component.onCompleted: {
        // 初始化时加载一次
        if (usbManager) {
            loadDevices();
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
                    usbManager.refresh();
                    root.loadDevices();
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
