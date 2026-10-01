//! Just enough EDID parsing to tell monitors apart and show their names and sizes.

pub struct Edid {
    /// Three-letter manufacturer code, e.g. "GSM".
    pub manufacturer: String,
    pub product: u16,
    pub serial: Option<String>,
    pub name: Option<String>,
    /// Physical image size in mm, as the panel reports it.
    pub size_mm: Option<(f64, f64)>,
}

pub fn parse(b: &[u8]) -> Option<Edid> {
    if b.len() < 128 || b[..8] != [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00] {
        return None;
    }
    let m = u16::from_be_bytes([b[8], b[9]]);
    let letter = |v: u16| char::from(b'A' - 1 + (v & 0x1F) as u8);
    let manufacturer: String = [letter(m >> 10), letter(m >> 5), letter(m)].iter().collect();
    let product = u16::from_le_bytes([b[10], b[11]]);
    let serial_num = u32::from_le_bytes([b[12], b[13], b[14], b[15]]);

    let mut name = None;
    let mut serial_text = None;
    for d in (54..=108).step_by(18) {
        let desc = &b[d..d + 18];
        if desc[0] == 0 && desc[1] == 0 {
            let text = || {
                let s: String = desc[5..18].iter().take_while(|&&c| c != 0x0A).map(|&c| c as char).collect();
                Some(s.trim().to_string()).filter(|s| !s.is_empty())
            };
            match desc[3] {
                0xFC => name = text(),
                0xFF => serial_text = text(),
                _ => {}
            }
        }
    }

    // Detailed timing descriptor sizes are in mm; the basic block only in cm.
    let dtd = &b[54..72];
    let dtd_size = (dtd[0] != 0 || dtd[1] != 0).then(|| {
        let w = u16::from(dtd[12]) | (u16::from(dtd[14] & 0xF0) << 4);
        let h = u16::from(dtd[13]) | (u16::from(dtd[14] & 0x0F) << 8);
        (f64::from(w), f64::from(h))
    });
    let cm_size = (b[21] != 0 && b[22] != 0).then(|| (f64::from(b[21]) * 10.0, f64::from(b[22]) * 10.0));
    let size_mm = [dtd_size, cm_size].into_iter().flatten().find(|&(w, h)| plausible(w, h));

    Some(Edid {
        manufacturer,
        product,
        serial: serial_text.or((serial_num != 0).then(|| format!("{serial_num:08X}"))),
        name,
        size_mm,
    })
}

/// TVs and projectors often report 0, 1 cm, or an aspect ratio code instead of a size.
fn plausible(w: f64, h: f64) -> bool {
    (80.0..=3000.0).contains(&w) && (50.0..=3000.0).contains(&h) && (0.2..=5.0).contains(&(w / h))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<u8> {
        let mut b = vec![0u8; 128];
        b[..8].copy_from_slice(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);
        // "GSM" = G(7) S(19) M(13)
        let m: u16 = (7 << 10) | (19 << 5) | 13;
        b[8..10].copy_from_slice(&m.to_be_bytes());
        b[10..12].copy_from_slice(&0x5B09u16.to_le_bytes());
        b[21] = 60;
        b[22] = 34;
        // DTD with 597 x 336 mm
        b[54] = 0x01;
        b[66] = (597 & 0xFF) as u8;
        b[67] = (336 & 0xFF) as u8;
        b[68] = (((597 >> 8) as u8) << 4) | ((336 >> 8) as u8);
        // Name descriptor
        b[72..77].copy_from_slice(&[0, 0, 0, 0xFC, 0]);
        b[77..88].copy_from_slice(b"LG HDR 4K\n ");
        b
    }

    #[test]
    fn reads_identity_and_size() {
        let e = parse(&sample()).unwrap();
        assert_eq!(e.manufacturer, "GSM");
        assert_eq!(e.product, 0x5B09);
        assert_eq!(e.name.as_deref(), Some("LG HDR 4K"));
        assert_eq!(e.size_mm, Some((597.0, 336.0)));
    }

    #[test]
    fn falls_back_to_cm_when_timing_size_is_nonsense() {
        let mut b = sample();
        b[66] = 16;
        b[67] = 9;
        b[68] = 0;
        assert_eq!(parse(&b).unwrap().size_mm, Some((600.0, 340.0)));
    }

    #[test]
    fn rejects_non_edid() {
        assert!(parse(&[0u8; 128]).is_none());
    }
}
