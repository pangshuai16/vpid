import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15

Page {
    id: root
    property QtObject usbManager: null
    property QtObject theme: null

    property var currentDevices: []
    property var addedDevices: []
    property var removedDevices: []

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
        usbManager.devicesChanged.connect(loadDevices);
        if (usbManager) {
            loadDevices();
        }
    }

    // 自动刷新 Timer — pollChanges 仅检查版本号并发射信号，不阻塞主线程
    Timer {
        interval: 3000
        running: true
        repeat: true
        onTriggered: {
            if (usbManager) {
                usbManager.pollChanges();
            }
        }
    }

    // 错误提示条（5 秒后自动消失并清除文本）
    Timer {
        id: errorTimer
        interval: 5000
        onTriggered: {
            errorBanner.visible = false
            errorLabel.text = ""
        }
    }

    // Toast 通知 — 复制设备信息时短暂显示
    Timer {
        id: toastTimer
        interval: 1500
        onTriggered: toastBanner.visible = false
    }

    // 复制 Toast
    Rectangle {
        id: toastBanner
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: theme.spacingMedium
        height: 36
        width: toastLabel.implicitWidth + theme.spacingLarge * 2
        visible: false
        color: theme.primaryColor
        radius: theme.radiusSmall
        opacity: 0.9

        Label {
            id: toastLabel
            anchors.centerIn: parent
            text: "已复制到剪贴板"
            color: "#fff"
            font.pixelSize: theme.fontSizeBody
        }

        function show() {
            visible = true;
            toastTimer.restart();
        }
    }

    // 错误提示条
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
            onCopyToast: toastBanner.show()
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
