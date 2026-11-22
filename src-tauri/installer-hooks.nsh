; Ganchos do instalador NSIS (ver bundle.windows.nsis.installerHooks no tauri.conf.json).

; "Iniciar com o Windows" grava uma entrada em Run com o nome do produto. Sem isto, depois de
; desinstalar ela sobraria apontando para um executável que não existe mais.
!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "${PRODUCTNAME}"
!macroend
