---
name: cxx-qt
description: >-
  Use when writing, reviewing, or debugging CXX-Qt bridge code,
  QML integration with Rust, or Qt6 desktop applications built
  with Rust. Covers bridge macros, QObject methods, properties,
  signals, QML element registration, build.rs configuration, and
  common pitfalls specific to cxx-qt 0.8.x.
metadata:
  author: project-custom
  version: "1.0"
  cxx-qt-version: "0.8.x"
  category: framework
---

# CXX-Qt Development Skill

Guidelines for building Qt6 desktop applications with Rust using CXX-Qt 0.8.x.

## Bridge Pattern

### Standard QObject declaration

```rust
#[cxx_qt::bridge(namespace = "my_ns")]
mod qobject {
    unsafe extern "C++" {
        // C++ headers if needed
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]                    // Required for QML access
        #[qproperty(i32, number)]         // Optional properties
        type MyObject = super::MyObjectRust;

        #[qinvokable]                     // Makes method callable from QML
        #[cxx_name = "incrementNumber"]   // C++ camelCase name
        fn increment_number(self: Pin<&mut Self>);
    }
}
```

### Key rules

1. **Always use `extern "RustQt"`** — never `extern "Rust"` for QObject types
2. **Always add `#[qml_element]`** — without it, QML cannot see the type
3. **Use `#[qinvokable]`** on every method QML needs to call
4. **Use `#[cxx_name = "camelCase"]`** for C++/QML-friendly method names
5. **`self` parameter**: `&Self` for reads, `Pin<&mut Self>` for mutations
6. **Properties**: `#[qproperty(Type, name)]` auto-generates getter/setter/notify

### Method implementation

```rust
// Implement on the bridge type, NOT the raw struct
impl qobject::MyObject {
    fn increment_number(self: Pin<&mut Self>) {
        let prev = *self.number();
        self.set_number(prev + 1);  // Triggers NOTIFY signal
    }
}
```

## Build Configuration

### build.rs

```rust
use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("com.example.App")
            .qml_file("src/qml/main.qml")
            .qml_file("src/qml/MyComponent.qml"),
    )
    .file("src/ui/my_object.rs")     // Each bridge file
    .qt_module("Quick")              // Required Qt modules
    .qt_module("QuickControls2")
    .qt_module("Qml")
    .build();
}
```

### Cargo.toml dependencies

```toml
[dependencies]
cxx = "1.0"                          # Required by cxx_qt bridge macro
cxx-qt = "0.8"
cxx-qt-lib = { version = "0.8", features = ["qt_full"] }

[build-dependencies]
cxx-qt-build = { version = "=0.8.1", features = ["link_qt_object_files"] }
```

**Critical**: `cxx` must be a direct dependency — the bridge macro generates `::cxx::` paths.

## Application Entry Point

```rust
// main.rs
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

fn main() {
    cxx_qt::init_crate!(cxx_qt_lib);

    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/com/example/App/qml/main.qml"));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
```

### QRC path convention

URI `com.example.App` → QRC path `qrc:/qt/qml/com/example/App/qml/main.qml`

## QML Integration

### Instantiating a CXX-Qt type

```qml
import QtQuick
import QtQuick.Controls
import com.example.App 1.0

ApplicationWindow {
    MyObject {
        id: myObj
        number: 42
    }

    Button {
        text: "Click me"
        onClicked: myObj.incrementNumber()
    }
}
```

### Passing data via JSON strings

For complex data (like device lists), serialize in Rust and parse in QML:

```rust
// Rust bridge
#[qinvokable]
fn get_devices_json(self: &MyObject) -> String {
    serde_json::to_string(&self.devices).unwrap_or_default()
}
```

```qml
// QML
property var devices: JSON.parse(myObj.getDevicesJson())
```

## Common Pitfalls

### 1. "invalid ABI: found `C++`"
**Cause**: Missing `cxx` dependency.
**Fix**: Add `cxx = "1.0"` to `[dependencies]`.

### 2. QML type not found
**Cause**: Missing `#[qml_element]` on the bridge type.
**Fix**: Add `#[qml_element]` attribute.

### 3. Method not callable from QML
**Cause**: Missing `#[qinvokable]`.
**Fix**: Add `#[qinvokable]` to the method declaration in `extern "RustQt"`.

### 4. Build fails with "module not found"
**Cause**: Qt module not registered in build.rs.
**Fix**: Add `.qt_module("ModuleName")` — check with `qmake6 --query QT_MODULES`.

### 5. `unsafe extern "Rust"` vs `extern "RustQt"`
**Use `extern "RustQt"`** for QObject types. `unsafe extern "Rust"` is for non-QObject CXX bridges.

## QML Best Practices (Qt 6)

- **No version numbers** on imports (Qt 6 dropped the requirement)
- **Use `Layout.*`** for sizing in layouts — never bare `width`/`height`
- **Never mix `anchors` + `Layout.*`** on the same item
- **Use `required property`** for model roles in delegates
- **Access model roles as `model.roleName`** to avoid shadowing
- **Use `QtQuick.Controls.Basic`** style when customizing controls
- **Prefer `Loader`** for conditional/dynamic UI components
- **Use `Animator` types** instead of `NumberAnimation` for opacity/scale/position

## Review Checklist

When reviewing CXX-Qt code:

1. Bridge uses `extern "RustQt"` (not `extern "Rust"`)
2. All QML-visible types have `#[qml_element]`
3. All QML-callable methods have `#[qinvokable]`
4. `cxx = "1.0"` is in direct dependencies
5. `build.rs` registers all QML files and Qt modules
6. Method names use `#[cxx_name]` for camelCase in C++/QML
7. QML imports have no version numbers (Qt 6)
8. No mixing of `anchors` and `Layout.*` in QML
