//! Detecta jogo/tela cheia/apresentação, quando o popup não deve aparecer.

use windows::Win32::UI::Shell::{
    SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
};

/// `Some(motivo)` se o popup deve ser suprimido agora.
pub fn suppress_reason() -> Option<&'static str> {
    // Falha ao consultar → não suprime (melhor mostrar do que sumir para sempre).
    match unsafe { SHQueryUserNotificationState() }.ok()? {
        s if s == QUNS_BUSY => Some("tela cheia (QUNS_BUSY)"),
        s if s == QUNS_RUNNING_D3D_FULL_SCREEN => Some("jogo em tela cheia (QUNS_RUNNING_D3D_FULL_SCREEN)"),
        s if s == QUNS_PRESENTATION_MODE => Some("modo apresentação (QUNS_PRESENTATION_MODE)"),
        _ => None,
    }
}
