//! Cartão para compartilhar: o front desenha a imagem num canvas; aqui só ficam as duas coisas
//! que ele não pode fazer sozinho: ler a capa (arquivo do app) e gravar o PNG em Imagens.

use std::path::{Path, PathBuf};

use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{AppHandle, Manager};

/// Pastas de onde o painel pode pedir uma capa: a temporária da faixa atual e a da estante.
pub(crate) fn cover_dirs(app: &AppHandle) -> Vec<PathBuf> {
    let mut dirs = vec![std::env::temp_dir().join("vitrolinha-art")];
    if let Ok(data) = app.path().app_data_dir() {
        dirs.push(data.join("shelf"));
    }
    dirs.into_iter().filter_map(|d| d.canonicalize().ok()).collect()
}

/// Bytes da capa. Só lê arquivos das pastas do próprio app (o painel não vira leitor de arquivos).
#[tauri::command]
pub fn read_cover(app: AppHandle, path: String) -> Result<Response, String> {
    let file = Path::new(&path).canonicalize().map_err(|_| "capa não encontrada")?;
    if !cover_dirs(&app).iter().any(|d| file.starts_with(d)) {
        return Err("caminho fora das pastas do app".into());
    }
    std::fs::read(&file).map(Response::new).map_err(|e| e.to_string())
}

/// Nome de arquivo seguro a partir do texto da faixa.
fn safe_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') { c } else { '_' })
        .collect();
    let trimmed = cleaned.trim_matches(|c: char| c == ' ' || c == '.' || c == '_');
    let short: String = trimmed.chars().take(80).collect();
    if short.is_empty() { "cartao".into() } else { short }
}

/// Grava o PNG (corpo bruto da chamada; o nome vem no cabeçalho `x-name`, codificado em URL)
/// em Imagens\Vitrolinha e devolve o caminho.
#[tauri::command]
pub fn save_share_image(app: AppHandle, request: Request<'_>) -> Result<String, String> {
    let InvokeBody::Raw(png) = request.body() else {
        return Err("imagem ausente".into());
    };
    if png.len() < 8 || &png[..4] != b"\x89PNG" || png.len() > 40 * 1024 * 1024 {
        return Err("não é um PNG válido".into());
    }
    let name = request
        .headers()
        .get("x-name")
        .and_then(|v| v.to_str().ok())
        .map(|v| percent_decode(v))
        .unwrap_or_default();
    let dir = app.path().picture_dir().map_err(|e| e.to_string())?.join("Vitrolinha");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let base = format!("Vitrolinha - {}", safe_name(&name));
    let mut file = dir.join(format!("{base}.png"));
    let mut n = 2;
    while file.exists() {
        file = dir.join(format!("{base} ({n}).png"));
        n += 1;
    }
    std::fs::write(&file, png).map_err(|e| e.to_string())?;
    Ok(file.to_string_lossy().into_owned())
}

/// Coloca a imagem na área de transferência (formato DIB: o corpo bruto é o cabeçalho BITMAPINFO
/// + pixels, montado pelo front). Feito aqui porque o `navigator.clipboard` do WebView pode
/// recusar sem permissão, e colar a imagem direto no chat é o uso principal do cartão.
#[tauri::command]
pub fn copy_share_image(app: AppHandle, request: Request<'_>) -> Result<(), String> {
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HWND};
    use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    const CF_DIB: u32 = 8;
    let InvokeBody::Raw(dib) = request.body() else {
        return Err("imagem ausente".into());
    };
    // Cabeçalho de 40 bytes (BITMAPINFOHEADER) e um limite de tamanho razoável.
    if dib.len() < 40 || dib.len() > 64 * 1024 * 1024 {
        return Err("imagem inválida".into());
    }
    // A área de transferência precisa de uma janela dona (com `None`, SetClipboardData falha).
    let hwnd = app
        .get_webview_window(crate::windows::PANEL)
        .ok_or("painel não encontrado")?
        .hwnd()
        .map_err(|e| e.to_string())?;
    let hwnd = HWND(hwnd.0);
    unsafe {
        let mem = GlobalAlloc(GMEM_MOVEABLE, dib.len()).map_err(|e| e.to_string())?;
        let dst = GlobalLock(mem);
        if dst.is_null() {
            let _ = GlobalFree(Some(mem));
            return Err("sem memória".into());
        }
        std::ptr::copy_nonoverlapping(dib.as_ptr(), dst as *mut u8, dib.len());
        let _ = GlobalUnlock(mem);
        if let Err(e) = OpenClipboard(Some(hwnd)) {
            let _ = GlobalFree(Some(mem));
            return Err(format!("área de transferência ocupada: {e}"));
        }
        let result = EmptyClipboard().and_then(|_| SetClipboardData(CF_DIB, Some(HANDLE(mem.0))));
        let _ = CloseClipboard();
        match result {
            Ok(_) => Ok(()), // a partir daqui o sistema é dono da memória
            Err(e) => {
                let _ = GlobalFree(Some(mem));
                Err(e.to_string())
            }
        }
    }
}

/// %XX → byte (o resto passa direto). Sem crate: só o que o cabeçalho precisa.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::{percent_decode, safe_name};

    #[test]
    fn names_are_filesystem_safe() {
        assert_eq!(safe_name("Tom Jobim / Águas de Março?"), "Tom Jobim _ Águas de Março");
        assert_eq!(safe_name("..."), "cartao");
        assert_eq!(safe_name("a\\b:c"), "a_b_c");
    }

    #[test]
    fn decodes_percent_escapes() {
        assert_eq!(percent_decode("%C3%81guas%20de%20Mar%C3%A7o"), "Águas de Março");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("a%2"), "a%2");
    }
}
