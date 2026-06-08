import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import app 1.0

ApplicationWindow {
    id: mainWindow
    visible: true
    width: 1000
    height: 700
    title: "USB 设备查看器"

    Theme {
        id: theme
    }

    UsbManager {
        id: usbManager
    }

    MainPage {
        anchors.fill: parent
        usbManager: usbManager
        theme: theme
    }
}
