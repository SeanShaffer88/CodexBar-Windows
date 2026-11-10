; CodexBar-Windows installer (NSIS). Build with:
;   makensis /DVERSION=0.1.0 codexbar-windows.nsi
; Expects the release exe at ..\target\x86_64-pc-windows-msvc\release\codexbar-windows.exe
; (the release workflow builds it there before invoking this script).

!ifndef VERSION
  !define VERSION "0.1.0"
!endif

Name "CodexBar-Windows"
OutFile "..\CodexBar-Windows-v${VERSION}-x64-Setup.exe"
InstallDir "$LOCALAPPDATA\Programs\CodexBar-Windows"
RequestExecutionLevel user
Unicode true
Icon "..\assets\app.ico"
UninstallIcon "..\assets\app.ico"

Page Directory
Page InstFiles
UninstPage UninstConfirm
UninstPage InstFiles

Section "Install"
  SetOutPath "$INSTDIR"
  File "..\target\x86_64-pc-windows-msvc\release\codexbar-windows.exe"
  File "..\README.md"
  File "..\LICENSE"
  CreateDirectory "$SMPROGRAMS\CodexBar-Windows"
  CreateShortcut "$SMPROGRAMS\CodexBar-Windows\CodexBar-Windows.lnk" "$INSTDIR\codexbar-windows.exe"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CodexBar-Windows" "DisplayName" "CodexBar-Windows"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CodexBar-Windows" "DisplayIcon" "$INSTDIR\codexbar-windows.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CodexBar-Windows" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CodexBar-Windows" "UninstallString" "$INSTDIR\Uninstall.exe"
SectionEnd

Section "Uninstall"
  Delete "$SMPROGRAMS\CodexBar-Windows\CodexBar-Windows.lnk"
  RMDir "$SMPROGRAMS\CodexBar-Windows"
  Delete "$INSTDIR\codexbar-windows.exe"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\Uninstall.exe"
  ; The portable data/ folder is left in place on purpose: it holds the
  ; user's history and settings. Remove it manually or via `clean`.
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CodexBar-Windows"
SectionEnd
