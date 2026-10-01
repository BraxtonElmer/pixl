; Extra installer steps for Pixl.

; True when Pixl wasn't installed before, so first-time defaults apply.
Var PixlFreshInstall

!macro NSIS_HOOK_PREINSTALL
  StrCpy $PixlFreshInstall "1"
  IfFileExists "$INSTDIR\Pixl.exe" 0 +2
    StrCpy $PixlFreshInstall "0"
  ; The tray app keeps running in the background; close both apps so they can
  ; be replaced. Any black screens go away with it.
  nsExec::Exec 'taskkill /F /IM Pixl.exe'
  nsExec::Exec 'taskkill /F /IM pixl-settings.exe'
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Start with Windows by default. Only on a fresh install: an update keeps
  ; whatever the user chose in the settings.
  StrCmp $PixlFreshInstall "1" 0 +2
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Pixl" '"$INSTDIR\Pixl.exe" --background'
  ; Start the tray app right away.
  Exec '"$INSTDIR\Pixl.exe" --background'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::Exec 'taskkill /F /IM Pixl.exe'
  nsExec::Exec 'taskkill /F /IM pixl-settings.exe'
  ; Remove "Start with Windows".
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Pixl"
!macroend
