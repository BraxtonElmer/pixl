; Extra installer steps for Pixl.

!macro NSIS_HOOK_PREINSTALL
  ; The tray app keeps running in the background; close both apps so they can
  ; be replaced. Any black screens go away with it.
  nsExec::Exec 'taskkill /F /IM Pixl.exe'
  nsExec::Exec 'taskkill /F /IM pixl-settings.exe'
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Start the tray app right away.
  Exec '"$INSTDIR\Pixl.exe" --background'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::Exec 'taskkill /F /IM Pixl.exe'
  nsExec::Exec 'taskkill /F /IM pixl-settings.exe'
  ; Remove "Start with Windows".
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Pixl"
!macroend
