; xp_iat_stubs.asm - XP 兼容：为 Vista+ API 定义本地 IAT 槽
;
; 背景：MSVC CRT（libucrt/libvcruntime，VC-LTL5 重定向版）内部通过
;   call dword ptr [__imp__GetFileSizeEx@8]
;   call dword ptr [__imp__SetFilePointerEx@20]
; 间接引用这两个 Vista+ API（XP 的 kernel32.dll 无导出）。
; kernel32.lib 的导入库成员定义了同名 __imp_ 符号，链接器会优先从导入库
; 解析 → 创建致命 IAT 导入。
;
; 解法：在本 .obj 中定义 __imp__GetFileSizeEx@8 / __imp__SetFilePointerEx@20
; 为数据符号（函数指针），初始值为本地 stub 函数地址。.obj 在链接时优先于
; 所有 .lib 处理，符号已被定义 → 导入库成员不会被拉入 → 无致命导入。
;
; 等价于 YY-Thunks kernel32.Lib 中 _WeakAlias 成员的机制，但覆盖
; YY-Thunks 未提供 thunk 的 2 个 API。
;
; 注意：.model flat 不带语言模型 → MASM 不做 C 下划线装饰，
; 符号名原样输出（与 COFF 期望的 __imp__Xxx@N / _Xxx@N 精确匹配）。

.686
.model flat
option casemap:none

; 外部 stub 函数（C++ extern "C" WINAPI，stdcall 修饰名 _Xxx@N）
externdef _GetFileSizeEx_stub@8:proc
externdef _SetFilePointerEx_stub@20:proc

; 导出 IAT 槽符号（原样，不加下划线）
public __imp__GetFileSizeEx@8
public __imp__SetFilePointerEx@20

.data
; IAT 槽：函数指针，指向本地 stub
; MASM 生成 DIR32 重定位，链接时填入 stub 函数地址
__imp__GetFileSizeEx@8    dd _GetFileSizeEx_stub@8
__imp__SetFilePointerEx@20 dd _SetFilePointerEx_stub@20

end
