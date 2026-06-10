import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ColumnLayout {
    id: root
    property string title: ""
    property var model: []
    required property QtObject theme

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
            required property var modelData
            readonly property bool hovered: rowMouse.containsMouse

            width: ListView.view ? ListView.view.width : 0
            implicitHeight: 32
            color: row.hovered ? theme.primaryHover : "transparent"

            RowLayout {
                anchors.fill: parent
                anchors.margins: theme.spacingSmall

                Text {
                    Layout.preferredWidth: theme.colWidthProduct
                    text: row.modelData.product || "Unknown"
                    elide: Text.ElideRight
                    maximumLineCount: 1
                    color: row.hovered ? theme.primaryColor : theme.textPrimary
                }
                Text {
                    Layout.preferredWidth: theme.colWidthVendor
                    text: row.modelData.manufacturer || ""
                    elide: Text.ElideRight
                    maximumLineCount: 1
                    color: theme.textSecondary
                }
                Text {
                    Layout.preferredWidth: theme.colWidthId
                    text: theme.formatHex(row.modelData.vendor_id, 4)
                    color: theme.textSecondary
                    font.family: "monospace"
                }
                Text {
                    Layout.preferredWidth: theme.colWidthId
                    text: theme.formatHex(row.modelData.product_id, 4)
                    color: theme.textSecondary
                    font.family: "monospace"
                }
                Text {
                    Layout.preferredWidth: theme.colWidthClass
                    text: row.modelData.device_class_name || ""
                    elide: Text.ElideRight
                    maximumLineCount: 1
                    color: theme.textSecondary
                }
                Text {
                    Layout.preferredWidth: theme.colWidthSpeed
                    text: row.modelData.device_speed || ""
                    color: theme.textSecondary
                }
                Text {
                    Layout.preferredWidth: theme.colWidthSerial
                    text: row.modelData.serial_number || ""
                    elide: Text.ElideRight
                    maximumLineCount: 1
                    color: theme.textSecondary
                }
            }

            TapHandler {
                id: rowMouse
                onTapped: {
                    var d = row.modelData;
                    var info = "VID:PID " + theme.formatHex(d.vendor_id, 4) + ":" + theme.formatHex(d.product_id, 4);
                    if (d.product) info += " " + d.product;
                    if (d.manufacturer) info += " (" + d.manufacturer + ")";
                    var clip = Qt.application.clipboard;
                    if (clip) {
                        clip.text = info;
                        root.copyToast(info);
                    }
                }
                gesturePolicy: TapHandler.ReleaseWithinBounds
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
