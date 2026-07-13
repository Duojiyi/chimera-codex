Unicode true
!include "MUI2.nsh"
!include "FileFunc.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!define ROOT "..\..\.."
!define CURRENT_UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\CodexPlusPlus"
!define LEGACY_UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex++"
!define SETUP_MUTEX_NAME "Local\ChimeraPlusPlus.Setup.Transaction"

Var SetupMutexHandle
Var UpdateContinuationToken
Var InstallStarted
Var UpdateRelaunchHandled

Name "Chimera++"
OutFile "${ROOT}\dist\windows\ChimeraPlusPlus-${VERSION}-windows-x64-setup.exe"
; 一期保持原版安装目录，支持覆盖升级
InstallDir "$LOCALAPPDATA\Programs\Codex++"
InstallDirRegKey HKCU "Software\Codex++" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!define MUI_ICON "${ROOT}\apps\codex-plus-manager\src-tauri\icons\icon.ico"
!define MUI_UNICON "${ROOT}\apps\codex-plus-manager\src-tauri\icons\icon.ico"

!macro BackupRegValue KEY VALUE SLOT
  ClearErrors
  ReadRegStr $8 HKCU "${KEY}" "${VALUE}"
  IfErrors backup_reg_${SLOT}_done
  ClearErrors
  WriteINIStr "$PLUGINSDIR\metadata-backup.ini" "Registry" "Has${SLOT}" "1"
  IfErrors install_metadata_backup_failed
  ClearErrors
  WriteINIStr "$PLUGINSDIR\metadata-backup.ini" "Registry" "${SLOT}" "$8"
  IfErrors install_metadata_backup_failed
backup_reg_${SLOT}_done:
!macroend

!macro RestoreRegValue KEY VALUE SLOT
  ReadINIStr $8 "$PLUGINSDIR\metadata-backup.ini" "Registry" "Has${SLOT}"
  StrCmp $8 "1" restore_reg_${SLOT}_existing restore_reg_${SLOT}_absent
restore_reg_${SLOT}_absent:
  DeleteRegValue HKCU "${KEY}" "${VALUE}"
  ClearErrors
  ReadRegStr $9 HKCU "${KEY}" "${VALUE}"
  IfErrors restore_reg_${SLOT}_done
  Goto restore_reg_${SLOT}_failed
restore_reg_${SLOT}_existing:
  ReadINIStr $8 "$PLUGINSDIR\metadata-backup.ini" "Registry" "${SLOT}"
  ClearErrors
  WriteRegStr HKCU "${KEY}" "${VALUE}" "$8"
  IfErrors restore_reg_${SLOT}_failed
  ClearErrors
  ReadRegStr $9 HKCU "${KEY}" "${VALUE}"
  IfErrors restore_reg_${SLOT}_failed
  StrCmp $9 $8 restore_reg_${SLOT}_done restore_reg_${SLOT}_failed
restore_reg_${SLOT}_failed:
  StrCpy $R9 "1"
restore_reg_${SLOT}_done:
!macroend

!macro BackupShortcut PATH SLOT
  IfFileExists "${PATH}" 0 backup_shortcut_${SLOT}_done
  ClearErrors
  CreateDirectory "$PLUGINSDIR\metadata-backup\${SLOT}"
  IfErrors install_metadata_backup_failed
  ClearErrors
  CopyFiles /SILENT "${PATH}" "$PLUGINSDIR\metadata-backup\${SLOT}"
  IfErrors install_metadata_backup_failed
backup_shortcut_${SLOT}_done:
!macroend

!macro RollbackShortcut PATH DESTDIR SLOT
  ClearErrors
  Delete "${PATH}"
  IfErrors 0 rollback_shortcut_${SLOT}_restore
  StrCpy $R9 "1"
rollback_shortcut_${SLOT}_restore:
  IfFileExists "$PLUGINSDIR\metadata-backup\${SLOT}\*" 0 rollback_shortcut_${SLOT}_done
  ClearErrors
  CopyFiles /SILENT "$PLUGINSDIR\metadata-backup\${SLOT}\*" "${DESTDIR}"
  IfErrors 0 rollback_shortcut_${SLOT}_done
  StrCpy $R9 "1"
rollback_shortcut_${SLOT}_done:
!macroend

!macro UninstallShortcut PATH SLOT
  IfFileExists "${PATH}" 0 uninstall_shortcut_${SLOT}_done
  ClearErrors
  Delete "${PATH}"
  IfErrors uninstall_metadata_failed
uninstall_shortcut_${SLOT}_done:
!macroend

!macro UninstallFile PATH SLOT
  IfFileExists "${PATH}" 0 uninstall_file_${SLOT}_done
  ClearErrors
  Delete "${PATH}"
  IfErrors uninstall_failed
uninstall_file_${SLOT}_done:
!macroend

Function .onInit
  StrCpy $InstallStarted "0"
  StrCpy $UpdateRelaunchHandled "0"
  ${GetParameters} $R8
  ${GetOptions} $R8 "/CONTINUATION_TOKEN=" $UpdateContinuationToken
  StrCmp $UpdateContinuationToken "" continuation_token_valid
  StrLen $R8 $UpdateContinuationToken
  IntCmp $R8 36 continuation_token_scan continuation_token_invalid continuation_token_invalid
continuation_token_scan:
  StrCpy $R8 0
continuation_token_scan_next:
  StrCpy $R9 $UpdateContinuationToken 1 $R8
  StrCmp $R9 "" continuation_token_valid
  StrCmp $R9 "-" continuation_token_scan_advance
  StrCmp $R9 "0" continuation_token_scan_advance
  StrCmp $R9 "1" continuation_token_scan_advance
  StrCmp $R9 "2" continuation_token_scan_advance
  StrCmp $R9 "3" continuation_token_scan_advance
  StrCmp $R9 "4" continuation_token_scan_advance
  StrCmp $R9 "5" continuation_token_scan_advance
  StrCmp $R9 "6" continuation_token_scan_advance
  StrCmp $R9 "7" continuation_token_scan_advance
  StrCmp $R9 "8" continuation_token_scan_advance
  StrCmp $R9 "9" continuation_token_scan_advance
  StrCmp $R9 "a" continuation_token_scan_advance
  StrCmp $R9 "b" continuation_token_scan_advance
  StrCmp $R9 "c" continuation_token_scan_advance
  StrCmp $R9 "d" continuation_token_scan_advance
  StrCmp $R9 "e" continuation_token_scan_advance
  StrCmp $R9 "f" continuation_token_scan_advance
  Goto continuation_token_invalid
continuation_token_scan_advance:
  IntOp $R8 $R8 + 1
  Goto continuation_token_scan_next
continuation_token_invalid:
  MessageBox MB_OK|MB_ICONSTOP "更新恢复令牌无效，安装已停止。"
  Abort
continuation_token_valid:
  ; 固定 .new/.bak 文件是共享事务状态，同一会话只允许一个 setup 使用。
  System::Call 'kernel32::CreateMutexW(p 0, i 1, w "${SETUP_MUTEX_NAME}") p .r0 ?e'
  Pop $1
  StrCpy $SetupMutexHandle $0
  StrCmp $SetupMutexHandle 0 setup_mutex_failed
  StrCmp $1 183 setup_mutex_already_running
  Return

setup_mutex_already_running:
  System::Call 'kernel32::CloseHandle(p $SetupMutexHandle)'
  StrCpy $SetupMutexHandle 0
  MessageBox MB_OK|MB_ICONEXCLAMATION "另一个 Chimera++ 安装程序正在运行。请等待其完成后重试。"
  Abort

setup_mutex_failed:
  MessageBox MB_OK|MB_ICONSTOP "无法创建安装互斥锁（错误 $1），安装已停止。"
  Abort
FunctionEnd

Function .onInstFailed
  StrCmp $InstallStarted "1" 0 update_failure_done
  IfFileExists "$INSTDIR\codex-plus-plus.exe" 0 update_failure_done
  StrCmp $UpdateContinuationToken "" update_failure_done
  Exec '"$INSTDIR\codex-plus-plus.exe" --update-continuation-token "$UpdateContinuationToken"'
  StrCpy $UpdateRelaunchHandled "1"
update_failure_done:
  SetErrorLevel 1
FunctionEnd

Function .onInstSuccess
  StrCmp $InstallStarted "1" 0 update_success_done
  IfFileExists "$INSTDIR\codex-plus-plus.exe" 0 update_success_done
  Exec '"$INSTDIR\codex-plus-plus.exe"'
  StrCpy $UpdateRelaunchHandled "1"
update_success_done:
  SetErrorLevel 0
FunctionEnd

Function .onGUIEnd
  StrCmp $UpdateRelaunchHandled "1" update_gui_end_done
  StrCmp $UpdateContinuationToken "" update_gui_end_done
  IfFileExists "$INSTDIR\codex-plus-plus.exe" 0 update_gui_end_done
  Exec '"$INSTDIR\codex-plus-plus.exe" --update-continuation-token "$UpdateContinuationToken"'
  StrCpy $UpdateRelaunchHandled "1"
update_gui_end_done:
FunctionEnd

Function un.onInit
  ; 卸载与安装共享同一事务锁，避免删除对方的 .new/.bak 恢复文件。
  System::Call 'kernel32::CreateMutexW(p 0, i 1, w "${SETUP_MUTEX_NAME}") p .r0 ?e'
  Pop $1
  StrCpy $SetupMutexHandle $0
  StrCmp $SetupMutexHandle 0 uninstall_mutex_failed
  StrCmp $1 183 uninstall_mutex_already_running
  Return

uninstall_mutex_already_running:
  System::Call 'kernel32::CloseHandle(p $SetupMutexHandle)'
  StrCpy $SetupMutexHandle 0
  MessageBox MB_OK|MB_ICONEXCLAMATION "另一个 Chimera++ 安装或卸载程序正在运行。请等待其完成后重试。"
  Abort

uninstall_mutex_failed:
  MessageBox MB_OK|MB_ICONSTOP "无法创建安装事务互斥锁（错误 $1），卸载已停止。"
  Abort
FunctionEnd

!macro UninstallRegKey KEY SLOT
  ClearErrors
  DeleteRegKey /ifempty HKCU "${KEY}"
  ClearErrors
!macroend

!macro BackupUninstallShortcut PATH SLOT
  IfFileExists "${PATH}" 0 backup_uninstall_shortcut_${SLOT}_done
  ClearErrors
  CreateDirectory "$PLUGINSDIR\metadata-backup\${SLOT}"
  IfErrors uninstall_metadata_backup_failed
  ClearErrors
  CopyFiles /SILENT "${PATH}" "$PLUGINSDIR\metadata-backup\${SLOT}"
  IfErrors uninstall_metadata_backup_failed
backup_uninstall_shortcut_${SLOT}_done:
!macroend

!macro BackupUninstallRegValue KEY VALUE SLOT
  ClearErrors
  ReadRegStr $8 HKCU "${KEY}" "${VALUE}"
  IfErrors backup_uninstall_reg_${SLOT}_done
  ClearErrors
  WriteINIStr "$PLUGINSDIR\metadata-backup.ini" "Registry" "Has${SLOT}" "1"
  IfErrors uninstall_metadata_backup_failed
  ClearErrors
  WriteINIStr "$PLUGINSDIR\metadata-backup.ini" "Registry" "${SLOT}" "$8"
  IfErrors uninstall_metadata_backup_failed
backup_uninstall_reg_${SLOT}_done:
!macroend

!macro UninstallBackedRegValue KEY VALUE SLOT
  ReadINIStr $8 "$PLUGINSDIR\metadata-backup.ini" "Registry" "Has${SLOT}"
  StrCmp $8 "1" 0 uninstall_reg_value_${SLOT}_done
  ClearErrors
  DeleteRegValue HKCU "${KEY}" "${VALUE}"
  IfErrors uninstall_metadata_failed
uninstall_reg_value_${SLOT}_done:
!macroend

!macro DeleteInstallShortcut PATH SLOT
  IfFileExists "${PATH}" 0 install_shortcut_${SLOT}_done
  ClearErrors
  Delete "${PATH}"
  IfErrors install_metadata_failed
install_shortcut_${SLOT}_done:
!macroend

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  StrCpy $InstallStarted "1"
  SetOutPath "$INSTDIR"

  nsExec::ExecToLog 'taskkill /IM codex-plus-plus.exe /F'
  Pop $0
  nsExec::ExecToLog 'taskkill /IM codex-plus-plus-manager.exe /F'
  Pop $0

  ; 程序与法律文件先完整 staging，再备份旧文件并提交。
  StrCpy $1 "0"
  StrCpy $2 "0"
  StrCpy $3 "0"
  StrCpy $4 "0"
  StrCpy $5 "0"
  StrCpy $6 "0"
  StrCpy $7 "0"
  StrCpy $R0 "0"
  StrCpy $R1 "0"
  StrCpy $R2 "0"
  StrCpy $R3 "0"
  StrCpy $R4 "0"
  StrCpy $R5 "0"
  StrCpy $R9 "0"

  Delete "$INSTDIR\codex-plus-plus.exe.new"
  Delete "$INSTDIR\codex-plus-plus-manager.exe.new"
  Delete "$INSTDIR\uninstall.exe.new"
  Delete "$INSTDIR\LICENSE.new"
  Delete "$INSTDIR\NOTICE.new"
  Delete "$INSTDIR\SOURCE_CODE.txt.new"
  ClearErrors
  File "/oname=codex-plus-plus.exe.new" "${ROOT}\dist\windows\app\codex-plus-plus.exe"
  IfErrors install_rollback
  ClearErrors
  File "/oname=codex-plus-plus-manager.exe.new" "${ROOT}\dist\windows\app\codex-plus-plus-manager.exe"
  IfErrors install_rollback
  ClearErrors
  WriteUninstaller "$INSTDIR\uninstall.exe.new"
  IfErrors install_rollback
  ClearErrors
  File "/oname=LICENSE.new" "${ROOT}\LICENSE"
  IfErrors install_rollback
  ClearErrors
  File "/oname=NOTICE.new" "${ROOT}\NOTICE"
  IfErrors install_rollback
  ClearErrors
  File "/oname=SOURCE_CODE.txt.new" "${ROOT}\SOURCE_CODE.txt"
  IfErrors install_rollback

  ; 上次失败留下的备份需要人工处理，禁止覆盖最后恢复副本。
  IfFileExists "$INSTDIR\codex-plus-plus.exe.bak" install_stale_backup 0
  IfFileExists "$INSTDIR\codex-plus-plus-manager.exe.bak" install_stale_backup 0
  IfFileExists "$INSTDIR\uninstall.exe.bak" install_stale_backup 0
  IfFileExists "$INSTDIR\LICENSE.bak" install_stale_backup 0
  IfFileExists "$INSTDIR\NOTICE.bak" install_stale_backup 0
  IfFileExists "$INSTDIR\SOURCE_CODE.txt.bak" install_stale_backup 0

  IfFileExists "$INSTDIR\codex-plus-plus.exe" 0 silent_backup_done
  ClearErrors
  Rename "$INSTDIR\codex-plus-plus.exe" "$INSTDIR\codex-plus-plus.exe.bak"
  IfErrors install_rollback
  StrCpy $1 "1"
silent_backup_done:

  IfFileExists "$INSTDIR\codex-plus-plus-manager.exe" 0 manager_backup_done
  ClearErrors
  Rename "$INSTDIR\codex-plus-plus-manager.exe" "$INSTDIR\codex-plus-plus-manager.exe.bak"
  IfErrors install_rollback
  StrCpy $2 "1"
manager_backup_done:

  IfFileExists "$INSTDIR\uninstall.exe" 0 uninstaller_backup_done
  ClearErrors
  Rename "$INSTDIR\uninstall.exe" "$INSTDIR\uninstall.exe.bak"
  IfErrors install_rollback
  StrCpy $5 "1"
uninstaller_backup_done:

  IfFileExists "$INSTDIR\LICENSE" 0 license_backup_done
  ClearErrors
  Rename "$INSTDIR\LICENSE" "$INSTDIR\LICENSE.bak"
  IfErrors install_rollback
  StrCpy $R0 "1"
license_backup_done:

  IfFileExists "$INSTDIR\NOTICE" 0 notice_backup_done
  ClearErrors
  Rename "$INSTDIR\NOTICE" "$INSTDIR\NOTICE.bak"
  IfErrors install_rollback
  StrCpy $R1 "1"
notice_backup_done:

  IfFileExists "$INSTDIR\SOURCE_CODE.txt" 0 source_code_backup_done
  ClearErrors
  Rename "$INSTDIR\SOURCE_CODE.txt" "$INSTDIR\SOURCE_CODE.txt.bak"
  IfErrors install_rollback
  StrCpy $R2 "1"
source_code_backup_done:

  ClearErrors
  Rename "$INSTDIR\codex-plus-plus.exe.new" "$INSTDIR\codex-plus-plus.exe"
  IfErrors install_rollback
  StrCpy $3 "1"
  ClearErrors
  Rename "$INSTDIR\codex-plus-plus-manager.exe.new" "$INSTDIR\codex-plus-plus-manager.exe"
  IfErrors install_rollback
  StrCpy $4 "1"
  ClearErrors
  Rename "$INSTDIR\uninstall.exe.new" "$INSTDIR\uninstall.exe"
  IfErrors install_rollback
  StrCpy $6 "1"
  ClearErrors
  Rename "$INSTDIR\LICENSE.new" "$INSTDIR\LICENSE"
  IfErrors install_rollback
  StrCpy $R3 "1"
  ClearErrors
  Rename "$INSTDIR\NOTICE.new" "$INSTDIR\NOTICE"
  IfErrors install_rollback
  StrCpy $R4 "1"
  ClearErrors
  Rename "$INSTDIR\SOURCE_CODE.txt.new" "$INSTDIR\SOURCE_CODE.txt"
  IfErrors install_rollback
  StrCpy $R5 "1"

  ; 在改动快捷方式和注册表前保存原元数据，供任何部分失败时恢复。
  InitPluginsDir
  ClearErrors
  CreateDirectory "$PLUGINSDIR\metadata-backup"
  IfErrors install_metadata_backup_failed
  IfFileExists "$SMPROGRAMS\Chimera++\*" metadata_startmenu_existed metadata_startmenu_snapshot_done
metadata_startmenu_existed:
  ClearErrors
  WriteINIStr "$PLUGINSDIR\metadata-backup.ini" "Paths" "StartMenuExisted" "1"
  IfErrors install_metadata_backup_failed
metadata_startmenu_snapshot_done:
  !insertmacro BackupShortcut "$DESKTOP\Chimera++.lnk" "DesktopSilent"
  !insertmacro BackupShortcut "$DESKTOP\Chimera++ 管理工具.lnk" "DesktopManager"
  !insertmacro BackupShortcut "$SMPROGRAMS\Chimera++\Chimera++.lnk" "MenuSilent"
  !insertmacro BackupShortcut "$SMPROGRAMS\Chimera++\Chimera++ 管理工具.lnk" "MenuManager"
  !insertmacro BackupShortcut "$SMPROGRAMS\Chimera++\卸载 Chimera++.lnk" "MenuUninstall"
  !insertmacro BackupShortcut "$DESKTOP\Codex++.lnk" "DesktopLegacySilent"
  !insertmacro BackupShortcut "$DESKTOP\Codex++ 管理工具.lnk" "DesktopLegacyManager"
  !insertmacro BackupShortcut "$DESKTOP\Codex++ 绠＄悊宸ュ叿.lnk" "DesktopMojibakeManager"
  !insertmacro BackupShortcut "$SMPROGRAMS\Codex++\Codex++.lnk" "MenuLegacySilent"
  !insertmacro BackupShortcut "$SMPROGRAMS\Codex++\Codex++ 管理工具.lnk" "MenuLegacyManager"
  !insertmacro BackupShortcut "$SMPROGRAMS\Codex++\Codex++ 绠＄悊宸ュ叿.lnk" "MenuMojibakeManager"
  !insertmacro BackupShortcut "$SMPROGRAMS\Codex++\卸载 Codex++.lnk" "MenuLegacyUninstall"
  !insertmacro BackupShortcut "$SMPROGRAMS\Codex++\Chimera++.lnk" "MenuCompatSilent"
  !insertmacro BackupShortcut "$SMPROGRAMS\Codex++\Chimera++ 管理工具.lnk" "MenuCompatManager"
  !insertmacro BackupShortcut "$SMPROGRAMS\Codex++\卸载 Chimera++.lnk" "MenuCompatUninstall"
  !insertmacro BackupRegValue "Software\Codex++" "InstallDir" "InstallDir"
  !insertmacro BackupRegValue "${CURRENT_UNINSTALL_KEY}" "DisplayName" "CurrentDisplayName"
  !insertmacro BackupRegValue "${CURRENT_UNINSTALL_KEY}" "DisplayVersion" "CurrentDisplayVersion"
  !insertmacro BackupRegValue "${CURRENT_UNINSTALL_KEY}" "Publisher" "CurrentPublisher"
  !insertmacro BackupRegValue "${CURRENT_UNINSTALL_KEY}" "DisplayIcon" "CurrentDisplayIcon"
  !insertmacro BackupRegValue "${CURRENT_UNINSTALL_KEY}" "InstallLocation" "CurrentInstallLocation"
  !insertmacro BackupRegValue "${CURRENT_UNINSTALL_KEY}" "UninstallString" "CurrentUninstallString"
  !insertmacro BackupRegValue "${CURRENT_UNINSTALL_KEY}" "QuietUninstallString" "CurrentQuietUninstallString"
  Goto metadata_backup_complete

install_metadata_backup_failed:
  Goto install_rollback

metadata_backup_complete:

  ; 新入口和注册表全部成功后，才清理 legacy 入口和备份。
  ClearErrors
  CreateShortcut "$DESKTOP\Chimera++.lnk" "$INSTDIR\codex-plus-plus-manager.exe" "" "$INSTDIR\codex-plus-plus-manager.exe"
  IfErrors install_metadata_failed
  !insertmacro DeleteInstallShortcut "$DESKTOP\Chimera++ 管理工具.lnk" "DesktopManager"
  ClearErrors
  CreateDirectory "$SMPROGRAMS\Chimera++"
  IfErrors install_metadata_failed
  ClearErrors
  CreateShortcut "$SMPROGRAMS\Chimera++\Chimera++.lnk" "$INSTDIR\codex-plus-plus.exe" "" "$INSTDIR\codex-plus-plus.exe"
  IfErrors install_metadata_failed
  ClearErrors
  CreateShortcut "$SMPROGRAMS\Chimera++\Chimera++ 管理工具.lnk" "$INSTDIR\codex-plus-plus-manager.exe" "" "$INSTDIR\codex-plus-plus-manager.exe"
  IfErrors install_metadata_failed
  ClearErrors
  CreateShortcut "$SMPROGRAMS\Chimera++\卸载 Chimera++.lnk" "$INSTDIR\uninstall.exe" "" "$INSTDIR\codex-plus-plus-manager.exe"
  IfErrors install_metadata_failed

  ClearErrors
  WriteRegStr HKCU "Software\Codex++" "InstallDir" "$INSTDIR"
  IfErrors install_metadata_failed
  ClearErrors
  WriteRegStr HKCU "${CURRENT_UNINSTALL_KEY}" "DisplayName" "Chimera++"
  IfErrors install_metadata_failed
  ClearErrors
  WriteRegStr HKCU "${CURRENT_UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  IfErrors install_metadata_failed
  ClearErrors
  WriteRegStr HKCU "${CURRENT_UNINSTALL_KEY}" "Publisher" "ChimeraHub"
  IfErrors install_metadata_failed
  ClearErrors
  WriteRegStr HKCU "${CURRENT_UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\codex-plus-plus-manager.exe"
  IfErrors install_metadata_failed
  ClearErrors
  WriteRegStr HKCU "${CURRENT_UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  IfErrors install_metadata_failed
  ClearErrors
  WriteRegStr HKCU "${CURRENT_UNINSTALL_KEY}" "UninstallString" '$\"$INSTDIR\uninstall.exe$\"'
  IfErrors install_metadata_failed
  ClearErrors
  WriteRegStr HKCU "${CURRENT_UNINSTALL_KEY}" "QuietUninstallString" '$\"$INSTDIR\uninstall.exe$\" /S'
  IfErrors install_metadata_failed
  !insertmacro DeleteInstallShortcut "$DESKTOP\Codex++.lnk" "DesktopLegacySilent"
  !insertmacro DeleteInstallShortcut "$DESKTOP\Codex++ 管理工具.lnk" "DesktopLegacyManager"
  !insertmacro DeleteInstallShortcut "$DESKTOP\Codex++ 绠＄悊宸ュ叿.lnk" "DesktopMojibakeManager"
  !insertmacro DeleteInstallShortcut "$SMPROGRAMS\Codex++\Codex++.lnk" "MenuLegacySilent"
  !insertmacro DeleteInstallShortcut "$SMPROGRAMS\Codex++\Codex++ 管理工具.lnk" "MenuLegacyManager"
  !insertmacro DeleteInstallShortcut "$SMPROGRAMS\Codex++\Codex++ 绠＄悊宸ュ叿.lnk" "MenuMojibakeManager"
  !insertmacro DeleteInstallShortcut "$SMPROGRAMS\Codex++\卸载 Codex++.lnk" "MenuLegacyUninstall"
  !insertmacro DeleteInstallShortcut "$SMPROGRAMS\Codex++\Chimera++.lnk" "MenuCompatSilent"
  !insertmacro DeleteInstallShortcut "$SMPROGRAMS\Codex++\Chimera++ 管理工具.lnk" "MenuCompatManager"
  !insertmacro DeleteInstallShortcut "$SMPROGRAMS\Codex++\卸载 Chimera++.lnk" "MenuCompatUninstall"
  RMDir "$SMPROGRAMS\Codex++"

  IfFileExists "$INSTDIR\codex-plus-plus.exe.bak" 0 cleanup_silent_backup_done
  ClearErrors
  Delete "$INSTDIR\codex-plus-plus.exe.bak"
  IfErrors install_backup_cleanup_failed
cleanup_silent_backup_done:
  IfFileExists "$INSTDIR\codex-plus-plus-manager.exe.bak" 0 cleanup_manager_backup_done
  ClearErrors
  Delete "$INSTDIR\codex-plus-plus-manager.exe.bak"
  IfErrors install_backup_cleanup_failed
cleanup_manager_backup_done:
  IfFileExists "$INSTDIR\uninstall.exe.bak" 0 cleanup_uninstaller_backup_done
  ClearErrors
  Delete "$INSTDIR\uninstall.exe.bak"
  IfErrors install_backup_cleanup_failed
cleanup_uninstaller_backup_done:
  IfFileExists "$INSTDIR\LICENSE.bak" 0 cleanup_license_backup_done
  ClearErrors
  Delete "$INSTDIR\LICENSE.bak"
  IfErrors install_backup_cleanup_failed
cleanup_license_backup_done:
  IfFileExists "$INSTDIR\NOTICE.bak" 0 cleanup_notice_backup_done
  ClearErrors
  Delete "$INSTDIR\NOTICE.bak"
  IfErrors install_backup_cleanup_failed
cleanup_notice_backup_done:
  IfFileExists "$INSTDIR\SOURCE_CODE.txt.bak" 0 cleanup_source_code_backup_done
  ClearErrors
  Delete "$INSTDIR\SOURCE_CODE.txt.bak"
  IfErrors install_backup_cleanup_failed
cleanup_source_code_backup_done:
  ; legacy ARP 整键可能包含未知值/类型/子键。仅在所有可回滚步骤完成后原子删除，
  ; 删除成功后不再执行任何可失败的元数据操作，避免伪造不完整的值级恢复。
  StrCpy $0 0
install_legacy_cleanup_probe:
  ClearErrors
  EnumRegKey $1 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall" $0
  IfErrors install_complete
  StrCmp $1 "" install_complete
  StrCmp $1 "Codex++" install_legacy_cleanup_found
  IntOp $0 $0 + 1
  Goto install_legacy_cleanup_probe

install_legacy_cleanup_found:
  ClearErrors
  DeleteRegKey HKCU "${LEGACY_UNINSTALL_KEY}"
  IfErrors install_legacy_cleanup_failed
  Goto install_complete

install_legacy_cleanup_failed:
  Abort "Chimera++ 已更新，但旧版控制面板注册项清理失败。请关闭占用程序后重试安装。"

install_metadata_failed:
  Goto install_metadata_rollback

install_metadata_rollback:
  !insertmacro RollbackShortcut "$DESKTOP\Chimera++.lnk" "$DESKTOP" "DesktopSilent"
  !insertmacro RollbackShortcut "$DESKTOP\Chimera++ 管理工具.lnk" "$DESKTOP" "DesktopManager"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Chimera++\Chimera++.lnk" "$SMPROGRAMS\Chimera++" "MenuSilent"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Chimera++\Chimera++ 管理工具.lnk" "$SMPROGRAMS\Chimera++" "MenuManager"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Chimera++\卸载 Chimera++.lnk" "$SMPROGRAMS\Chimera++" "MenuUninstall"
  !insertmacro RollbackShortcut "$DESKTOP\Codex++.lnk" "$DESKTOP" "DesktopLegacySilent"
  !insertmacro RollbackShortcut "$DESKTOP\Codex++ 管理工具.lnk" "$DESKTOP" "DesktopLegacyManager"
  !insertmacro RollbackShortcut "$DESKTOP\Codex++ 绠＄悊宸ュ叿.lnk" "$DESKTOP" "DesktopMojibakeManager"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Codex++\Codex++.lnk" "$SMPROGRAMS\Codex++" "MenuLegacySilent"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Codex++\Codex++ 管理工具.lnk" "$SMPROGRAMS\Codex++" "MenuLegacyManager"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Codex++\Codex++ 绠＄悊宸ュ叿.lnk" "$SMPROGRAMS\Codex++" "MenuMojibakeManager"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Codex++\卸载 Codex++.lnk" "$SMPROGRAMS\Codex++" "MenuLegacyUninstall"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Codex++\Chimera++.lnk" "$SMPROGRAMS\Codex++" "MenuCompatSilent"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Codex++\Chimera++ 管理工具.lnk" "$SMPROGRAMS\Codex++" "MenuCompatManager"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Codex++\卸载 Chimera++.lnk" "$SMPROGRAMS\Codex++" "MenuCompatUninstall"
  ReadINIStr $8 "$PLUGINSDIR\metadata-backup.ini" "Paths" "StartMenuExisted"
  StrCmp $8 "1" metadata_startmenu_restored
  ClearErrors
  RMDir "$SMPROGRAMS\Chimera++"
  IfErrors 0 metadata_startmenu_restored
  StrCpy $R9 "1"
metadata_startmenu_restored:
  !insertmacro RestoreRegValue "Software\Codex++" "InstallDir" "InstallDir"
  !insertmacro RestoreRegValue "${CURRENT_UNINSTALL_KEY}" "DisplayName" "CurrentDisplayName"
  !insertmacro RestoreRegValue "${CURRENT_UNINSTALL_KEY}" "DisplayVersion" "CurrentDisplayVersion"
  !insertmacro RestoreRegValue "${CURRENT_UNINSTALL_KEY}" "Publisher" "CurrentPublisher"
  !insertmacro RestoreRegValue "${CURRENT_UNINSTALL_KEY}" "DisplayIcon" "CurrentDisplayIcon"
  !insertmacro RestoreRegValue "${CURRENT_UNINSTALL_KEY}" "InstallLocation" "CurrentInstallLocation"
  !insertmacro RestoreRegValue "${CURRENT_UNINSTALL_KEY}" "UninstallString" "CurrentUninstallString"
  !insertmacro RestoreRegValue "${CURRENT_UNINSTALL_KEY}" "QuietUninstallString" "CurrentQuietUninstallString"
  Goto install_rollback

install_stale_backup:
  Delete "$INSTDIR\codex-plus-plus.exe.new"
  Delete "$INSTDIR\codex-plus-plus-manager.exe.new"
  Delete "$INSTDIR\uninstall.exe.new"
  Delete "$INSTDIR\LICENSE.new"
  Delete "$INSTDIR\NOTICE.new"
  Delete "$INSTDIR\SOURCE_CODE.txt.new"
  Abort "检测到上次失败留下的 .bak 文件。为保护旧版本，安装已停止，请先人工恢复或移走备份。"

install_rollback:
  StrCpy $7 "0"
  StrCmp $R5 "1" 0 rollback_source_code_backup
  ClearErrors
  Delete "$INSTDIR\SOURCE_CODE.txt"
  IfErrors 0 +2
  StrCpy $7 "1"
rollback_source_code_backup:
  StrCmp $R2 "1" 0 rollback_notice_new
  ClearErrors
  Rename "$INSTDIR\SOURCE_CODE.txt.bak" "$INSTDIR\SOURCE_CODE.txt"
  IfErrors 0 +2
  StrCpy $7 "1"
rollback_notice_new:
  StrCmp $R4 "1" 0 rollback_notice_backup
  ClearErrors
  Delete "$INSTDIR\NOTICE"
  IfErrors 0 +2
  StrCpy $7 "1"
rollback_notice_backup:
  StrCmp $R1 "1" 0 rollback_license_new
  ClearErrors
  Rename "$INSTDIR\NOTICE.bak" "$INSTDIR\NOTICE"
  IfErrors 0 +2
  StrCpy $7 "1"
rollback_license_new:
  StrCmp $R3 "1" 0 rollback_license_backup
  ClearErrors
  Delete "$INSTDIR\LICENSE"
  IfErrors 0 +2
  StrCpy $7 "1"
rollback_license_backup:
  StrCmp $R0 "1" 0 rollback_uninstaller_new
  ClearErrors
  Rename "$INSTDIR\LICENSE.bak" "$INSTDIR\LICENSE"
  IfErrors 0 +2
  StrCpy $7 "1"
rollback_uninstaller_new:
  StrCmp $6 "1" 0 rollback_uninstaller_backup
  ClearErrors
  Delete "$INSTDIR\uninstall.exe"
  IfErrors 0 +2
  StrCpy $7 "1"
rollback_uninstaller_backup:
  StrCmp $5 "1" 0 rollback_manager_new
  ClearErrors
  Rename "$INSTDIR\uninstall.exe.bak" "$INSTDIR\uninstall.exe"
  IfErrors 0 +2
  StrCpy $7 "1"
rollback_manager_new:
  StrCmp $4 "1" 0 rollback_manager_backup
  ClearErrors
  Delete "$INSTDIR\codex-plus-plus-manager.exe"
  IfErrors 0 +2
  StrCpy $7 "1"
rollback_manager_backup:
  StrCmp $2 "1" 0 rollback_silent_new
  ClearErrors
  Rename "$INSTDIR\codex-plus-plus-manager.exe.bak" "$INSTDIR\codex-plus-plus-manager.exe"
  IfErrors 0 +2
  StrCpy $7 "1"
rollback_silent_new:
  StrCmp $3 "1" 0 rollback_silent_backup
  ClearErrors
  Delete "$INSTDIR\codex-plus-plus.exe"
  IfErrors 0 +2
  StrCpy $7 "1"
rollback_silent_backup:
  StrCmp $1 "1" 0 rollback_staging
  ClearErrors
  Rename "$INSTDIR\codex-plus-plus.exe.bak" "$INSTDIR\codex-plus-plus.exe"
  IfErrors 0 +2
  StrCpy $7 "1"
rollback_staging:
  Delete "$INSTDIR\codex-plus-plus.exe.new"
  Delete "$INSTDIR\codex-plus-plus-manager.exe.new"
  Delete "$INSTDIR\uninstall.exe.new"
  Delete "$INSTDIR\LICENSE.new"
  Delete "$INSTDIR\NOTICE.new"
  Delete "$INSTDIR\SOURCE_CODE.txt.new"
  StrCmp $R9 "0" 0 rollback_failed
  StrCmp $7 "0" rollback_restored rollback_failed

rollback_restored:
  Abort "Chimera++ 安装失败，原有程序文件已恢复。"

rollback_failed:
  Abort "Chimera++ 安装失败，且自动回滚未完整成功。.bak 备份已保留，请人工恢复。"

install_backup_cleanup_failed:
  Abort "Chimera++ 已更新，但旧版 .bak 备份清理失败。请关闭占用进程并人工删除 .bak 后再升级。"

install_complete:
SectionEnd

Section "Uninstall"
  nsExec::ExecToLog 'taskkill /IM codex-plus-plus.exe /F'
  Pop $0
  nsExec::ExecToLog 'taskkill /IM codex-plus-plus-manager.exe /F'
  Pop $0

  InitPluginsDir
  ClearErrors
  CreateDirectory "$PLUGINSDIR\metadata-backup"
  IfErrors uninstall_metadata_backup_failed
  !insertmacro BackupUninstallShortcut "$DESKTOP\Codex++.lnk" "DesktopLegacySilent"
  !insertmacro BackupUninstallShortcut "$DESKTOP\Codex++ 管理工具.lnk" "DesktopLegacyManager"
  !insertmacro BackupUninstallShortcut "$DESKTOP\Codex++ 绠＄悊宸ュ叿.lnk" "DesktopMojibakeManager"
  !insertmacro BackupUninstallShortcut "$DESKTOP\Chimera++.lnk" "DesktopChimeraSilent"
  !insertmacro BackupUninstallShortcut "$DESKTOP\Chimera++ 管理工具.lnk" "DesktopChimeraManager"
  !insertmacro BackupUninstallShortcut "$SMPROGRAMS\Codex++\Codex++.lnk" "MenuLegacySilent"
  !insertmacro BackupUninstallShortcut "$SMPROGRAMS\Codex++\Codex++ 管理工具.lnk" "MenuLegacyManager"
  !insertmacro BackupUninstallShortcut "$SMPROGRAMS\Codex++\Codex++ 绠＄悊宸ュ叿.lnk" "MenuMojibakeManager"
  !insertmacro BackupUninstallShortcut "$SMPROGRAMS\Codex++\卸载 Codex++.lnk" "MenuLegacyUninstall"
  !insertmacro BackupUninstallShortcut "$SMPROGRAMS\Codex++\Chimera++.lnk" "MenuCompatSilent"
  !insertmacro BackupUninstallShortcut "$SMPROGRAMS\Codex++\Chimera++ 管理工具.lnk" "MenuCompatManager"
  !insertmacro BackupUninstallShortcut "$SMPROGRAMS\Codex++\卸载 Chimera++.lnk" "MenuCompatUninstall"
  !insertmacro BackupUninstallShortcut "$SMPROGRAMS\Chimera++\Chimera++.lnk" "MenuChimeraSilent"
  !insertmacro BackupUninstallShortcut "$SMPROGRAMS\Chimera++\Chimera++ 管理工具.lnk" "MenuChimeraManager"
  !insertmacro BackupUninstallShortcut "$SMPROGRAMS\Chimera++\卸载 Chimera++.lnk" "MenuChimeraUninstall"
  !insertmacro BackupUninstallRegValue "Software\Codex++" "InstallDir" "InstallDir"
  !insertmacro BackupUninstallRegValue "${CURRENT_UNINSTALL_KEY}" "DisplayName" "CurrentDisplayName"
  !insertmacro BackupUninstallRegValue "${CURRENT_UNINSTALL_KEY}" "DisplayVersion" "CurrentDisplayVersion"
  !insertmacro BackupUninstallRegValue "${CURRENT_UNINSTALL_KEY}" "Publisher" "CurrentPublisher"
  !insertmacro BackupUninstallRegValue "${CURRENT_UNINSTALL_KEY}" "DisplayIcon" "CurrentDisplayIcon"
  !insertmacro BackupUninstallRegValue "${CURRENT_UNINSTALL_KEY}" "InstallLocation" "CurrentInstallLocation"
  !insertmacro BackupUninstallRegValue "${CURRENT_UNINSTALL_KEY}" "UninstallString" "CurrentUninstallString"
  !insertmacro BackupUninstallRegValue "${CURRENT_UNINSTALL_KEY}" "QuietUninstallString" "CurrentQuietUninstallString"
  !insertmacro BackupUninstallRegValue "${LEGACY_UNINSTALL_KEY}" "DisplayName" "LegacyDisplayName"
  !insertmacro BackupUninstallRegValue "${LEGACY_UNINSTALL_KEY}" "DisplayVersion" "LegacyDisplayVersion"
  !insertmacro BackupUninstallRegValue "${LEGACY_UNINSTALL_KEY}" "Publisher" "LegacyPublisher"
  !insertmacro BackupUninstallRegValue "${LEGACY_UNINSTALL_KEY}" "DisplayIcon" "LegacyDisplayIcon"
  !insertmacro BackupUninstallRegValue "${LEGACY_UNINSTALL_KEY}" "InstallLocation" "LegacyInstallLocation"
  !insertmacro BackupUninstallRegValue "${LEGACY_UNINSTALL_KEY}" "UninstallString" "LegacyUninstallString"
  !insertmacro BackupUninstallRegValue "${LEGACY_UNINSTALL_KEY}" "QuietUninstallString" "LegacyQuietUninstallString"
  !insertmacro BackupUninstallRegValue "${URL_PROTOCOL_SUBKEY}" "" "UrlDisplayName"
  !insertmacro BackupUninstallRegValue "${URL_PROTOCOL_SUBKEY}" "URL Protocol" "UrlProtocol"
  !insertmacro BackupUninstallRegValue "${URL_PROTOCOL_SUBKEY}\shell\open\command" "" "UrlCommand"

  ; 程序文件无法删除时保留快捷方式和卸载注册表，便于用户重试/修复。
  ClearErrors
  Delete "$INSTDIR\codex-plus-plus.exe"
  IfErrors uninstall_failed
  ClearErrors
  Delete "$INSTDIR\codex-plus-plus-manager.exe"
  IfErrors uninstall_failed
  ClearErrors
  Delete "$INSTDIR\LICENSE"
  IfErrors uninstall_failed
  ClearErrors
  Delete "$INSTDIR\NOTICE"
  IfErrors uninstall_failed
  ClearErrors
  Delete "$INSTDIR\SOURCE_CODE.txt"
  IfErrors uninstall_failed
  !insertmacro UninstallFile "$INSTDIR\LICENSE.new" "LicenseNew"
  !insertmacro UninstallFile "$INSTDIR\NOTICE.new" "NoticeNew"
  !insertmacro UninstallFile "$INSTDIR\SOURCE_CODE.txt.new" "SourceCodeNew"
  !insertmacro UninstallFile "$INSTDIR\codex-plus-plus.exe.new" "SilentNew"
  !insertmacro UninstallFile "$INSTDIR\codex-plus-plus-manager.exe.new" "ManagerNew"
  !insertmacro UninstallFile "$INSTDIR\uninstall.exe.new" "UninstallerNew"
  !insertmacro UninstallFile "$INSTDIR\codex-plus-plus.exe.bak" "SilentBackup"
  !insertmacro UninstallFile "$INSTDIR\codex-plus-plus-manager.exe.bak" "ManagerBackup"
  !insertmacro UninstallFile "$INSTDIR\uninstall.exe.bak" "UninstallerBackup"
  !insertmacro UninstallFile "$INSTDIR\LICENSE.bak" "LicenseBackup"
  !insertmacro UninstallFile "$INSTDIR\NOTICE.bak" "NoticeBackup"
  !insertmacro UninstallFile "$INSTDIR\SOURCE_CODE.txt.bak" "SourceCodeBackup"
  ; 控制面板和快捷方式清理成功前保留卸载器本身，失败时仍可重试。
  !insertmacro UninstallShortcut "$DESKTOP\Codex++.lnk" "DesktopLegacySilent"
  !insertmacro UninstallShortcut "$DESKTOP\Codex++ 管理工具.lnk" "DesktopLegacyManager"
  !insertmacro UninstallShortcut "$DESKTOP\Codex++ 绠＄悊宸ュ叿.lnk" "DesktopMojibakeManager"
  !insertmacro UninstallShortcut "$DESKTOP\Chimera++.lnk" "DesktopChimeraSilent"
  !insertmacro UninstallShortcut "$DESKTOP\Chimera++ 管理工具.lnk" "DesktopChimeraManager"
  !insertmacro UninstallShortcut "$SMPROGRAMS\Codex++\Codex++.lnk" "MenuLegacySilent"
  !insertmacro UninstallShortcut "$SMPROGRAMS\Codex++\Codex++ 管理工具.lnk" "MenuLegacyManager"
  !insertmacro UninstallShortcut "$SMPROGRAMS\Codex++\Codex++ 绠＄悊宸ュ叿.lnk" "MenuMojibakeManager"
  !insertmacro UninstallShortcut "$SMPROGRAMS\Codex++\卸载 Codex++.lnk" "MenuLegacyUninstall"
  !insertmacro UninstallShortcut "$SMPROGRAMS\Codex++\Chimera++.lnk" "MenuCompatSilent"
  !insertmacro UninstallShortcut "$SMPROGRAMS\Codex++\Chimera++ 管理工具.lnk" "MenuCompatManager"
  !insertmacro UninstallShortcut "$SMPROGRAMS\Codex++\卸载 Chimera++.lnk" "MenuCompatUninstall"
  !insertmacro UninstallShortcut "$SMPROGRAMS\Chimera++\Chimera++.lnk" "MenuChimeraSilent"
  !insertmacro UninstallShortcut "$SMPROGRAMS\Chimera++\Chimera++ 管理工具.lnk" "MenuChimeraManager"
  !insertmacro UninstallShortcut "$SMPROGRAMS\Chimera++\卸载 Chimera++.lnk" "MenuChimeraUninstall"
  RMDir "$SMPROGRAMS\Codex++"
  RMDir "$SMPROGRAMS\Chimera++"

  ; 只删除本程序拥有的 URL protocol 值，未知扩展保持原样；随后由深到浅清理空键。
  !insertmacro UninstallBackedRegValue "${URL_PROTOCOL_SUBKEY}\shell\open\command" "" "UrlCommand"
  !insertmacro UninstallBackedRegValue "${URL_PROTOCOL_SUBKEY}" "URL Protocol" "UrlProtocol"
  !insertmacro UninstallBackedRegValue "${URL_PROTOCOL_SUBKEY}" "" "UrlDisplayName"
  !insertmacro UninstallRegKey "${URL_PROTOCOL_SUBKEY}\shell\open\command" "UrlCommandKey"
  !insertmacro UninstallRegKey "${URL_PROTOCOL_SUBKEY}\shell\open" "UrlOpenKey"
  !insertmacro UninstallRegKey "${URL_PROTOCOL_SUBKEY}\shell" "UrlShellKey"
  !insertmacro UninstallRegKey "${URL_PROTOCOL_SUBKEY}" "UrlProtocolKey"

  !insertmacro UninstallBackedRegValue "${CURRENT_UNINSTALL_KEY}" "DisplayName" "CurrentDisplayName"
  !insertmacro UninstallBackedRegValue "${CURRENT_UNINSTALL_KEY}" "DisplayVersion" "CurrentDisplayVersion"
  !insertmacro UninstallBackedRegValue "${CURRENT_UNINSTALL_KEY}" "Publisher" "CurrentPublisher"
  !insertmacro UninstallBackedRegValue "${CURRENT_UNINSTALL_KEY}" "DisplayIcon" "CurrentDisplayIcon"
  !insertmacro UninstallBackedRegValue "${CURRENT_UNINSTALL_KEY}" "InstallLocation" "CurrentInstallLocation"
  !insertmacro UninstallBackedRegValue "${CURRENT_UNINSTALL_KEY}" "UninstallString" "CurrentUninstallString"
  !insertmacro UninstallBackedRegValue "${CURRENT_UNINSTALL_KEY}" "QuietUninstallString" "CurrentQuietUninstallString"
  !insertmacro UninstallBackedRegValue "${LEGACY_UNINSTALL_KEY}" "DisplayName" "LegacyDisplayName"
  !insertmacro UninstallBackedRegValue "${LEGACY_UNINSTALL_KEY}" "DisplayVersion" "LegacyDisplayVersion"
  !insertmacro UninstallBackedRegValue "${LEGACY_UNINSTALL_KEY}" "Publisher" "LegacyPublisher"
  !insertmacro UninstallBackedRegValue "${LEGACY_UNINSTALL_KEY}" "DisplayIcon" "LegacyDisplayIcon"
  !insertmacro UninstallBackedRegValue "${LEGACY_UNINSTALL_KEY}" "InstallLocation" "LegacyInstallLocation"
  !insertmacro UninstallBackedRegValue "${LEGACY_UNINSTALL_KEY}" "UninstallString" "LegacyUninstallString"
  !insertmacro UninstallBackedRegValue "${LEGACY_UNINSTALL_KEY}" "QuietUninstallString" "LegacyQuietUninstallString"
  !insertmacro UninstallBackedRegValue "Software\Codex++" "InstallDir" "InstallDir"

  ClearErrors
  Delete "$INSTDIR\uninstall.exe"
  IfErrors uninstall_metadata_failed
  RMDir "$INSTDIR"
  Goto uninstall_complete

uninstall_metadata_backup_failed:
  Abort "无法创建卸载元数据备份，尚未删除程序或入口。"

uninstall_metadata_failed:
  StrCpy $R9 "0"
  !insertmacro RollbackShortcut "$DESKTOP\Codex++.lnk" "$DESKTOP" "DesktopLegacySilent"
  !insertmacro RollbackShortcut "$DESKTOP\Codex++ 管理工具.lnk" "$DESKTOP" "DesktopLegacyManager"
  !insertmacro RollbackShortcut "$DESKTOP\Codex++ 绠＄悊宸ュ叿.lnk" "$DESKTOP" "DesktopMojibakeManager"
  !insertmacro RollbackShortcut "$DESKTOP\Chimera++.lnk" "$DESKTOP" "DesktopChimeraSilent"
  !insertmacro RollbackShortcut "$DESKTOP\Chimera++ 管理工具.lnk" "$DESKTOP" "DesktopChimeraManager"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Codex++\Codex++.lnk" "$SMPROGRAMS\Codex++" "MenuLegacySilent"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Codex++\Codex++ 管理工具.lnk" "$SMPROGRAMS\Codex++" "MenuLegacyManager"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Codex++\Codex++ 绠＄悊宸ュ叿.lnk" "$SMPROGRAMS\Codex++" "MenuMojibakeManager"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Codex++\卸载 Codex++.lnk" "$SMPROGRAMS\Codex++" "MenuLegacyUninstall"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Codex++\Chimera++.lnk" "$SMPROGRAMS\Codex++" "MenuCompatSilent"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Codex++\Chimera++ 管理工具.lnk" "$SMPROGRAMS\Codex++" "MenuCompatManager"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Codex++\卸载 Chimera++.lnk" "$SMPROGRAMS\Codex++" "MenuCompatUninstall"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Chimera++\Chimera++.lnk" "$SMPROGRAMS\Chimera++" "MenuChimeraSilent"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Chimera++\Chimera++ 管理工具.lnk" "$SMPROGRAMS\Chimera++" "MenuChimeraManager"
  !insertmacro RollbackShortcut "$SMPROGRAMS\Chimera++\卸载 Chimera++.lnk" "$SMPROGRAMS\Chimera++" "MenuChimeraUninstall"
  !insertmacro RestoreRegValue "Software\Codex++" "InstallDir" "InstallDir"
  !insertmacro RestoreRegValue "${CURRENT_UNINSTALL_KEY}" "DisplayName" "CurrentDisplayName"
  !insertmacro RestoreRegValue "${CURRENT_UNINSTALL_KEY}" "DisplayVersion" "CurrentDisplayVersion"
  !insertmacro RestoreRegValue "${CURRENT_UNINSTALL_KEY}" "Publisher" "CurrentPublisher"
  !insertmacro RestoreRegValue "${CURRENT_UNINSTALL_KEY}" "DisplayIcon" "CurrentDisplayIcon"
  !insertmacro RestoreRegValue "${CURRENT_UNINSTALL_KEY}" "InstallLocation" "CurrentInstallLocation"
  !insertmacro RestoreRegValue "${CURRENT_UNINSTALL_KEY}" "UninstallString" "CurrentUninstallString"
  !insertmacro RestoreRegValue "${CURRENT_UNINSTALL_KEY}" "QuietUninstallString" "CurrentQuietUninstallString"
  !insertmacro RestoreRegValue "${LEGACY_UNINSTALL_KEY}" "DisplayName" "LegacyDisplayName"
  !insertmacro RestoreRegValue "${LEGACY_UNINSTALL_KEY}" "DisplayVersion" "LegacyDisplayVersion"
  !insertmacro RestoreRegValue "${LEGACY_UNINSTALL_KEY}" "Publisher" "LegacyPublisher"
  !insertmacro RestoreRegValue "${LEGACY_UNINSTALL_KEY}" "DisplayIcon" "LegacyDisplayIcon"
  !insertmacro RestoreRegValue "${LEGACY_UNINSTALL_KEY}" "InstallLocation" "LegacyInstallLocation"
  !insertmacro RestoreRegValue "${LEGACY_UNINSTALL_KEY}" "UninstallString" "LegacyUninstallString"
  !insertmacro RestoreRegValue "${LEGACY_UNINSTALL_KEY}" "QuietUninstallString" "LegacyQuietUninstallString"
  !insertmacro RestoreRegValue "${URL_PROTOCOL_SUBKEY}" "" "UrlDisplayName"
  !insertmacro RestoreRegValue "${URL_PROTOCOL_SUBKEY}" "URL Protocol" "UrlProtocol"
  !insertmacro RestoreRegValue "${URL_PROTOCOL_SUBKEY}\shell\open\command" "" "UrlCommand"
  StrCmp $R9 "0" uninstall_metadata_restored uninstall_metadata_rollback_failed

uninstall_metadata_restored:
  Abort "卸载元数据清理失败，快捷方式和注册项已恢复；请关闭占用后重试。"

uninstall_metadata_rollback_failed:
  Abort "卸载元数据清理失败，且自动恢复不完整；请使用安装包执行修复。"

uninstall_failed:
  Abort "程序文件或卸载元数据仍被占用，卸载已停止；卸载器已保留，请关闭相关进程后重试。"

uninstall_complete:
SectionEnd
