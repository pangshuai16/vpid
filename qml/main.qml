import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
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
