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

            Text { Layout.preferredWidth: 200; text: "设备"; font.bold: true; color: theme.textPrimary; elide: Text.ElideRight }
            Text { Layout.preferredWidth: 200; text: "厂商"; font.bold: true; color: theme.textPrimary; elide: Text.ElideRight }
            Text { Layout.preferredWidth: 80; text: "VID"; font.bold: true; color: theme.textPrimary }
            Text { Layout.preferredWidth: 80; text: "PID"; font.bold: true; color: theme.textPrimary }
        }
    }

    ListView {
        id: listView
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        model: root.model
        implicitHeight: contentHeight

        delegate: Rectangle {
            id: row
            required property var modelData
            required property int index
            readonly property bool hovered: rowMouse.containsMouse

            width: ListView.view ? ListView.view.width : 0
            implicitHeight: 32
            color: row.hovered ? theme.primaryHover : "transparent"

            RowLayout {
                anchors.fill: parent
                anchors.margins: theme.spacingSmall

                Text {
                    Layout.preferredWidth: 200
                    text: row.modelData.product || "Unknown"
                    elide: Text.ElideRight
                    maximumLineCount: 1
                    color: row.hovered ? theme.primaryColor : theme.textPrimary
                }
                Text {
                    Layout.preferredWidth: 200
                    text: row.modelData.manufacturer || ""
                    elide: Text.ElideRight
                    maximumLineCount: 1
                    color: theme.textSecondary
                }
                Text {
                    Layout.preferredWidth: 80
                    text: theme.formatHex(row.modelData.vendor_id, 4)
                    color: theme.textSecondary
                    font.family: "monospace"
                }
                Text {
                    Layout.preferredWidth: 80
                    text: theme.formatHex(row.modelData.product_id, 4)
                    color: theme.textSecondary
                    font.family: "monospace"
                }
            }

            TapHandler {
                id: rowMouse
                onTapped: {
                    var clip = Qt.application.clipboard;
                    if (clip) {
                        clip.text = theme.formatHex(row.modelData.vendor_id, 4) + ":" +
                                    theme.formatHex(row.modelData.product_id, 4);
                        console.log("Copied:", clip.text);
                    }
                }
                gesturePolicy: TapHandler.ReleaseWithinBounds
            }
        }
    }
}
