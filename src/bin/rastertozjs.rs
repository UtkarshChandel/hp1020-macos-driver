//! CUPS filter: application/vnd.cups-raster -> HP ZjStream.
//!
//! Usage (CUPS): rastertozjs job user title copies options [file]

use hp1020::{raster, zjs};
use std::io::{self, BufReader, BufWriter, Read};

fn option<'a>(options: &'a str, key: &str) -> Option<&'a str> {
    options.split_whitespace().find_map(|kv| {
        let (k, v) = kv.split_once('=').unwrap_or((kv, "true"));
        k.eq_ignore_ascii_case(key).then_some(v)
    })
}

fn run() -> io::Result<u32> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 6 || args.len() > 7 {
        eprintln!("Usage: rastertozjs job user title copies options [file]");
        std::process::exit(1);
    }
    let options = args[5].as_str();
    let density = option(options, "PrintDensity").and_then(|v| v.parse().ok()).unwrap_or(3u32).clamp(1, 5);
    let economode = option(options, "EconoMode").is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "true" | "on" | "yes"));

    let input: Box<dyn Read> = match args.get(6) {
        Some(path) => Box::new(std::fs::File::open(path)?),
        None => Box::new(io::stdin().lock()),
    };
    let mut reader = raster::Reader::new(BufReader::with_capacity(1 << 20, input))?;
    let out = BufWriter::with_capacity(1 << 20, io::stdout().lock());
    let mut writer = zjs::Writer::start(out, zjs::JobOptions { density, economode, timestamp: hp1020::timestamp() })?;

    let mut pages = 0;
    while let Some(page) = reader.next_page()? {
        let h = &page.header;
        if h.hw_resolution[0] != 600 || h.hw_resolution[1] != 600 {
            eprintln!("WARNING: raster is {}x{} dpi, the printer expects 600x600", h.hw_resolution[0], h.hw_resolution[1]);
        }
        pages += 1;
        eprintln!("INFO: Encoding page {pages} ({}x{}, {})", h.width, h.height, h.page_size_name);
        writer.page(
            &zjs::PageInfo {
                width: h.width,
                height: h.height,
                paper: zjs::paper_code(&h.page_size_name, h.page_size[0], h.page_size[1]),
                copies: 1,
                resolution: h.hw_resolution[0],
            },
            &page.bitmap,
            page.stride,
        )?;
        eprintln!("PAGE: {pages} 1");
    }
    writer.finish()?;
    Ok(pages)
}

fn main() {
    match run() {
        Ok(0) => eprintln!("WARNING: no pages in raster stream"),
        Ok(n) => eprintln!("INFO: rastertozjs: {n} page(s)"),
        Err(e) => {
            eprintln!("ERROR: rastertozjs: {e}");
            std::process::exit(1);
        }
    }
}
