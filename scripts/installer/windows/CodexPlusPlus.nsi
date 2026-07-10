Unicode true
!include "MUI2.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!define ROOT "..\..\.."

Name "Chimera Codex"
OutFile "${ROOT}\dist\windows\ChimeraCodex-${VERSION}-windows-x64-setup.exe"
; 一期保持原版安装目录，支持覆盖升级
InstallDir "$LOCALAPPDATA\Programs\Codex++"
InstallDirRegKey HKCU "Software\Codex++" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma

!define MUI_ICON "${ROOT}\apps\codex-plus-manager\src-tauri\icons\icon.ico"
!define MUI_UNICON "${ROOT}\apps\codex-plus-manager\src-tauri\icons\icon.ico"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  SetOutPath "$INSTDIR"

  nsExec::ExecToLog 'taskkill /IM codex-plus-plus.exe /F'
  Pop $0
  nsExec::ExecToLog 'taskkill /IM codex-plus-plus-manager.exe /F'
  Pop $0

  ; 先写入 staging，成功后再替换，避免升级失败时提前删掉现有二进制
  File "/oname=codex-plus-plus.exe.new" "${ROOT}\dist\windows\app\codex-plus-plus.exe"
  File "/oname=codex-plus-plus-manager.exe.new" "${ROOT}\dist\windows\app\codex-plus-plus-manager.exe"
  Delete "$INSTDIR\codex-plus-plus.exe"
  Rename "$INSTDIR\codex-plus-plus.exe.new" "$INSTDIR\codex-plus-plus.exe"
  Delete "$INSTDIR\codex-plus-plus-manager.exe"
  Rename "$INSTDIR\codex-plus-plus-manager.exe.new" "$INSTDIR\codex-plus-plus-manager.exe"

  ; 清理旧/新桌面与开始菜单入口（含历史乱码快捷方式），再只创建 Chimera 入口
  Delete "$DESKTOP\Codex++.lnk"
  Delete "$DESKTOP\Codex++ 管理工具.lnk"
  Delete "$DESKTOP\Codex++ 绠＄悊宸ュ叿.lnk"
  Delete "$DESKTOP\Chimera Codex.lnk"
  Delete "$DESKTOP\Chimera Codex 管理工具.lnk"
  Delete "$SMPROGRAMS\Codex++\Codex++.lnk"
  Delete "$SMPROGRAMS\Codex++\Codex++ 管理工具.lnk"
  Delete "$SMPROGRAMS\Codex++\Codex++ 绠＄悊宸ュ叿.lnk"
  Delete "$SMPROGRAMS\Codex++\卸载 Codex++.lnk"
  Delete "$SMPROGRAMS\Codex++\Chimera Codex.lnk"
  Delete "$SMPROGRAMS\Codex++\Chimera Codex 管理工具.lnk"
  Delete "$SMPROGRAMS\Codex++\卸载 Chimera Codex.lnk"
  Delete "$SMPROGRAMS\Chimera Codex\Chimera Codex.lnk"
  Delete "$SMPROGRAMS\Chimera Codex\Chimera Codex 管理工具.lnk"
  Delete "$SMPROGRAMS\Chimera Codex\卸载 Chimera Codex.lnk"
  RMDir "$SMPROGRAMS\Codex++"
  RMDir "$SMPROGRAMS\Chimera Codex"

  CreateShortcut "$DESKTOP\Chimera Codex.lnk" "$INSTDIR\codex-plus-plus.exe" "" "$INSTDIR\codex-plus-plus.exe"
  CreateShortcut "$DESKTOP\Chimera Codex 管理工具.lnk" "$INSTDIR\codex-plus-plus-manager.exe" "" "$INSTDIR\codex-plus-plus-manager.exe"
  CreateDirectory "$SMPROGRAMS\Chimera Codex"
  CreateShortcut "$SMPROGRAMS\Chimera Codex\Chimera Codex.lnk" "$INSTDIR\codex-plus-plus.exe" "" "$INSTDIR\codex-plus-plus.exe"
  CreateShortcut "$SMPROGRAMS\Chimera Codex\Chimera Codex 管理工具.lnk" "$INSTDIR\codex-plus-plus-manager.exe" "" "$INSTDIR\codex-plus-plus-manager.exe"
  CreateShortcut "$SMPROGRAMS\Chimera Codex\卸载 Chimera Codex.lnk" "$INSTDIR\uninstall.exe" "" "$INSTDIR\codex-plus-plus-manager.exe"

  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "Software\Codex++" "InstallDir" "$INSTDIR"
  ; 保留原版卸载 subkey，仅更新用户可见 DisplayName/Publisher
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex++" "DisplayName" "Chimera Codex"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex++" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex++" "Publisher" "ChimeraHub"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex++" "DisplayIcon" "$INSTDIR\codex-plus-plus-manager.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex++" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex++" "UninstallString" "$INSTDIR\uninstall.exe"
SectionEnd

Section "Uninstall"
  nsExec::ExecToLog 'taskkill /IM codex-plus-plus.exe /F'
  Pop $0
  nsExec::ExecToLog 'taskkill /IM codex-plus-plus-manager.exe /F'
  Pop $0

  Delete "$DESKTOP\Codex++.lnk"
  Delete "$DESKTOP\Codex++ 管理工具.lnk"
  Delete "$DESKTOP\Codex++ 绠＄悊宸ュ叿.lnk"
  Delete "$DESKTOP\Chimera Codex.lnk"
  Delete "$DESKTOP\Chimera Codex 管理工具.lnk"
  Delete "$SMPROGRAMS\Codex++\Codex++.lnk"
  Delete "$SMPROGRAMS\Codex++\Codex++ 管理工具.lnk"
  Delete "$SMPROGRAMS\Codex++\Codex++ 绠＄悊宸ュ叿.lnk"
  Delete "$SMPROGRAMS\Codex++\卸载 Codex++.lnk"
  Delete "$SMPROGRAMS\Codex++\Chimera Codex.lnk"
  Delete "$SMPROGRAMS\Codex++\Chimera Codex 管理工具.lnk"
  Delete "$SMPROGRAMS\Codex++\卸载 Chimera Codex.lnk"
  Delete "$SMPROGRAMS\Chimera Codex\Chimera Codex.lnk"
  Delete "$SMPROGRAMS\Chimera Codex\Chimera Codex 管理工具.lnk"
  Delete "$SMPROGRAMS\Chimera Codex\卸载 Chimera Codex.lnk"
  RMDir "$SMPROGRAMS\Codex++"
  RMDir "$SMPROGRAMS\Chimera Codex"

  Delete "$INSTDIR\codex-plus-plus.exe"
  Delete "$INSTDIR\codex-plus-plus-manager.exe"
  Delete "$INSTDIR\codex-plus-plus.exe.new"
  Delete "$INSTDIR\codex-plus-plus-manager.exe.new"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex++"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CodexPlusPlus"
  DeleteRegKey HKCU "Software\Codex++"
SectionEnd
