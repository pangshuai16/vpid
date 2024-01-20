declare interface DeviceInfo {
  product_name: string;
  path: string;
  vendor_id: string;
  product_id: string;
  vendor_name: string;
}
declare interface RustDeviceInfo {
  product_name: string;
  path: string;
  vendor_id: number;
  product_id: number;
  vendor_name: string;
}
