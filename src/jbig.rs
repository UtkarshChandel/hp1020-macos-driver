//! Minimal JBIG1 (ITU-T T.82) encoder: exactly the subset the LaserJet 1018/1020
//! ZjStream firmware consumes, and byte-identical to jbig-kit 2.1 (as used by foo2zjs)
//! for these parameters:
//!
//! * single plane, single resolution layer (D = 0)
//! * two-line template (LRLTWO), typical prediction (TPBON)
//! * L0 = 128 lines per stripe, no adaptive-template moves (MX = 0)
//! * stripes terminated by SDNORM, arithmetic-coder state carried across stripes
//!
//! Input is a packed 1-bit bitmap, MSB first, 1 = black, `width` a multiple of 8.

/// Lines per stripe. foo2zjs uses 128 for every ZjStream model.
pub const L0: u32 = 128;

const TPB2CX: usize = 0x195;
const MARKER_ESC: u8 = 0xff;
const MARKER_STUFF: u8 = 0x00;
const MARKER_SDNORM: u8 = 0x02;

// BIH order/options bytes as foo2zjs emits them (JBG_ILEAVE | JBG_SMID and
// LRLTWO | TPDON | TPBON | DPON; TPDON/DPON are inert with a single layer).
const ORDER: u8 = 0x03;
const OPTIONS: u8 = 0x5c;

#[rustfmt::skip]
static LSZ: [u16; 113] = [
    0x5a1d, 0x2586, 0x1114, 0x080b, 0x03d8, 0x01da, 0x00e5, 0x006f,
    0x0036, 0x001a, 0x000d, 0x0006, 0x0003, 0x0001, 0x5a7f, 0x3f25,
    0x2cf2, 0x207c, 0x17b9, 0x1182, 0x0cef, 0x09a1, 0x072f, 0x055c,
    0x0406, 0x0303, 0x0240, 0x01b1, 0x0144, 0x00f5, 0x00b7, 0x008a,
    0x0068, 0x004e, 0x003b, 0x002c, 0x5ae1, 0x484c, 0x3a0d, 0x2ef1,
    0x261f, 0x1f33, 0x19a8, 0x1518, 0x1177, 0x0e74, 0x0bfb, 0x09f8,
    0x0861, 0x0706, 0x05cd, 0x04de, 0x040f, 0x0363, 0x02d4, 0x025c,
    0x01f8, 0x01a4, 0x0160, 0x0125, 0x00f6, 0x00cb, 0x00ab, 0x008f,
    0x5b12, 0x4d04, 0x412c, 0x37d8, 0x2fe8, 0x293c, 0x2379, 0x1edf,
    0x1aa9, 0x174e, 0x1424, 0x119c, 0x0f6b, 0x0d51, 0x0bb6, 0x0a40,
    0x5832, 0x4d1c, 0x438e, 0x3bdd, 0x34ee, 0x2eae, 0x299a, 0x2516,
    0x5570, 0x4ca9, 0x44d9, 0x3e22, 0x3824, 0x32b4, 0x2e17, 0x56a8,
    0x4f46, 0x47e5, 0x41cf, 0x3c3d, 0x375e, 0x5231, 0x4c0f, 0x4639,
    0x415e, 0x5627, 0x50e7, 0x4b85, 0x5597, 0x504f, 0x5a10, 0x5522,
    0x59eb,
];

#[rustfmt::skip]
static NMPS: [u8; 113] = [
      1,   2,   3,   4,   5,   6,   7,   8,   9,  10,  11,  12,  13,  13,  15,  16,
     17,  18,  19,  20,  21,  22,  23,  24,  25,  26,  27,  28,  29,  30,  31,  32,
     33,  34,  35,   9,  37,  38,  39,  40,  41,  42,  43,  44,  45,  46,  47,  48,
     49,  50,  51,  52,  53,  54,  55,  56,  57,  58,  59,  60,  61,  62,  63,  32,
     65,  66,  67,  68,  69,  70,  71,  72,  73,  74,  75,  76,  77,  78,  79,  48,
     81,  82,  83,  84,  85,  86,  87,  71,  89,  90,  91,  92,  93,  94,  86,  96,
     97,  98,  99, 100,  93, 102, 103, 104,  99, 106, 107, 103, 109, 107, 111, 109,
    111,
];

// Bit 7 set = swap MPS/LPS sense.
#[rustfmt::skip]
static NLPS: [u8; 113] = [
    129,  14,  16,  18,  20,  23,  25,  28,  30,  33,  35,   9,  10,  12, 143,  36,
     38,  39,  40,  42,  43,  45,  46,  48,  49,  51,  52,  54,  56,  57,  59,  60,
     62,  63,  32,  33, 165,  64,  65,  67,  68,  69,  70,  72,  73,  74,  75,  77,
     78,  79,  48,  50,  50,  51,  52,  53,  54,  55,  56,  57,  58,  59,  61,  61,
    193,  80,  81,  82,  83,  84,  86,  87,  87,  72,  72,  74,  74,  75,  77,  77,
    208,  88,  89,  90,  91,  92,  93,  86, 216,  95,  96,  97,  99,  99,  93, 223,
    101, 102, 103, 104,  99, 105, 106, 107, 103, 233, 108, 109, 110, 111, 238, 112,
    240,
];

/// QM-coder state (T.82 section 6.8), a direct port of jbig-kit's `jbg_arenc_state`.
struct ArithEncoder {
    st: [u8; 4096],
    c: u64,
    a: u64,
    sc: u32,
    ct: i32,
    buffer: i32,
}

impl ArithEncoder {
    fn new() -> Self {
        ArithEncoder { st: [0; 4096], c: 0, a: 0x10000, sc: 0, ct: 11, buffer: -1 }
    }

    /// Restart the coder for a new stripe; probability states are kept unless `reset`.
    fn init(&mut self, reset: bool) {
        if reset {
            self.st = [0; 4096];
        }
        self.c = 0;
        self.a = 0x10000;
        self.sc = 0;
        self.ct = 11;
        self.buffer = -1;
    }

    fn encode(&mut self, out: &mut Vec<u8>, cx: usize, pix: u8) {
        let st = self.st[cx];
        let ss = (st & 0x7f) as usize;
        let lsz = LSZ[ss] as u64;

        if ((pix << 7) ^ st) & 0x80 != 0 {
            // less probable symbol
            self.a -= lsz;
            if self.a >= lsz {
                self.c += self.a;
                self.a = lsz;
            }
            self.st[cx] = (st & 0x80) ^ NLPS[ss];
        } else {
            // more probable symbol
            self.a -= lsz;
            if self.a & 0xffff_8000 != 0 {
                return;
            }
            if self.a < lsz {
                self.c += self.a;
                self.a = lsz;
            }
            self.st[cx] = (st & 0x80) | NMPS[ss];
        }

        // renormalisation
        loop {
            self.a <<= 1;
            self.c <<= 1;
            self.ct -= 1;
            if self.ct == 0 {
                let temp = self.c >> 19;
                if temp & 0xffff_ff00 != 0 {
                    // carry into the buffered byte and any pending 0xff run
                    if self.buffer >= 0 {
                        self.buffer += 1;
                        out.push(self.buffer as u8);
                        if self.buffer as u8 == MARKER_ESC {
                            out.push(MARKER_STUFF);
                        }
                    }
                    for _ in 0..self.sc {
                        out.push(0x00);
                    }
                    self.sc = 0;
                    self.buffer = (temp & 0xff) as i32;
                } else if temp == 0xff {
                    self.sc += 1;
                } else {
                    if self.buffer >= 0 {
                        out.push(self.buffer as u8);
                    }
                    for _ in 0..self.sc {
                        out.push(0xff);
                        out.push(MARKER_STUFF);
                    }
                    self.sc = 0;
                    self.buffer = temp as i32;
                }
                self.c &= 0x7ffff;
                self.ct = 8;
            }
            if self.a >= 0x8000 {
                break;
            }
        }
    }

    fn flush(&mut self, out: &mut Vec<u8>) {
        // pick the value in the coding interval with the most trailing zero bits
        let temp = (self.a - 1 + self.c) & 0xffff_0000;
        self.c = if temp < self.c { temp + 0x8000 } else { temp };
        self.c <<= self.ct;
        if self.c & 0xf800_0000 != 0 {
            // one final carry
            if self.buffer >= 0 {
                let b = (self.buffer + 1) as u8;
                out.push(b);
                if b == MARKER_ESC {
                    out.push(MARKER_STUFF);
                }
            }
            if self.c & 0x7fff800 != 0 {
                for _ in 0..self.sc {
                    out.push(0x00);
                }
                self.sc = 0;
            }
        } else {
            if self.buffer >= 0 {
                out.push(self.buffer as u8);
            }
            for _ in 0..self.sc {
                out.push(0xff);
                out.push(MARKER_STUFF);
            }
            self.sc = 0;
        }
        if self.c & 0x7fff800 != 0 {
            let b = ((self.c >> 19) & 0xff) as u8;
            out.push(b);
            if b == MARKER_ESC {
                out.push(MARKER_STUFF);
            }
            if self.c & 0x7f800 != 0 {
                let b = ((self.c >> 11) & 0xff) as u8;
                out.push(b);
                if b == MARKER_ESC {
                    out.push(MARKER_STUFF);
                }
            }
        }
    }
}

/// The 20-byte Bi-level Image Header.
pub fn bih(width: u32, height: u32) -> [u8; 20] {
    let mut b = [0u8; 20];
    b[2] = 1; // planes
    b[4..8].copy_from_slice(&width.to_be_bytes());
    b[8..12].copy_from_slice(&height.to_be_bytes());
    b[12..16].copy_from_slice(&L0.to_be_bytes());
    b[18] = ORDER;
    b[19] = OPTIONS;
    b
}

/// Encode a bitmap into a Bi-level Image Data stream (all stripes, without the BIH).
pub fn encode(bitmap: &[u8], width: u32, height: u32) -> Vec<u8> {
    assert!(width % 8 == 0, "width must be byte aligned");
    let bpl = (width / 8) as usize;
    let h = height as usize;
    assert!(bitmap.len() >= bpl * h, "bitmap too small");
    let line = |y: usize| &bitmap[y * bpl..(y + 1) * bpl];

    let mut out = Vec::with_capacity(bpl * h / 16);
    let mut ar = ArithEncoder::new();
    let stripes = h.div_ceil(L0 as usize);

    for stripe in 0..stripes {
        let reset = stripe == 0;
        ar.init(reset);
        let start = out.len();
        let mut y = stripe * L0 as usize;

        // typical-prediction state carried in from the previous stripe
        let mut ltp_old = if reset {
            false
        } else if y > 1 {
            line(y - 1) == line(y - 2)
        } else {
            line(y - 1).iter().all(|&b| b == 0)
        };

        for i in 0..L0 as usize {
            if y >= h {
                break;
            }
            let have_prev = i > 0 || !reset;
            let cur = line(y);

            // TPB: a line identical to the one above is coded as a single "typical" bit
            let ltp = if have_prev { cur == line(y - 1) } else { cur.iter().all(|&b| b == 0) };
            ar.encode(&mut out, TPB2CX, (ltp == ltp_old) as u8);
            ltp_old = ltp;
            if ltp {
                y += 1;
                continue;
            }

            let prev = if have_prev { Some(line(y - 1)) } else { None };
            let mut line_h1: u64 = 0;
            let mut line_h2: u64 = prev.map_or(0, |p| (p[0] as u64) << 8);
            for k in 0..bpl {
                line_h1 |= cur[k] as u64;
                if let Some(p) = prev {
                    if k + 1 < bpl {
                        line_h2 |= p[k + 1] as u64;
                    }
                }
                for _ in 0..8 {
                    line_h1 <<= 1;
                    line_h2 <<= 1;
                    let cx = (((line_h2 >> 10) & 0x3f0) | ((line_h1 >> 9) & 0x00f)) as usize;
                    ar.encode(&mut out, cx, ((line_h1 >> 8) & 1) as u8);
                }
            }
            y += 1;
        }

        ar.flush(&mut out);
        // jbg_buf_remove_zeros: drop trailing 0x00, but keep the stuffing after a final 0xff
        while out.len() > start && out[out.len() - 1] == 0 {
            out.pop();
        }
        if out.len() > start && out[out.len() - 1] == MARKER_ESC {
            out.push(MARKER_STUFF);
        }
        out.push(MARKER_ESC);
        out.push(MARKER_SDNORM);
    }
    out
}
