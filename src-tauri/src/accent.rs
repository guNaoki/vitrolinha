//! Cor de destaque a partir dos pixels da capa (uma vez por faixa).
//!
//! Pega o matiz dominante entre os pixels coloridos (ignora cinzas e escuros),
//! e ajusta saturação e luminosidade para servir de acento sobre o fundo escuro
//! do tema. Capa praticamente cinza → `None` (o front mantém o âmbar padrão).

const BINS: usize = 36; // matizes de 10°
/// Pixels com saturação/valor abaixo disso não contam como "cor".
const MIN_SAT: f64 = 0.25;
const MIN_VAL: f64 = 0.20;
/// Fração mínima de pixels coloridos; abaixo disso a capa é considerada cinza.
const MIN_COLORFUL: f64 = 0.08;
/// Faixa de saturação/luminosidade do acento final (legível sobre #0d0a07).
const SAT_RANGE: (f64, f64) = (0.55, 0.90);
const LIGHT_RANGE: (f64, f64) = (0.52, 0.66);

/// `px`: BGRA de 8 bits, `width × height`. Devolve "#rrggbb".
pub fn accent_from_bgra(px: &[u8], width: u32, height: u32) -> Option<String> {
    let (w, h) = (width as usize, height as usize);
    let total = w * h;
    if total == 0 || px.len() < total * 4 {
        return None;
    }
    // ~4096 amostras bastam para achar o matiz dominante.
    let step = (((total as f64) / 4096.0).sqrt().ceil() as usize).max(1);

    let mut weight = [0f64; BINS];
    let mut sum = [[0f64; 3]; BINS];
    let (mut samples, mut colorful) = (0usize, 0usize);

    for y in (0..h).step_by(step) {
        for x in (0..w).step_by(step) {
            let i = (y * w + x) * 4;
            let (r, g, b) = (px[i + 2] as f64 / 255.0, px[i + 1] as f64 / 255.0, px[i] as f64 / 255.0);
            samples += 1;
            let (hue, sat, val) = hsv(r, g, b);
            if sat < MIN_SAT || val < MIN_VAL {
                continue;
            }
            colorful += 1;
            let bin = ((hue / 360.0) * BINS as f64) as usize % BINS;
            let wgt = sat * val; // pixels vivos e claros pesam mais
            weight[bin] += wgt;
            sum[bin][0] += r * wgt;
            sum[bin][1] += g * wgt;
            sum[bin][2] += b * wgt;
        }
    }
    if samples == 0 || (colorful as f64) < samples as f64 * MIN_COLORFUL {
        return None;
    }

    // O matiz é circular: cada faixa também recebe metade do peso das vizinhas.
    let score = |i: usize| weight[i] + 0.5 * (weight[(i + 1) % BINS] + weight[(i + BINS - 1) % BINS]);
    let best = (0..BINS).max_by(|&a, &b| score(a).total_cmp(&score(b)))?;
    if weight[best] <= 0.0 {
        return None;
    }
    let (r, g, b) = (sum[best][0] / weight[best], sum[best][1] / weight[best], sum[best][2] / weight[best]);

    let (hue, sat, light) = hsl(r, g, b);
    let (r, g, b) = hsl_to_rgb(
        hue,
        sat.clamp(SAT_RANGE.0, SAT_RANGE.1),
        light.clamp(LIGHT_RANGE.0, LIGHT_RANGE.1),
    );
    Some(format!("#{:02x}{:02x}{:02x}", to_u8(r), to_u8(g), to_u8(b)))
}

fn to_u8(v: f64) -> u8 {
    (v * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Matiz em graus [0, 360), a partir de max/min já conhecidos.
fn hue(r: f64, g: f64, b: f64, max: f64, min: f64) -> f64 {
    let d = max - min;
    if d == 0.0 {
        return 0.0;
    }
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    h * 60.0
}

fn hsv(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let s = if max == 0.0 { 0.0 } else { (max - min) / max };
    (hue(r, g, b, max, min), s, max)
}

fn hsl(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    let d = max - min;
    let s = if d == 0.0 { 0.0 } else { d / (1.0 - (2.0 * l - 1.0).abs()) };
    (hue(r, g, b, max, min), s, l)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0) as u32 % 6 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (r + m, g + m, b + m)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Imagem `w×h` BGRA onde cada pixel vem de `f(x, y) -> (r, g, b)`.
    fn image(w: u32, h: u32, f: impl Fn(u32, u32) -> (u8, u8, u8)) -> Vec<u8> {
        let mut v = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let (r, g, b) = f(x, y);
                v.extend([b, g, r, 255]);
            }
        }
        v
    }

    fn rgb(hex: &str) -> (u8, u8, u8) {
        let n = u32::from_str_radix(&hex[1..], 16).unwrap();
        ((n >> 16) as u8, (n >> 8) as u8, n as u8)
    }

    #[test]
    fn solid_red_gives_a_red_accent() {
        let px = image(64, 64, |_, _| (200, 30, 30));
        let (r, g, b) = rgb(&accent_from_bgra(&px, 64, 64).unwrap());
        assert!(r > g + 60 && r > b + 60, "esperava vermelho, veio {r},{g},{b}");
    }

    #[test]
    fn gray_cover_has_no_accent() {
        let px = image(64, 64, |x, _| ((x * 4) as u8, (x * 4) as u8, (x * 4) as u8));
        assert_eq!(accent_from_bgra(&px, 64, 64), None);
    }

    #[test]
    fn dominant_hue_wins_over_a_small_patch() {
        // 3/4 azul, 1/4 vermelho → azul.
        let px = image(64, 64, |x, _| if x < 48 { (30, 60, 200) } else { (200, 30, 30) });
        let (r, _, b) = rgb(&accent_from_bgra(&px, 64, 64).unwrap());
        assert!(b > r + 60, "esperava azul, veio r={r} b={b}");
    }

    #[test]
    fn accent_is_legible_on_dark_background() {
        // Marrom bem escuro → o acento é clareado para a faixa de luminosidade.
        let px = image(64, 64, |_, _| (90, 40, 20));
        let (r, g, b) = rgb(&accent_from_bgra(&px, 64, 64).unwrap());
        let l = (r.max(g).max(b) as f64 + r.min(g).min(b) as f64) / 2.0 / 255.0;
        assert!((0.50..=0.68).contains(&l), "luminosidade {l}");
    }

    #[test]
    fn mostly_black_cover_has_no_accent() {
        let px = image(64, 64, |x, y| if x < 4 && y < 4 { (200, 30, 30) } else { (5, 5, 5) });
        assert_eq!(accent_from_bgra(&px, 64, 64), None);
    }

    #[test]
    fn short_buffer_is_rejected() {
        assert_eq!(accent_from_bgra(&[0; 8], 64, 64), None);
    }
}
