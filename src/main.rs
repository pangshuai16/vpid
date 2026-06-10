use std::ffi::CString;
use std::path::PathBuf;

use qmetaobject::*;

fn main() {
    env_logger::init();

    // WSL2 / 软件渲染兼容：使用软件渲染后端
    if std::env::var("QT_QUICK_BACKEND").is_err() {
        std::env::set_var("QT_QUICK_BACKEND", "software");
    }

    // 解析 QML 路径（支持可执行文件同目录下的 qml/ 目录）
    let qml_path = resolve_qml_path("qml/main.qml");
    if !qml_path.exists() {
        eprintln!("Error: QML file not found: {}", qml_path.display());
        std::process::exit(1);
    }

    // 注册 UsbManager 类型，QML 中通过 import app 1.0 使用 UsbManager {}
    let uri = CString::new("app").unwrap();
    let name = CString::new("UsbManager").unwrap();
    qml_register_type::<vpid::ui::usb_manager::UsbManager>(&uri, 1, 0, &name);

    // 创建 QML 引擎
    let mut engine = QmlEngine::new();

    // 加载主 QML 文件
    engine.load_file(qml_path.into());

    // 启动事件循环
    engine.exec();
}

/// 解析 QML 文件路径，优先查找可执行文件同目录
fn resolve_qml_path(relative: &str) -> PathBuf {
    // 尝试当前工作目录
    let cwd_path = PathBuf::from(relative);
    if cwd_path.exists() {
        return cwd_path;
    }

    // 尝试可执行文件同目录
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let exe_path = dir.join(relative);
            if exe_path.exists() {
                return exe_path;
            }
        }
    }

    cwd_path
}
