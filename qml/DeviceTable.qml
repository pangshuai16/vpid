import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15

ColumnLayout {
    id: root
    property string title: ""
    property var model: []
    property QtObject theme: null

    // Toast 提示
    property bool showToast: false
    signal copyToast(string text)

    spacing: theme.spacingSmall

    Label {
        text: root.title
        font.pixelSize: theme.fontSizeTitle
        font.weight: Font.Medium
        color: theme.primaryColor
    }

    // 表头
    Rectangle {
        Layout.fillWidth: true
        Layout.preferredHeight: 36
        color: theme.surfaceColor
        radius: theme.radiusSmall

        RowLayout {
            anchors.fill: parent
            anchors.margins: theme.spacingSmall

            Text { Layout.preferredWidth: theme.colWidthProduct; text: "设备"; font.bold: true; color: theme.textPrimary; elide: Text.ElideRight }
            Text { Layout.preferredWidth: theme.colWidthVendor; text: "厂商"; font.bold: true; color: theme.textPrimary; elide: Text.ElideRight }
            Text { Layout.preferredWidth: theme.colWidthId; text: "VID"; font.bold: true; color: theme.textPrimary }
            Text { Layout.preferredWidth: theme.colWidthId; text: "PID"; font.bold: true; color: theme.textPrimary }
            Text { Layout.preferredWidth: theme.colWidthClass; text: "类别"; font.bold: true; color: theme.textPrimary }
            Text { Layout.preferredWidth: theme.colWidthSpeed; text: "速度"; font.bold: true; color: theme.textPrimary }
            Text { Layout.preferredWidth: theme.colWidthSerial; text: "序列号"; font.bold: true; color: theme.textPrimary; elide: Text.ElideRight }
        }
    }

    ListView {
        id: listView
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        model: root.model

        delegate: Rectangle {
            id: row
            property var device: model.modelData || model
            property bool rowHovered: rowMouse.containsMouse

            width: ListView.view ? ListView.view.width : 0
            implicitHeight: 32
            color: row.rowHovered ? theme.primaryHover : "transparent"

            RowLayout {
                anchors.fill: parent
                anchors.margins: theme.spacingSmall

                Text {
                    Layout.preferredWidth: theme.colWidthProduct
                    text: row.device.product || "Unknown"
                    elide: Text.ElideRight
                    maximumLineCount: 1
                    color: row.rowHovered ? theme.primaryColor : theme.textPrimary
                }
                Text {
                    Layout.preferredWidth: theme.colWidthVendor
                    text: row.device.manufacturer || ""
                    elide: Text.ElideRight
                    maximumLineCount: 1
                    color: theme.textSecondary
                }
                Text {
                    Layout.preferredWidth: theme.colWidthId
                    text: theme.formatHex(row.device.vendor_id, 4)
                    color: theme.textSecondary
                    font.family: "monospace"
                }
                Text {
                    Layout.preferredWidth: theme.colWidthId
                    text: theme.formatHex(row.device.product_id, 4)
                    color: theme.textSecondary
                    font.family: "monospace"
                }
                Text {
                    Layout.preferredWidth: theme.colWidthClass
                    text: row.device.device_class_name || ""
                    elide: Text.ElideRight
                    maximumLineCount: 1
                    color: theme.textSecondary
                }
                Text {
                    Layout.preferredWidth: theme.colWidthSpeed
                    text: row.device.device_speed || ""
                    color: theme.textSecondary
                }
                Text {
                    Layout.preferredWidth: theme.colWidthSerial
                    text: row.device.serial_number || ""
                    elide: Text.ElideRight
                    maximumLineCount: 1
                    color: theme.textSecondary
                }
            }

            MouseArea {
                id: rowMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: {
                    var d = row.device;
                    var info = "VID:PID " + theme.formatHex(d.vendor_id, 4) + ":" + theme.formatHex(d.product_id, 4);
                    if (d.product) info += " " + d.product;
                    if (d.manufacturer) info += " (" + d.manufacturer + ")";
                    var clip = Qt.application.clipboard;
                    if (clip) {
                        clip.text = info;
                        root.copyToast(info);
                    }
                }
            }
        }

        // 空状态提示
        Rectangle {
            anchors.fill: parent
            visible: root.model.length === 0
            color: theme.backgroundColor
            Label {
                anchors.centerIn: parent
                text: root.title === "新增设备" || root.title === "移除设备" ? "无变化" : "未检测到设备"
                color: theme.textSecondary
                font.pixelSize: theme.fontSizeBody
            }
        }
    }
}
