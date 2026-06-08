pub mod ui;
pub mod usb;

use std::ffi::CString;

use qmetaobject::*;

fn main() {
    env_logger::init();

    // WSL2 / 软件渲染兼容：使用 QPainter 软件渲染后端
    if std::env::var("QT_QUICK_BACKEND").is_err() {
        // SAFETY: 单线程启动时设置环境变量，无其他线程竞争
        unsafe {
            std::env::set_var("QT_QUICK_BACKEND", "software");
        }
    }

    // 注册 UsbManager 类型，QML 可通过 "app" 模块引用
    let uri = CString::new("app").unwrap();
    let name = CString::new("UsbManager").unwrap();
    qml_register_type::<ui::usb_manager::UsbManager>(&uri, 1, 0, &name);

    // 创建 QML 引擎
    let mut engine = QmlEngine::new();

    // 加载主 QML 文件（从文件系统，非 QRC）
    engine.load_file("qml/main.qml".into());

    // 启动事件循环
    engine.exec();
}
