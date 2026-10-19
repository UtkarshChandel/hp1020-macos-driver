//! Byte-exact check against foo2zjs (jbig-kit 2.1) output for the same bitmap.
//!
//! `pattern.foo2zjs.zjs` was produced with
//! `foo2zjs -r600x600 -g1024x700 -p9 -m1 -n1 -d1 -s7 -z1 -P -L0 -T3 < pattern.pbm`
//! and its PJL timestamp pinned to 20000101000000.

use hp1020::zjs;

fn read_pbm(data: &[u8]) -> (u32, u32, &[u8]) {
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 3 {
        while data[i].is_ascii_whitespace() {
            i += 1;
        }
        let start = i;
        while !data[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(std::str::from_utf8(&data[start..i]).unwrap());
    }
    assert_eq!(fields[0], "P4");
    (fields[1].parse().unwrap(), fields[2].parse().unwrap(), &data[i + 1..])
}

#[test]
fn matches_foo2zjs() {
    let pbm = std::fs::read("tests/golden/pattern.pbm").unwrap();
    let expected = std::fs::read("tests/golden/pattern.foo2zjs.zjs").unwrap();
    let (w, h, bits) = read_pbm(&pbm);

    let opts = zjs::JobOptions { density: 3, economode: false, timestamp: "20000101000000".into() };
    let mut writer = zjs::Writer::start(Vec::new(), opts).unwrap();
    let page = zjs::PageInfo { width: w, height: h, paper: 9, copies: 1, resolution: 600 };
    writer.page(&page, bits, (w as usize).div_ceil(8)).unwrap();
    let out = writer.finish().unwrap();

    assert_eq!(out.len(), expected.len(), "stream length differs");
    if let Some(i) = out.iter().zip(&expected).position(|(a, b)| a != b) {
        panic!("first difference at byte {i}");
    }
}
