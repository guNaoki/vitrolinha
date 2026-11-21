//! Capa do álbum: decodifica o thumbnail do GSMTC, reduz para ~1024px (lado
//! menor) e grava um JPEG temporário. Feito UMA vez por faixa; o resultado é
//! reutilizado enquanto a faixa não muda (ver `media::sample`).
//!
//! Usa o Windows.Graphics.Imaging do próprio sistema, sem crate de imagem.

use std::fs;
use std::path::{Path, PathBuf};

use crate::accent;
use windows::core::Result;
use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapEncoder, BitmapInterpolationMode, BitmapPixelFormat,
    BitmapTransform, ColorManagementMode, ExifOrientationMode,
};
use windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties as Props;
use windows::Storage::Streams::{DataReader, DataWriter, InMemoryRandomAccessStream};

const TARGET: u32 = 1024;

pub struct ArtCache {
    dir: PathBuf,
    seq: u32,
    current: Option<PathBuf>,
    /// Lado menor do último thumbnail entregue pelo player (0 = sem capa).
    short: u32,
}

impl ArtCache {
    pub fn new() -> Self {
        let dir = std::env::temp_dir().join("flyout-art");
        let _ = fs::remove_dir_all(&dir); // lixo de execuções anteriores
        let _ = fs::create_dir_all(&dir);
        Self { dir, seq: 0, current: None, short: 0 }
    }

    /// Tamanho (lado menor, px) do último thumbnail do player: decide se vale buscar uma capa melhor.
    pub fn short_side(&self) -> u32 {
        self.short
    }

    /// Extrai a capa de `props`. `None` se a fonte não tem capa ou falhou.
    pub fn extract(&mut self, props: &Props) -> Option<Cover> {
        let (bytes, accent, short) = match encode_cover(props) {
            Ok(Some(c)) => c,
            Ok(None) => {
                self.short = 0;
                return None;
            }
            Err(e) => {
                eprintln!("[art] falha ao processar a capa: {e}");
                self.short = 0;
                return None;
            }
        };
        self.short = short;
        self.seq += 1; // nome novo a cada capa: sem cache velho no WebView
        let path = self.dir.join(format!("{}.jpg", self.seq));
        if let Err(e) = fs::write(&path, bytes) {
            eprintln!("[art] falha ao gravar {}: {e}", path.display());
            return None;
        }
        remove(self.current.replace(path.clone()).as_deref());
        Some(Cover { path: path.to_string_lossy().into_owned(), accent })
    }
}

/// Resultado de `extract`: arquivo da capa + cor de destaque ("#rrggbb").
pub struct Cover {
    pub path: String,
    pub accent: Option<String>,
}

fn remove(path: Option<&Path>) {
    if let Some(p) = path {
        let _ = fs::remove_file(p);
    }
}

/// Reduz uma imagem (JPEG/PNG em memória) para `target` px no lado menor, sem ampliar,
/// e devolve JPEG. Usado pela estante (miniaturas leves). Precisa de WinRT (MTA) na thread.
pub fn shrink_jpeg(bytes: &[u8], target: u32) -> Option<Vec<u8>> {
    match shrink(bytes, target) {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("[art] falha ao reduzir a miniatura: {e}");
            None
        }
    }
}

fn shrink(bytes: &[u8], target: u32) -> Result<Vec<u8>> {
    let stream = InMemoryRandomAccessStream::new()?;
    let writer = DataWriter::CreateDataWriter(&stream.GetOutputStreamAt(0)?)?;
    writer.WriteBytes(bytes)?;
    writer.StoreAsync()?.join()?;
    writer.FlushAsync()?.join()?;
    writer.DetachStream()?;
    stream.Seek(0)?;

    let decoder = BitmapDecoder::CreateAsync(&stream)?.join()?;
    let (w, h) = (decoder.PixelWidth()?, decoder.PixelHeight()?);
    if w == 0 || h == 0 {
        return Err(windows::core::Error::from_hresult(windows::core::HRESULT(0x8000_4005u32 as i32)));
    }
    let short = w.min(h);
    let k = if short > target { target as f64 / short as f64 } else { 1.0 };
    let (nw, nh) = (((w as f64 * k).round() as u32).max(1), ((h as f64 * k).round() as u32).max(1));

    let transform = BitmapTransform::new()?;
    transform.SetScaledWidth(nw)?;
    transform.SetScaledHeight(nh)?;
    transform.SetInterpolationMode(BitmapInterpolationMode::Fant)?;
    let pixels = decoder
        .GetPixelDataTransformedAsync(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Ignore,
            &transform,
            ExifOrientationMode::IgnoreExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )?
        .join()?
        .DetachPixelData()?;

    let mem = InMemoryRandomAccessStream::new()?;
    let encoder = BitmapEncoder::CreateAsync(BitmapEncoder::JpegEncoderId()?, &mem)?.join()?;
    encoder.SetPixelData(BitmapPixelFormat::Bgra8, BitmapAlphaMode::Ignore, nw, nh, 96.0, 96.0, &pixels)?;
    encoder.FlushAsync()?.join()?;
    let size = mem.Size()? as u32;
    let reader = DataReader::CreateDataReader(&mem.GetInputStreamAt(0)?)?;
    reader.LoadAsync(size)?.join()?;
    let mut out = vec![0u8; size as usize];
    reader.ReadBytes(&mut out)?;
    Ok(out)
}

/// JPEG reduzido + cor de destaque, calculada nos mesmos pixels (sem decodificar de novo).
fn encode_cover(props: &Props) -> Result<Option<(Vec<u8>, Option<String>, u32)>> {
    let Ok(thumb) = props.Thumbnail() else {
        return Ok(None); // sem thumbnail
    };
    let stream = thumb.OpenReadAsync()?.join()?;
    let decoder = BitmapDecoder::CreateAsync(&stream)?.join()?;

    let (w, h) = (decoder.PixelWidth()?, decoder.PixelHeight()?);
    if w == 0 || h == 0 {
        return Ok(None);
    }
    // Lado menor → TARGET (nunca amplia). O CSS faz o recorte circular.
    let short = w.min(h);
    let k = if short > TARGET { TARGET as f64 / short as f64 } else { 1.0 };
    let (nw, nh) = (((w as f64 * k).round() as u32).max(1), ((h as f64 * k).round() as u32).max(1));

    let transform = BitmapTransform::new()?;
    transform.SetScaledWidth(nw)?;
    transform.SetScaledHeight(nh)?;
    transform.SetInterpolationMode(BitmapInterpolationMode::Fant)?;

    let pixels = decoder
        .GetPixelDataTransformedAsync(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Ignore,
            &transform,
            ExifOrientationMode::IgnoreExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )?
        .join()?
        .DetachPixelData()?;

    let accent = accent::accent_from_bgra(&pixels, nw, nh);

    let mem = InMemoryRandomAccessStream::new()?;
    let encoder = BitmapEncoder::CreateAsync(BitmapEncoder::JpegEncoderId()?, &mem)?.join()?;
    encoder.SetPixelData(BitmapPixelFormat::Bgra8, BitmapAlphaMode::Ignore, nw, nh, 96.0, 96.0, &pixels)?;
    encoder.FlushAsync()?.join()?;

    let size = mem.Size()? as u32;
    let reader = DataReader::CreateDataReader(&mem.GetInputStreamAt(0)?)?;
    reader.LoadAsync(size)?.join()?;
    let mut out = vec![0u8; size as usize];
    reader.ReadBytes(&mut out)?;
    Ok(Some((out, accent, short)))
}
