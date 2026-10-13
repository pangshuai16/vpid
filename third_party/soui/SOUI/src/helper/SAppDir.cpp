#include "souistd.h"
#include "helper/SAppDir.h"
#ifdef WIN32
#include <shellapi.h>
#include <helper/souidlgs.h>
#else
#include <commdlg.h>
#endif /**< WIN32 */

SNSBEGIN
SAppDir::SAppDir(HINSTANCE hInst)
{
    TCHAR apppath[MAX_PATH] = { 0 };
    if (GetModuleFileName(hInst, apppath, MAX_PATH))
    {
        TCHAR appfullpath[MAX_PATH];
        GetFullPathName(apppath, MAX_PATH, appfullpath, NULL);
        LPTSTR lpInsertPos = _tcsrchr(appfullpath, PATH_SLASH);
        if (lpInsertPos)
        {
            *lpInsertPos = 0;
        }
        m_strAppPath = appfullpath;
    }
}

const SStringT &SAppDir::AppDir()
{
    return m_strAppPath;
}

typedef HRESULT(WINAPI *FunSHCreateItemFromParsingName)(PCWSTR, IBindCtx *, REFIID, void **);

BOOL SOUI_EXP SSelectFolder(TCHAR szPath[MAX_PATH], LPCTSTR pszTitle, LPCTSTR initPath)
{
#ifdef WIN32
    SStringT defPath(initPath);
    SStringT ret;
    bool bNewDialog = false;
    // ---- [XP 兼容补丁] IFileOpenDialog / FOS_PICKFOLDERS 是 Vista+ API。
    // XP 目标下（_WIN32_WINNT=0x0501）shobjidl.h 隐藏了 CShellFileOpenDialog 与
    // FOS_PICKFOLDERS 的声明，编译即报 C2065。本项目的目标宏是全局的（见
    // CMakeLists.txt 顶部 add_compile_definitions），故这里必须自己门控，
    // XP 下让 bNewDialog 保持 false，走下方 CFolderDialog（SHBrowseForFolder）
    // 回退分支——功能等价（都是系统"浏览文件夹"对话框），且 XP 可用。
#if (_WIN32_WINNT >= 0x0600)
    do
    {
        CShellFileOpenDialog dlg;
        if (dlg.IsNull())
            break;
        DWORD dwOptions;
        if (!SUCCEEDED(dlg.GetPtr()->GetOptions(&dwOptions)))
            break;
        if (!SUCCEEDED(dlg.GetPtr()->SetOptions(dwOptions | FOS_PICKFOLDERS)))
            break;
        if (!defPath.IsEmpty())
        {
            HMODULE hShell = LoadLibrary(_T("shell32.dll"));
            if (hShell)
            {
                IShellItem *folderItem = NULL;
                SStringW strDefPath = S_CT2W(defPath);
                FunSHCreateItemFromParsingName funSHCreateItemFromParsingName = (FunSHCreateItemFromParsingName)GetProcAddress(hShell, "SHCreateItemFromParsingName");
                if (funSHCreateItemFromParsingName && funSHCreateItemFromParsingName(strDefPath.c_str(), NULL, IID_PPV_ARGS(&folderItem)) == S_OK)
                {
                    dlg.GetPtr()->SetFolder(folderItem);
                    folderItem->Release();
                }
                FreeLibrary(hShell);
            }
        }
        if (pszTitle)
        {
            dlg.GetPtr()->SetTitle(S_CT2W(pszTitle));
        }
        bNewDialog = true;
        if (IDOK == dlg.DoModal())
        {
            SStringW strDir;
            dlg.GetFilePath(strDir);
            ret = S_CW2T(strDir);
        }
    } while (false);
#endif
    if (!bNewDialog)
    {
        CFolderDialog folderDialog(0, pszTitle);
        if (folderDialog.DoModal() == IDOK)
        {
            ret = folderDialog.GetFolderPath();
        }
    }
    if (ret.IsEmpty())
        return FALSE;
    _tcscpy(szPath, ret);
    return TRUE;
#else
    BROWSEINFO info = { 0 };
    info.nMaxPath = MAX_PATH;
    info.lpszPath = szPath;
    info.strlRoot = initPath;
    info.lpszTitle = pszTitle;
    return PickFolder(&info);
#endif
}
SNSEND