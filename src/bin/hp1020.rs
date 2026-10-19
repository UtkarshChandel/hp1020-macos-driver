//! CUPS backend `hp1020://` for the HP LaserJet 1018 / 1020 / 1020 Plus.
//!
//! These printers have no firmware in flash: after every power-on the host has to
//! upload it before the first job. The backend reads the IEEE-1284 Device ID, and if
//! there is no `FWVER` field it sends the firmware blob, waits for the printer to boot,
//! and only then streams the job to the bulk-OUT endpoint.
//!
//! Firmware boot takes ~30 s, and USB requests that arrive during it are lost or wedge
//! the printer until it is power-cycled, so after an upload we sleep before touching
//! the device again.
//!
//! Invocations:
//!   hp1020                                     CUPS device discovery
//!   hp1020 job user title copies options [f]   CUPS print job (DEVICE_URI in env)
//!   hp1020 status | load-firmware [fw] | send <file>   manual tools

use nusb::transfer::{Bulk, ControlIn, ControlType, Out, Recipient};
use nusb::MaybeFuture;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const HP_VID: u16 = 0x03f0;
const FIRMWARE_DIR: &str = "/Library/Printers/hp1020/firmware";
/// How long the printer needs to boot an uploaded firmware before it answers again.
const FW_BOOT: Duration = Duration::from_secs(35);

struct Model {
    pid: u16,
    name: &'static str,
    firmware: &'static str,
}

const MODELS: &[Model] = &[
    Model { pid: 0x2b17, name: "HP LaserJet 1020", firmware: "hp_laserjet_1020.fw" },
    Model { pid: 0x4117, name: "HP LaserJet 1018", firmware: "hp_laserjet_1018.fw" },
];

// CUPS backend exit codes
const CUPS_BACKEND_OK: i32 = 0;
const CUPS_BACKEND_FAILED: i32 = 1;
const CUPS_BACKEND_STOP: i32 = 4;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

macro_rules! log {
    ($lvl:literal, $($arg:tt)*) => { eprintln!(concat!($lvl, ": hp1020: {}"), format!($($arg)*)) };
}

struct Printer {
    model: &'static Model,
    intf: nusb::Interface,
    _dev: nusb::Device,
}

fn model_for(pid: u16) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.pid == pid)
}

fn find(serial: Option<&str>) -> Result<Option<nusb::DeviceInfo>> {
    Ok(nusb::list_devices().wait()?.find(|d| {
        d.vendor_id() == HP_VID
            && model_for(d.product_id()).is_some()
            && serial.is_none_or(|s| d.serial_number() == Some(s))
    }))
}

fn open(info: &nusb::DeviceInfo) -> Result<Printer> {
    let model = model_for(info.product_id()).ok_or("unsupported model")?;
    let dev = info.open().wait()?;
    let intf = dev.claim_interface(0).wait()?;
    Ok(Printer { model, intf, _dev: dev })
}

impl Printer {
    fn device_id(&self, timeout: Duration) -> Result<String> {
        let id = self
            .intf
            .control_in(
                ControlIn {
                    control_type: ControlType::Class,
                    recipient: Recipient::Interface,
                    request: 0, // GET_DEVICE_ID
                    value: 0,
                    index: 0,
                    length: 1023,
                },
                timeout,
            )
            .wait()?;
        Ok(String::from_utf8_lossy(id.get(2..).unwrap_or_default()).into_owned())
    }

    fn writer(&self) -> Result<nusb::io::EndpointWrite<Bulk>> {
        let mut w = self.intf.endpoint::<Bulk, Out>(0x01)?.writer(64 * 1024);
        // The printer stops accepting data while it is busy (warming up, out of
        // paper, door open), so writes may legitimately block for a long time.
        w.set_write_timeout(Duration::from_secs(600));
        Ok(w)
    }

    fn send(&self, mut input: impl Read) -> Result<u64> {
        let mut w = self.writer()?;
        let n = io::copy(&mut input, &mut w)?;
        w.flush()?;
        Ok(n)
    }
}

fn firmware_path(model: &Model) -> Option<PathBuf> {
    let mut candidates = vec![];
    if let Ok(p) = std::env::var("HP1020_FIRMWARE") {
        candidates.push(PathBuf::from(p));
    }
    candidates.push(PathBuf::from(FIRMWARE_DIR).join(model.firmware));
    if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
        candidates.push(dir.join(model.firmware));
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// Make sure the printer is running its firmware, uploading it if needed.
/// Returns the (possibly re-opened) printer.
fn ensure_firmware(p: Printer, info: &nusb::DeviceInfo, fw_override: Option<PathBuf>) -> Result<Printer> {
    let id = p.device_id(Duration::from_secs(5))?;
    log!("DEBUG", "device id: {id}");
    if id.contains("FWVER:") && fw_override.is_none() {
        return Ok(p);
    }
    let path = fw_override.or_else(|| firmware_path(p.model)).ok_or_else(|| {
        format!("firmware {} not found in {FIRMWARE_DIR}; run: sudo /Library/Printers/hp1020/fetch-firmware.sh", p.model.firmware)
    })?;
    let fw = std::fs::read(&path)?;
    eprintln!("STATE: +connecting-to-device");
    log!("INFO", "Loading printer firmware {} ({} bytes)", path.display(), fw.len());
    p.send(&fw[..])?;
    drop(p);

    log!("INFO", "Waiting for the printer to start its firmware");
    std::thread::sleep(FW_BOOT);

    let p = open(info)?;
    let started = Instant::now();
    loop {
        match p.device_id(Duration::from_secs(10)) {
            Ok(id) if id.contains("FWVER:") => {
                log!("INFO", "Firmware running ({})", fwver(&id).unwrap_or("?"));
                eprintln!("STATE: -connecting-to-device");
                return Ok(p);
            }
            Ok(id) => log!("DEBUG", "still booting: {id}"),
            Err(e) => log!("DEBUG", "device id failed: {e}"),
        }
        if started.elapsed() > Duration::from_secs(60) {
            return Err("printer did not come up after firmware upload; power-cycle it and retry".into());
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}

fn fwver(id: &str) -> Option<&str> {
    id.split(';').find_map(|f| f.strip_prefix("FWVER:"))
}

fn uri_serial(uri: &str) -> Option<String> {
    let q = uri.split_once('?')?.1;
    q.split('&').find_map(|kv| kv.strip_prefix("serial=")).map(|s| s.to_string())
}

fn discover() -> Result<()> {
    for d in nusb::list_devices().wait()? {
        let Some(m) = (d.vendor_id() == HP_VID).then(|| model_for(d.product_id())).flatten() else { continue };
        let serial = d.serial_number().unwrap_or("");
        let mdl = m.name.trim_start_matches("HP ");
        println!(
            "direct hp1020://HP/{}?serial={} \"{}\" \"{} (native ZJS)\" \"MFG:Hewlett-Packard;MDL:{};CMD:ACL;CLS:PRINTER;DES:{};\" \"\"",
            mdl.replace(' ', "%20"),
            serial,
            m.name,
            m.name,
            m.name,
            m.name
        );
    }
    Ok(())
}

/// Wait (like the stock usb backend) until the printer shows up and can be claimed.
fn wait_for_printer(serial: Option<&str>) -> (nusb::DeviceInfo, Printer) {
    let mut announced = false;
    loop {
        match find(serial) {
            Ok(Some(info)) => match open(&info) {
                Ok(p) => {
                    if announced {
                        eprintln!("STATE: -offline-report");
                    }
                    return (info, p);
                }
                Err(e) => log!("DEBUG", "open failed: {e}"),
            },
            Ok(None) => {}
            Err(e) => log!("DEBUG", "enumeration failed: {e}"),
        }
        if !announced {
            eprintln!("STATE: +offline-report");
            log!("INFO", "Waiting for printer to become available");
            announced = true;
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}

fn print_job(args: &[String]) -> i32 {
    let uri = std::env::var("DEVICE_URI").unwrap_or_default();
    let serial = uri_serial(&uri);
    let input: Box<dyn Read> = match args.get(6) {
        Some(path) => match std::fs::File::open(path) {
            Ok(f) => Box::new(f),
            Err(e) => {
                log!("ERROR", "cannot open {path}: {e}");
                return CUPS_BACKEND_FAILED;
            }
        },
        None => Box::new(io::stdin().lock()),
    };

    let (info, p) = wait_for_printer(serial.as_deref());
    let p = match ensure_firmware(p, &info, None) {
        Ok(p) => p,
        Err(e) => {
            log!("ERROR", "{e}");
            return CUPS_BACKEND_STOP;
        }
    };
    log!("INFO", "Sending print data");
    match p.send(input) {
        Ok(n) => {
            log!("INFO", "Sent {n} bytes");
            CUPS_BACKEND_OK
        }
        Err(e) => {
            log!("ERROR", "write failed: {e}");
            CUPS_BACKEND_FAILED
        }
    }
}

fn tool(cmd: &str, arg: Option<&str>) -> Result<()> {
    let info = find(None)?.ok_or("no supported HP LaserJet found on USB")?;
    let p = open(&info)?;
    match cmd {
        "status" => {
            let id = p.device_id(Duration::from_secs(5))?;
            println!("{} serial {}", p.model.name, info.serial_number().unwrap_or("?"));
            println!("device id: {id}");
            match fwver(&id) {
                Some(v) => println!("firmware: running ({v})"),
                None => println!("firmware: not loaded"),
            }
        }
        "load-firmware" => {
            let p = ensure_firmware(p, &info, arg.map(PathBuf::from))?;
            println!("{}", p.device_id(Duration::from_secs(5))?);
        }
        "send" => {
            let path = arg.ok_or("usage: hp1020 send <file>")?;
            let p = ensure_firmware(p, &info, None)?;
            let n = p.send(std::fs::File::open(path)?)?;
            println!("sent {n} bytes");
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let code = match args.len() {
        1 => match discover() {
            Ok(()) => CUPS_BACKEND_OK,
            Err(e) => {
                log!("ERROR", "{e}");
                CUPS_BACKEND_FAILED
            }
        },
        2 | 3 if matches!(args[1].as_str(), "status" | "load-firmware" | "send") => {
            match tool(&args[1], args.get(2).map(String::as_str)) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("hp1020: {e}");
                    1
                }
            }
        }
        6 | 7 => print_job(&args),
        _ => {
            eprintln!("Usage: hp1020 job-id user title copies options [file]");
            eprintln!("       hp1020 status | load-firmware [firmware.fw] | send <file.zjs>");
            CUPS_BACKEND_FAILED
        }
    };
    std::process::exit(code);
}
