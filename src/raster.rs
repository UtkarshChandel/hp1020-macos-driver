//! CUPS raster reader (`application/vnd.cups-raster`, sync words RaSt / RaS2 / RaS3,
//! either byte order). Pages come out as 1-bit black bitmaps whatever the input depth,
//! because that is all the 1020 can print.

use std::io::{self, Read};

const HEADER_LEN: usize = 1796;

const CSPACE_W: u32 = 0;
const CSPACE_SW: u32 = 18;

#[derive(Debug, Clone)]
pub struct Header {
    pub hw_resolution: [u32; 2],
    pub page_size: [u32; 2],
    pub num_copies: u32,
    pub width: u32,
    pub height: u32,
    pub bits_per_color: u32,
    pub bits_per_pixel: u32,
    pub bytes_per_line: u32,
    pub color_space: u32,
    pub page_size_name: String,
}

pub struct Page {
    pub header: Header,
    /// 1 bpp, MSB first, 1 = black, `stride` bytes per row
    pub bitmap: Vec<u8>,
    pub stride: usize,
}

pub struct Reader<R: Read> {
    input: R,
    big_endian: bool,
    compressed: bool,
}

#[derive(Clone, Copy)]
enum Version {
    V1,
    V2,
    V3,
}

fn bad(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

impl<R: Read> Reader<R> {
    pub fn new(mut input: R) -> io::Result<Self> {
        let mut sync = [0u8; 4];
        input.read_exact(&mut sync)?;
        let (version, big_endian) = match &sync {
            b"RaSt" => (Version::V1, true),
            b"tSaR" => (Version::V1, false),
            b"RaS2" => (Version::V2, true),
            b"2SaR" => (Version::V2, false),
            b"RaS3" => (Version::V3, true),
            b"3SaR" => (Version::V3, false),
            _ => return Err(bad("not a CUPS raster stream")),
        };
        Ok(Reader { input, big_endian, compressed: matches!(version, Version::V2) })
    }

    fn u32_at(&self, h: &[u8], off: usize) -> u32 {
        let b: [u8; 4] = h[off..off + 4].try_into().unwrap();
        if self.big_endian { u32::from_be_bytes(b) } else { u32::from_le_bytes(b) }
    }

    /// Next page, or `None` at end of stream.
    pub fn next_page(&mut self) -> io::Result<Option<Page>> {
        let mut h = vec![0u8; HEADER_LEN];
        match read_full(&mut self.input, &mut h)? {
            0 => return Ok(None),
            n if n < HEADER_LEN => return Err(bad("truncated raster page header")),
            _ => {}
        }
        let name = &h[HEADER_LEN - 64..];
        let header = Header {
            hw_resolution: [self.u32_at(&h, 276), self.u32_at(&h, 280)],
            page_size: [self.u32_at(&h, 352), self.u32_at(&h, 356)],
            num_copies: self.u32_at(&h, 340),
            width: self.u32_at(&h, 372),
            height: self.u32_at(&h, 376),
            bits_per_color: self.u32_at(&h, 384),
            bits_per_pixel: self.u32_at(&h, 388),
            bytes_per_line: self.u32_at(&h, 392),
            color_space: self.u32_at(&h, 400),
            page_size_name: String::from_utf8_lossy(&name[..name.iter().position(|&b| b == 0).unwrap_or(64)])
                .into_owned(),
        };
        if header.width == 0 || header.height == 0 || header.width > 20000 || header.height > 30000 {
            return Err(bad("implausible raster page size"));
        }
        if header.bytes_per_line < (header.width * header.bits_per_pixel).div_ceil(8) {
            return Err(bad("bytes_per_line too small"));
        }

        let src_bpl = header.bytes_per_line as usize;
        let stride = (header.width as usize).div_ceil(8);
        let mut bitmap = vec![0u8; stride * header.height as usize];
        let mut row = vec![0u8; src_bpl];
        let mut y = 0usize;
        while y < header.height as usize {
            let repeat = if self.compressed { self.read_compressed_line(&header, &mut row)? } else {
                self.input.read_exact(&mut row)?;
                1
            };
            for _ in 0..repeat {
                if y >= header.height as usize {
                    break;
                }
                to_bilevel(&header, &row, &mut bitmap[y * stride..(y + 1) * stride], y);
                y += 1;
            }
        }
        Ok(Some(Page { header, bitmap, stride }))
    }

    /// CUPS v2 PackBits-style line: repeat count, then runs of whole pixels.
    fn read_compressed_line(&mut self, header: &Header, row: &mut [u8]) -> io::Result<usize> {
        let mut b = [0u8; 1];
        self.input.read_exact(&mut b)?;
        let repeat = b[0] as usize + 1;
        let unit = (header.bits_per_pixel as usize).div_ceil(8).max(1);
        let white = if matches!(header.color_space, CSPACE_W | CSPACE_SW) { 0xff } else { 0x00 };
        let mut pos = 0;
        while pos < row.len() {
            self.input.read_exact(&mut b)?;
            let n = b[0];
            if n == 128 {
                row[pos..].fill(white);
                break;
            } else if n & 128 != 0 {
                let len = ((257 - n as usize) * unit).min(row.len() - pos);
                self.input.read_exact(&mut row[pos..pos + len])?;
                pos += len;
            } else {
                let count = n as usize + 1;
                let mut px = vec![0u8; unit];
                self.input.read_exact(&mut px)?;
                for _ in 0..count {
                    if pos >= row.len() {
                        break;
                    }
                    let len = unit.min(row.len() - pos);
                    row[pos..pos + len].copy_from_slice(&px[..len]);
                    pos += len;
                }
            }
        }
        Ok(repeat)
    }
}

fn read_full<R: Read>(r: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(n)
}

#[rustfmt::skip]
const BAYER8: [[u8; 8]; 8] = [
    [ 0, 32,  8, 40,  2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44,  4, 36, 14, 46,  6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [ 3, 35, 11, 43,  1, 33,  9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47,  7, 39, 13, 45,  5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

/// Convert one raster row to 1 bpp black. 1-bit K rows pass through; anything deeper
/// is reduced to an ink level and ordered-dithered.
fn to_bilevel(h: &Header, src: &[u8], dst: &mut [u8], y: usize) {
    if h.bits_per_pixel == 1 {
        let n = dst.len();
        dst.copy_from_slice(&src[..n]);
        if matches!(h.color_space, CSPACE_W | CSPACE_SW) {
            dst.iter_mut().for_each(|b| *b = !*b);
        }
        return;
    }
    // W, RGB, RGBA, sW, sRGB, AdobeRGB store light, everything else stores ink
    let additive = matches!(h.color_space, 0 | 1 | 2 | 18 | 19 | 20);
    dst.fill(0);
    for x in 0..h.width as usize {
        let ink = ink_level(h, src, x, additive);
        let threshold = BAYER8[y % 8][x % 8] as u32 * 4 + 2;
        if ink as u32 > threshold {
            dst[x / 8] |= 0x80 >> (x % 8);
        }
    }
}

/// Ink coverage 0 (white) ..= 255 (black) of pixel `x`.
fn ink_level(h: &Header, src: &[u8], x: usize, additive: bool) -> u8 {
    let bpp = h.bits_per_pixel as usize;
    let v: u8 = match bpp {
        2 | 4 => {
            let bit = x * bpp;
            let raw = (src[bit / 8] >> (8 - bpp - bit % 8)) & ((1 << bpp) - 1);
            (raw as u32 * 255 / ((1 << bpp) - 1)) as u8
        }
        8 => src[x],
        16 => src[x * 2],
        24 | 32 if additive => {
            let p = &src[x * (bpp / 8)..];
            ((p[0] as u32 * 30 + p[1] as u32 * 59 + p[2] as u32 * 11) / 100) as u8
        }
        _ => src[x * (bpp / 8)],
    };
    if additive { 255 - v } else { v }
}
