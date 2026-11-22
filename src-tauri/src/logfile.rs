//! Log em arquivo, para relatar problemas. O release roda sem console, então tudo o que o
//! app escreve em stderr (os `eprintln!` e as mensagens de pânico) é desviado para
//! `%LOCALAPPDATA%\app.vitrolinha\logs\vitrolinha.log`, cada linha com data e hora.
//!
//! Só erros e estados do app. O que você está ouvindo (título, artista) é impresso em stdout,
//! só para depuração, e não entra no arquivo. Passa de 1 MB, o arquivo vira `vitrolinha.old.log`
//! (o anterior é descartado). Se o stderr já tem destino (console de desenvolvimento,
//! `-RedirectStandardError`), nada é desviado.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tauri::{AppHandle, Manager};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{GetStdHandle, SetStdHandle, STD_ERROR_HANDLE};
use windows::Win32::System::SystemInformation::GetLocalTime;

const FILE_NAME: &str = "vitrolinha.log";
const OLD_NAME: &str = "vitrolinha.old.log";
const MAX_BYTES: u64 = 1024 * 1024;

/// Pasta dos logs (`%LOCALAPPDATA%\app.vitrolinha\logs`).
pub fn dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_log_dir().ok()
}

/// Desvia o stderr para o arquivo de log. Chamada uma vez, no começo do `setup`.
pub fn init(app: &AppHandle) {
    if stderr_has_destination() {
        return;
    }
    let Some(dir) = dir(app) else { return };
    let Some(mut file) = open_log(&dir) else { return };
    let Ok((reader, writer)) = std::io::pipe() else { return };
    let _ = writeln!(file, "\n{} === Vitrolinha {} iniciada ===", now(), env!("CARGO_PKG_VERSION"));

    // Um cano em vez de apontar o stderr direto para o arquivo: assim cada linha ganha data e hora.
    // A thread precisa estar de pé antes do desvio; se o desvio falhar, o `writer` cai, a thread
    // vê o fim do cano e termina, e o stderr fica como estava.
    let spawned = std::thread::Builder::new().name("log".into()).spawn(move || drain(reader, &dir, file));
    if spawned.is_err() {
        return;
    }
    // SAFETY: o handle do `writer` é válido; depois do desvio ele pertence ao processo.
    if unsafe { SetStdHandle(STD_ERROR_HANDLE, HANDLE(writer.as_raw_handle())) }.is_ok() {
        std::mem::forget(writer); // fica aberto até o processo terminar
    }
}

/// Abre a pasta de logs no Explorer (item do menu da bandeja).
pub fn open_folder(app: &AppHandle) {
    let Some(dir) = dir(app) else { return };
    let _ = fs::create_dir_all(&dir);
    if let Err(e) = std::process::Command::new("explorer").arg(&dir).spawn() {
        eprintln!("[log] não consegui abrir a pasta de logs: {e}");
    }
}

/// O stderr já aponta para algum lugar (console, arquivo, cano)? Sem console e sem
/// redirecionamento o handle é nulo ou inválido.
fn stderr_has_destination() -> bool {
    // SAFETY: só consulta o handle padrão do processo.
    match unsafe { GetStdHandle(STD_ERROR_HANDLE) } {
        Ok(handle) => !handle.is_invalid() && !handle.0.is_null(),
        Err(_) => false,
    }
}

fn now() -> String {
    // SAFETY: sem argumentos; devolve a hora local.
    let t = unsafe { GetLocalTime() };
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}

/// Abre o log para acrescentar, girando o arquivo se já passou do limite.
fn open_log(dir: &Path) -> Option<File> {
    fs::create_dir_all(dir).ok()?;
    let path = dir.join(FILE_NAME);
    if fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = fs::rename(&path, dir.join(OLD_NAME)); // substitui o .old anterior
    }
    OpenOptions::new().create(true).append(true).open(path).ok()
}

/// Copia o que chega no cano para o arquivo, linha a linha, com data e hora. Nunca para de
/// esvaziar o cano (um cano cheio travaria quem escreve em stderr), mesmo se o disco falhar.
fn drain<R: Read>(reader: R, dir: &Path, mut file: File) {
    let mut reader = BufReader::new(reader);
    let mut buf = Vec::new();
    let mut written = file.metadata().map(|m| m.len()).unwrap_or(0);
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => {
                // Bytes inválidos viram � em vez de derrubar o log.
                let line = format!("{} {}\n", now(), String::from_utf8_lossy(&buf).trim_end());
                if file.write_all(line.as_bytes()).is_ok() {
                    written += line.len() as u64;
                }
                if written > MAX_BYTES {
                    if let Some(next) = open_log(dir) {
                        file = next;
                        written = 0;
                    }
                }
            }
            Err(_) => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vitrolinha-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn hora_tem_o_formato_esperado() {
        let t = now();
        assert_eq!(t.len(), 19, "{t}");
        assert_eq!(&t[4..5], "-");
        assert_eq!(&t[10..11], " ");
        assert_eq!(&t[13..14], ":");
    }

    #[test]
    fn cada_linha_ganha_hora_e_bytes_invalidos_nao_derrubam() {
        let dir = temp_dir("linhas");
        let file = open_log(&dir).unwrap();
        let entrada = b"[discord] conectado\n\xff\xfe quebrada\nsem quebra no fim".to_vec();
        drain(Cursor::new(entrada), &dir, file);
        let texto = fs::read_to_string(dir.join(FILE_NAME)).unwrap();
        let linhas: Vec<&str> = texto.lines().collect();
        assert_eq!(linhas.len(), 3, "{texto}");
        assert!(linhas[0].ends_with("[discord] conectado"));
        assert!(linhas[1].contains('\u{FFFD}') && linhas[1].ends_with("quebrada"));
        assert!(linhas[2].ends_with("sem quebra no fim"));
        assert!(linhas.iter().all(|l| l.as_bytes()[4] == b'-'));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn arquivo_grande_vira_old_ao_abrir_e_durante_o_uso() {
        let dir = temp_dir("giro");
        fs::write(dir.join(FILE_NAME), vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
        fs::write(dir.join(OLD_NAME), "antigo").unwrap();
        let file = open_log(&dir).unwrap();
        assert_eq!(fs::metadata(dir.join(FILE_NAME)).unwrap().len(), 0);
        assert_eq!(fs::metadata(dir.join(OLD_NAME)).unwrap().len(), MAX_BYTES + 1);

        // Durante o uso: passa do limite no meio do fluxo e continua gravando no arquivo novo.
        let linha = "y".repeat(1000) + "\n";
        let entrada = linha.repeat(1100); // ~1,1 MB
        drain(Cursor::new(entrada), &dir, file);
        assert!(fs::metadata(dir.join(FILE_NAME)).unwrap().len() < MAX_BYTES);
        assert!(fs::metadata(dir.join(OLD_NAME)).unwrap().len() > 1_000_000);
        let _ = fs::remove_dir_all(&dir);
    }
}
