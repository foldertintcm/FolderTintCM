//! Writing `.ico` files: 32-bit DIBs for the small sizes and a PNG for the
//! 256-pixel one, as Windows' own icons store them. The PNG encoder is the
//! minimum a folder icon needs — each row filtered against the one above,
//! then deflated with the fixed Huffman code and only two match distances
//! (the byte before, the pixel before) — which shrinks a 256-pixel folder
//! from 256 KB to a few KB without a compression crate.
//!
//! Pure Rust, no Windows calls: `build.rs` includes this file too.

/// The sizes Explorer asks for, from the details view (16) up to extra
/// large icons (256), with the in-between steps of 125–250 % scaling.
pub const SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];

/// An `.ico` from a renderer of straight-alpha RGBA, top-down, square.
pub fn ico(render: impl Fn(u32) -> Vec<u8>) -> Vec<u8> {
    let images: Vec<Vec<u8>> = SIZES
        .iter()
        .map(|&s| {
            let rgba = render(s);
            if s >= 256 {
                png(s, s, &rgba)
            } else {
                dib(s, &rgba)
            }
        })
        .collect();
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]); // reserved, type 1 = icon
    out.extend_from_slice(&(SIZES.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * SIZES.len() as u32;
    for (&s, img) in SIZES.iter().zip(&images) {
        let dim = if s >= 256 { 0 } else { s as u8 }; // 0 means 256
        out.extend_from_slice(&[dim, dim, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(img.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += img.len() as u32;
    }
    for img in images {
        out.extend_from_slice(&img);
    }
    out
}

/// A 32-bit BGRA DIB with its (unused, but required) AND mask.
fn dib(size: u32, rgba: &[u8]) -> Vec<u8> {
    let mask_stride = size.div_ceil(32) * 4;
    let mut out = Vec::with_capacity(40 + rgba.len() + (mask_stride * size) as usize);
    // BITMAPINFOHEADER; the height counts the colour and the mask rows.
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(size as i32).to_le_bytes());
    out.extend_from_slice(&((size * 2) as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&[0u8; 24]);
    let stride = (size * 4) as usize;
    for row in rgba.chunks_exact(stride).rev() {
        for p in row.as_chunks::<4>().0 {
            out.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
    }
    out.extend(std::iter::repeat_n(0u8, (mask_stride * size) as usize));
    out
}

/// An RGBA PNG.
pub fn png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    assert_eq!(
        rgba.len(),
        (width * height * 4) as usize,
        "RGBA buffer size"
    );
    let stride = (width * 4) as usize;
    // Filter type 2 (Up) on every row: transparent margins and the panels'
    // vertical gradients become runs the deflater folds away.
    let mut filtered = Vec::with_capacity(rgba.len() + height as usize);
    let zero = vec![0u8; stride];
    let mut previous: &[u8] = &zero;
    for row in rgba.chunks_exact(stride) {
        filtered.push(2);
        filtered.extend(row.iter().zip(previous).map(|(c, p)| c.wrapping_sub(*p)));
        previous = row;
    }

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8 bits, RGBA, deflate, adaptive, no interlace

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib(&filtered));
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

/// Bits, least significant first, as deflate packs them.
struct Bits {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl Bits {
    fn put(&mut self, value: u32, count: u32) {
        self.acc |= (value as u64) << self.n;
        self.n += count;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }

    /// A Huffman code: stored most significant bit first.
    fn code(&mut self, code: u32, len: u32) {
        self.put(code.reverse_bits() >> (32 - len), len);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push(self.acc as u8);
        }
        self.out
    }
}

/// The fixed literal/length code of RFC 1951 §3.2.6.
fn literal(bits: &mut Bits, symbol: u32) {
    match symbol {
        0..=143 => bits.code(0x30 + symbol, 8),
        144..=255 => bits.code(0x190 + symbol - 144, 9),
        256..=279 => bits.code(symbol - 256, 7),
        _ => bits.code(0xC0 + symbol - 280, 8),
    }
}

const LENGTH_BASE: [u32; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

fn length(bits: &mut Bits, len: u32) {
    let i = LENGTH_BASE.iter().rposition(|&b| b <= len).unwrap_or(0);
    literal(bits, 257 + i as u32);
    bits.put(len - LENGTH_BASE[i], LENGTH_EXTRA[i]);
}

/// Distances 1 to 4 are codes 0 to 3 of the fixed 5-bit code, no extra bits.
fn distance(bits: &mut Bits, dist: u32) {
    debug_assert!((1..=4).contains(&dist));
    bits.code(dist - 1, 5);
}

/// zlib stream: one fixed-Huffman block whose only matches repeat the
/// previous byte or the previous pixel.
fn zlib(data: &[u8]) -> Vec<u8> {
    let mut bits = Bits {
        out: vec![0x78, 0x01],
        acc: 0,
        n: 0,
    };
    bits.put(1, 1); // final block
    bits.put(1, 2); // fixed Huffman
    let mut i = 0;
    while i < data.len() {
        let (mut best_len, mut best_dist) = (0, 0);
        for dist in [1usize, 4] {
            if i < dist {
                continue;
            }
            let max = (data.len() - i).min(258);
            let len = (0..max)
                .take_while(|&k| data[i + k] == data[i + k - dist])
                .count();
            if len > best_len {
                (best_len, best_dist) = (len, dist);
            }
        }
        if best_len >= 3 {
            length(&mut bits, best_len as u32);
            distance(&mut bits, best_dist as u32);
            i += best_len;
        } else {
            literal(&mut bits, data[i] as u32);
            i += 1;
        }
    }
    literal(&mut bits, 256); // end of block
    let mut out = bits.finish();
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn checksums_match_known_values() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn ico_directory_points_at_each_image() {
        let ico = ico(|s| crate::art::folder(s, |_| [1.0, 0.5, 0.0]));
        let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
        assert_eq!(count, SIZES.len());
        let mut end = 0;
        for i in 0..count {
            let e = 6 + 16 * i;
            let size = ico[e];
            let len = u32::from_le_bytes(ico[e + 8..e + 12].try_into().unwrap()) as usize;
            let off = u32::from_le_bytes(ico[e + 12..e + 16].try_into().unwrap()) as usize;
            if size == 0 {
                assert_eq!(&ico[off..off + 4], b"\x89PNG", "256 pixels: PNG");
            } else {
                let header = u32::from_le_bytes(ico[off..off + 4].try_into().unwrap());
                assert_eq!(header, 40, "{size} pixels: DIB");
            }
            end = end.max(off + len);
        }
        assert_eq!(end, ico.len());
    }

    #[test]
    fn png_is_small() {
        let rgba = crate::art::folder(256, |_| [0.0, 0.48, 1.0]);
        let png = png(256, 256, &rgba);
        assert!(png.len() < 60 * 1024, "{} bytes", png.len());
    }

    /// Windows' own PNG decoder (WIC) reads back exactly what we wrote.
    #[test]
    fn png_decodes_with_wic_to_the_same_pixels() {
        let (w, h) = (256, 256);
        let rgba = crate::art::spectrum(w);
        let decoded = wic::decode_rgba(&png(w, h, &rgba)).expect("WIC decodes the PNG");
        assert_eq!(decoded.0, (w, h));
        assert!(decoded.1 == rgba, "pixels differ after a round trip");
    }

    pub(crate) mod wic {
        use windows::Win32::Graphics::Imaging::*;
        use windows::Win32::System::Com::*;

        /// (size, straight RGBA) of the first frame of an encoded image.
        pub fn decode_rgba(bytes: &[u8]) -> windows::core::Result<((u32, u32), Vec<u8>)> {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                let factory: IWICImagingFactory =
                    CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
                let stream = factory.CreateStream()?;
                stream.InitializeFromMemory(bytes)?;
                let decoder = factory.CreateDecoderFromStream(
                    &stream,
                    std::ptr::null(),
                    WICDecodeMetadataCacheOnDemand,
                )?;
                let frame = decoder.GetFrame(0)?;
                let converter = factory.CreateFormatConverter()?;
                converter.Initialize(
                    &frame,
                    &GUID_WICPixelFormat32bppRGBA,
                    WICBitmapDitherTypeNone,
                    None,
                    0.0,
                    WICBitmapPaletteTypeCustom,
                )?;
                let (mut w, mut h) = (0, 0);
                converter.GetSize(&mut w, &mut h)?;
                let mut out = vec![0u8; (w * h * 4) as usize];
                converter.CopyPixels(std::ptr::null(), w * 4, &mut out)?;
                Ok(((w, h), out))
            }
        }
    }
}
