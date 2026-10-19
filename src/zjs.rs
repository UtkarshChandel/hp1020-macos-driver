//! ZjStream writer for the HP LaserJet 1018 / 1020 / 1022 (foo2zjs "model 1").
//!
//! A job is a PJL header, the big-endian "JZJZ" record stream, and a PJL trailer:
//!
//! ```text
//! UEL @PJL JOB ... @PJL SET JOBATTR="..." NUL UEL
//! JZJZ
//!   START_DOC (collate, duplex, pagecount)
//!   per page: START_PAGE (13 items)  JBIG_BIH  JBIG_BID...  END_JBIG  END_PAGE
//!   END_DOC
//! UEL @PJL EOJ UEL
//! ```
//!
//! Every record is a 16-byte header `size, type, items, reserved, "ZZ"` followed by
//! `items` 12-byte uint32 items or a raw payload. The `reserved` values (0x24 on
//! START_DOC, 0x9c on START_PAGE) are what the 1020 firmware expects.

use crate::jbig;
use std::io::{self, Write};

const UEL: &[u8] = b"\x1b%-12345X";

#[repr(u32)]
#[derive(Clone, Copy)]
enum Rec {
    StartDoc = 0,
    EndDoc = 1,
    StartPage = 2,
    EndPage = 3,
    JbigBih = 4,
    JbigBid = 5,
    EndJbig = 6,
}

#[repr(u16)]
#[derive(Clone, Copy)]
enum Item {
    PageCount = 0,
    DmCollate = 1,
    DmDuplex = 2,
    DmPaper = 3,
    DmCopies = 4,
    DmDefaultSource = 5,
    DmMediaType = 6,
    Nbie = 7,
    ResolutionX = 8,
    ResolutionY = 9,
    RasterX = 12,
    RasterY = 13,
    VideoBpp = 16,
    VideoX = 17,
    VideoY = 18,
    EconoMode = 23,
}

/// Windows DEVMODE paper codes understood by the firmware.
pub fn paper_code(name: &str, width_pt: u32, height_pt: u32) -> u32 {
    let n = name.to_ascii_lowercase();
    let by_name = match n.as_str() {
        "letter" | "na_letter_8.5x11in" => Some(1),
        "legal" | "na_legal_8.5x14in" => Some(5),
        "executive" | "na_executive_7.25x10.5in" => Some(7),
        "a4" | "iso_a4_210x297mm" => Some(9),
        "a5" | "iso_a5_148x210mm" => Some(11),
        "b5" | "jis_b5_182x257mm" => Some(13),
        _ => None,
    };
    if let Some(c) = by_name {
        return c;
    }
    let near = |w: u32, h: u32| width_pt.abs_diff(w) <= 3 && height_pt.abs_diff(h) <= 3;
    match () {
        _ if near(612, 792) => 1,
        _ if near(612, 1008) => 5,
        _ if near(522, 756) => 7,
        _ if near(420, 595) => 11,
        _ if near(516, 729) => 13,
        _ => 9,
    }
}

#[derive(Clone, Debug)]
pub struct JobOptions {
    /// 1 (lightest) ..= 5 (darkest)
    pub density: u32,
    pub economode: bool,
    /// `YYYYmmddHHMMSS`, sent as the PJL job attribute
    pub timestamp: String,
}

pub struct PageInfo {
    pub width: u32,
    pub height: u32,
    pub paper: u32,
    pub copies: u32,
    pub resolution: u32,
}

/// Payload size of each BID record; the firmware misbehaves with larger blocks.
const BID_CHUNK: usize = 65536;
/// Zero padding foo2zjs appends to the last BID record (its `ExtraPad`).
const EXTRA_PAD: usize = 16;

pub struct Writer<W: Write> {
    out: W,
    opts: JobOptions,
}

impl<W: Write> Writer<W> {
    pub fn start(mut out: W, opts: JobOptions) -> io::Result<Self> {
        out.write_all(UEL)?;
        write!(
            out,
            "@PJL JOB\n\
             @PJL SET JAMRECOVERY=OFF\n\
             @PJL SET DENSITY={}\n\
             @PJL SET ECONOMODE={}\n\
             @PJL SET RET=MEDIUM\n\
             @PJL INFO STATUS\n\
             @PJL USTATUS DEVICE = ON\n\
             @PJL USTATUS JOB = ON\n\
             @PJL USTATUS PAGE = ON\n\
             @PJL USTATUS TIMED = 30\n\
             @PJL SET JOBATTR=\"JobAttr4={}\"",
            opts.density,
            if opts.economode { "ON" } else { "OFF" },
            opts.timestamp
        )?;
        out.write_all(&[0])?;
        out.write_all(UEL)?;
        out.write_all(b"JZJZ")?;

        let mut w = Writer { out, opts };
        w.record(Rec::StartDoc, 0x24, 3, 3 * 12)?;
        w.item(Item::DmCollate, 0)?;
        w.item(Item::DmDuplex, 1)?;
        w.item(Item::PageCount, 0)?;
        Ok(w)
    }

    fn record(&mut self, rec: Rec, reserved: u16, items: u32, payload: usize) -> io::Result<()> {
        let mut h = [0u8; 16];
        h[0..4].copy_from_slice(&(16 + payload as u32).to_be_bytes());
        h[4..8].copy_from_slice(&(rec as u32).to_be_bytes());
        h[8..12].copy_from_slice(&items.to_be_bytes());
        h[12..14].copy_from_slice(&reserved.to_be_bytes());
        h[14..16].copy_from_slice(b"ZZ");
        self.out.write_all(&h)
    }

    fn item(&mut self, item: Item, value: u32) -> io::Result<()> {
        let mut b = [0u8; 12];
        b[0..4].copy_from_slice(&12u32.to_be_bytes());
        b[4..6].copy_from_slice(&(item as u16).to_be_bytes());
        b[6] = 1; // ZJIT_UINT32
        b[8..12].copy_from_slice(&value.to_be_bytes());
        self.out.write_all(&b)
    }

    /// Write one page. `bitmap` is 1 bpp, MSB first, 1 = black, rows `stride` bytes apart,
    /// at least `page.width` pixels wide.
    pub fn page(&mut self, page: &PageInfo, bitmap: &[u8], stride: usize) -> io::Result<()> {
        // The firmware wants the JBIG width padded to a multiple of 128 pixels.
        let jw = (page.width + 127) & !127;
        let jbpl = (jw / 8) as usize;
        let h = page.height as usize;
        let bpl = (page.width as usize).div_ceil(8);
        let mut padded = vec![0u8; jbpl * h];
        for y in 0..h {
            let row = &mut padded[y * jbpl..y * jbpl + bpl];
            row.copy_from_slice(&bitmap[y * stride..y * stride + bpl]);
            // no stray bits past the real width
            if page.width % 8 != 0 {
                row[bpl - 1] &= 0xffu8 << (8 - page.width % 8);
            }
        }

        let bie = jbig::encode(&padded, jw, page.height);

        self.record(Rec::StartPage, 0x9c, 13, 13 * 12)?;
        self.item(Item::EconoMode, self.opts.economode as u32)?;
        self.item(Item::VideoX, page.width)?;
        self.item(Item::VideoY, page.height)?;
        self.item(Item::VideoBpp, 1)?;
        self.item(Item::RasterX, page.width)?;
        self.item(Item::RasterY, page.height)?;
        self.item(Item::Nbie, 1)?;
        self.item(Item::ResolutionX, page.resolution)?;
        self.item(Item::ResolutionY, page.resolution)?;
        self.item(Item::DmDefaultSource, 7)?; // DMBIN_AUTO
        self.item(Item::DmCopies, page.copies)?;
        self.item(Item::DmPaper, page.paper)?;
        self.item(Item::DmMediaType, 1)?; // plain paper

        self.record(Rec::JbigBih, 0, 0, 20)?;
        self.out.write_all(&jbig::bih(jw, page.height))?;

        let chunks: Vec<&[u8]> = bie.chunks(BID_CHUNK).collect();
        for (i, chunk) in chunks.iter().enumerate() {
            let pad = if i + 1 == chunks.len() {
                EXTRA_PAD + chunk.len().next_multiple_of(4) - chunk.len()
            } else {
                0
            };
            self.record(Rec::JbigBid, 0, 0, chunk.len() + pad)?;
            self.out.write_all(chunk)?;
            self.out.write_all(&vec![0u8; pad])?;
        }
        self.record(Rec::EndJbig, 0, 0, 0)?;
        self.record(Rec::EndPage, 0, 0, 0)?;
        Ok(())
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.record(Rec::EndDoc, 0, 0, 0)?;
        self.out.write_all(UEL)?;
        self.out.write_all(b"@PJL EOJ\n")?;
        self.out.write_all(UEL)?;
        self.out.flush()?;
        Ok(self.out)
    }
}
