import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ColumnLayout {
    id: root
    property string title: ""
    property var model: []
    required property QtObject theme

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
                    if (d.product) info += "\n" + d.product;
                    if (d.manufacturer) info += "\n" + d.manufacturer;
                    if (d.serial_number) info += "\nS/N: " + d.serial_number;
                    if (d.device_class_name) info += "\n" + d.device_class_name;
                    if (d.device_speed) info += "\nSpeed: " + d.device_speed;
                    var clip = Qt.application.clipboard;
                    if (clip) {
                        clip.text = info;
                        console.log("Copied device info");
                    }
                }
                gesturePolicy: TapHandler.ReleaseWithinBounds
            }
        }
    }
}
