/// 获取 USB 类代码对应的名称
pub fn usb_class_name(class: u8) -> &'static str {
    match class {
        0x00 => "Device Class",
        0x01 => "Audio",
        0x02 => "CDC (Communication)",
        0x03 => "HID (Human Interface)",
        0x05 => "Physical",
        0x06 => "Image (Printer/Scanner)",
        0x07 => "Printer",
        0x08 => "Mass Storage",
        0x09 => "Hub",
        0x0A => "CDC-Data",
        0x0B => "Smart Card",
        0x0D => "Content Security",
        0x0E => "Video",
        0x0F => "Personal Healthcare",
        0x10 => "Audio/Video",
        0xDC => "Diagnostic Device",
        0xE0 => "Wireless Controller",
        0xEF => "Miscellaneous",
        0xFE => "Application Specific",
        0xFF => "Vendor Specific",
        _ => "Unknown",
    }
}