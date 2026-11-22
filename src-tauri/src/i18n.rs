//! Idioma da interface: português (padrão do projeto) ou inglês. "Automático" segue o idioma do
//! Windows (qualquer `pt-*` fica em português; o resto, em inglês).
//!
//! As traduções são por texto: o código continua escrevendo o texto em português e `tr` devolve o
//! inglês quando o idioma é esse (sem tabela de chaves para manter em dia). Texto que não está
//! na tabela aparece em português mesmo, em vez de quebrar. O front tem a mesma ideia em
//! `src/lib/i18n.ts`.

use tauri::{AppHandle, Emitter};
use windows::Win32::Globalization::GetUserDefaultLocaleName;

use crate::settings;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Pt,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Pt => "pt",
            Lang::En => "en",
        }
    }
}

/// Idioma do Windows ("pt-BR", "en-US"...). Se não der para saber, português.
fn system_lang() -> Lang {
    let mut buf = [0u16; 85]; // LOCALE_NAME_MAX_LENGTH
    let n = unsafe { GetUserDefaultLocaleName(&mut buf) };
    if n <= 0 {
        return Lang::Pt;
    }
    let name = String::from_utf16_lossy(&buf[..(n as usize).saturating_sub(1)]).to_ascii_lowercase();
    if name.starts_with("pt") { Lang::Pt } else { Lang::En }
}

/// O idioma em uso: o escolhido no menu, ou o do Windows.
pub fn lang(app: &AppHandle) -> Lang {
    match settings::language(app).as_str() {
        "pt" => Lang::Pt,
        "en" => Lang::En,
        _ => system_lang(),
    }
}

/// Grava a escolha ("auto" | "pt" | "en") e avisa as janelas (elas recarregam no idioma novo).
pub fn set_language(app: &AppHandle, choice: &str) {
    settings::set_language(app, choice);
    let _ = app.emit("lang", lang(app).code());
}

/// Traduz um texto (escrito em português). Em português, devolve o próprio texto.
pub fn tr(app: &AppHandle, pt: &str) -> String {
    translate(lang(app), pt).to_string()
}

/// Como `tr`, depois substitui cada `{}` pelos argumentos, na ordem.
pub fn fmt(app: &AppHandle, pt: &str, args: &[&str]) -> String {
    let mut out = String::new();
    let mut rest = translate(lang(app), pt);
    for a in args {
        match rest.split_once("{}") {
            Some((head, tail)) => {
                out.push_str(head);
                out.push_str(a);
                rest = tail;
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

fn translate(lang: Lang, pt: &str) -> &str {
    if lang == Lang::Pt {
        return pt;
    }
    EN.iter().find(|(k, _)| *k == pt).map_or(pt, |(_, v)| *v)
}

const EN: &[(&str, &str)] = &[
    ("Em cima, à esquerda", "Top left"),
    ("Em cima, centralizado", "Top center"),
    ("Em cima, à direita", "Top right"),
    ("Embaixo, à esquerda", "Bottom left"),
    ("Embaixo, centralizado", "Bottom center"),
    ("Embaixo, à direita", "Bottom right"),
    ("Posição", "Position"),
    ("Tempo na tela", "Time on screen"),
    ("Popup", "Popup"),
    ("Mostrar ao trocar de faixa", "Show when the track changes"),
    ("Mostrar também ao pausar/tocar", "Also show on pause/play"),
    ("Painel", "Panel"),
    ("Mostrar painel", "Show panel"),
    ("Monitor", "Monitor"),
    ("Monitor {} — {}×{}{}", "Monitor {} — {}×{}{}"),
    (" (principal)", " (primary)"),
    ("Ocupar o monitor inteiro", "Fill the whole monitor"),
    ("Fonte de mídia", "Media source"),
    ("Todos os apps", "All apps"),
    ("Só {}", "Only {}"),
    ("Só {} (fechado)", "Only {} (closed)"),
    ("Estante", "Shelf"),
    ("Guardar o que eu ouço (só neste computador)", "Remember what I listen to (this computer only)"),
    ("Limpar a estante (apaga o histórico)", "Clear the shelf (deletes the history)"),
    ("Efeitos do disco", "Record effects"),
    ("Desligados", "Off"),
    ("Suaves", "Soft"),
    ("Fortes", "Strong"),
    ("Vinil", "Vinyl"),
    ("Rotação do disco", "Record speed"),
    ("Som de disco antigo (com o painel aberto)", "Old record sound (while the panel is open)"),
    ("Desligado", "Off"),
    ("Leve", "Light"),
    ("Médio", "Medium"),
    ("Gasto (muito velho)", "Worn (very old)"),
    ("Dormir (pausar depois de...)", "Sleep timer (pause after...)"),
    ("Dormir — pausa em {} min", "Sleep — pausing in {} min"),
    ("{} minutos", "{} minutes"),
    ("Cancelar o timer", "Cancel the timer"),
    ("Cor de destaque da capa", "Accent color from the cover"),
    ("Letras sincronizadas (consulta lrclib.net)", "Synced lyrics (queries lrclib.net)"),
    ("Capa em alta resolução (consulta iTunes)", "High-resolution cover (queries iTunes)"),
    ("Call do Discord na tela cheia (precisa de configuração)", "Discord call on the full screen (needs setup)"),
    ("Mostrar no Discord o que estou ouvindo", "Show on Discord what I'm listening to"),
    (
        "Mostrar no Discord o que estou ouvindo (configure o Discord antes)",
        "Show on Discord what I'm listening to (set up Discord first)",
    ),
    ("Overlay para OBS (http://localhost:{})", "OBS overlay (http://localhost:{})"),
    ("Atalhos globais ({})", "Global shortcuts ({})"),
    ("Iniciar com o Windows", "Start with Windows"),
    ("Idioma", "Language"),
    ("Automático (idioma do Windows)", "Automatic (Windows language)"),
    ("Abrir a pasta de logs (para relatar um problema)", "Open the logs folder (to report a problem)"),
    ("Sair", "Quit"),
];

#[cfg(test)]
mod tests {
    use super::{translate, Lang, EN};

    #[test]
    fn portuguese_is_the_identity() {
        assert_eq!(translate(Lang::Pt, "Sair"), "Sair");
    }

    #[test]
    fn english_translates_known_and_keeps_unknown() {
        assert_eq!(translate(Lang::En, "Sair"), "Quit");
        assert_eq!(translate(Lang::En, "texto que não está na tabela"), "texto que não está na tabela");
    }

    #[test]
    fn placeholders_match_between_languages() {
        for (pt, en) in EN {
            assert_eq!(pt.matches("{}").count(), en.matches("{}").count(), "{pt}");
        }
    }
}
