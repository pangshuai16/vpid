// XP 兼容 stubs：
// 1. 为 YY-Thunks 提供 ProcessPrng 兜底
// 2. 为 CRT 的 __imp_ IAT 引用提供 GetFileSizeEx / SetFilePointerEx 兜底实现
//    这两个 API 是 Vista+，XP 的 kernel32.dll 没有导出。
//    CRT 通过 call [__imp__GetFileSizeEx@8] 间接引用（IAT），YY-Thunks 的
//    weak symbol obj 无法覆盖 IAT 引用，YY-Thunks 也没有这 2 个 API 的 thunk。
//
//    完整机制（与 src/xp_iat_stubs.asm 配合）：
//    - xp_iat_stubs.asm 定义 __imp__GetFileSizeEx@8 / __imp__SetFilePointerEx@20
//      为数据符号（函数指针），DIR32 重定位指向下面的 stub 函数
//    - .obj 在链接时优先于所有 .lib 处理 → 符号已定义 →
//      kernel32.lib 的导入库成员不会被拉入 → 不创建致命 IAT 导入
//    - CRT 的 call [__imp__GetFileSizeEx@8] 最终调到我们的 stub
//    - stub 内部回退到 XP 存在的 GetFileSize / SetFilePointer
//
//    这与 YY-Thunks kernel32.Lib 中 _WeakAlias 成员的机制相同，只是覆盖
//    YY-Thunks 未提供 thunk 的 2 个 API。

#ifdef _WIN32
#include <windows.h>

// ============================================================================
// YY-Thunks 依赖：ProcessPrng（crypt32.dll）
// YY-Thunks 期望 _ProcessPrng@8（2 参数），实际 API 是 4 参数。
// ============================================================================
extern "C" BOOL WINAPI ProcessPrng(HCRYPTPROV hProv, BYTE* pbBuffer)
{
    (void)hProv; (void)pbBuffer;
    return FALSE;
}

// ============================================================================
// GetFileSizeEx 兜底（Vista+ API，XP 无导出）
// CRT 通过 __imp__GetFileSizeEx@8 IAT 间接引用（见 xp_iat_stubs.asm）。
// 回退到 GetFileSize（XP 有），支持 >2GB 文件（高 32 位从 lpFileSizeHigh 取）。
// ============================================================================
extern "C" BOOL WINAPI GetFileSizeEx_stub(HANDLE hFile, PLARGE_INTEGER lpFileSize)
{
    DWORD hi = 0;
    DWORD lo = GetFileSize(hFile, &hi);
    if (lo == INVALID_FILE_SIZE && GetLastError() != NO_ERROR)
        return FALSE;
    if (lpFileSize)
    {
        lpFileSize->LowPart = lo;
        lpFileSize->HighPart = hi;
    }
    return TRUE;
}

// ============================================================================
// SetFilePointerEx 兜底（Vista+ API，XP 无导出）
// CRT 通过 __imp__SetFilePointerEx@20 IAT 间接引用（见 xp_iat_stubs.asm）。
// 回退到 SetFilePointer（XP 有），支持 64 位偏移（高分两次移动）。
// ============================================================================
extern "C" BOOL WINAPI SetFilePointerEx_stub(HANDLE hFile,
                                              LARGE_INTEGER liDistanceToMove,
                                              PLARGE_INTEGER lpNewFilePointer,
                                              DWORD dwMoveMethod)
{
    LONG hi = liDistanceToMove.HighPart;
    DWORD lo = SetFilePointer(hFile, liDistanceToMove.LowPart, &hi, dwMoveMethod);
    if (lo == INVALID_SET_FILE_POINTER && GetLastError() != NO_ERROR)
        return FALSE;
    if (lpNewFilePointer)
    {
        lpNewFilePointer->LowPart = lo;
        lpNewFilePointer->HighPart = hi;
    }
    return TRUE;
}

#endif // _WIN32
