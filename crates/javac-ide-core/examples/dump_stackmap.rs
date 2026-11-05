//! Dump StackMapTable frames for byte-level comparison.
//! Usage: cargo run --example dump_stackmap -- <class-file>

use std::collections::HashMap;
use std::env;
use std::fs;

fn u2(b: &[u8], p: usize) -> u16 {
    u16::from_be_bytes([b[p], b[p + 1]])
}
fn u4(b: &[u8], p: usize) -> u32 {
    u32::from_be_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]])
}

fn main() {
    let path = env::args().nth(1).expect("usage: dump_stackmap <class-file>");
    let data = fs::read(&path).expect("read class file");
    let mut pos = 8usize;
    let cp_count = u2(&data, pos) as usize;
    pos += 2;
    let mut cp_utf8: Vec<String> = vec![String::new(); cp_count];
    let mut cp_tag: Vec<u8> = vec![0; cp_count];
    let mut i = 1usize;
    while i < cp_count {
        let tag = data[pos];
        cp_tag[i] = tag;
        pos += 1;
        match tag {
            1 => {
                let len = u2(&data, pos) as usize;
                pos += 2;
                let s = String::from_utf8_lossy(&data[pos..pos + len]).into_owned();
                cp_utf8[i] = s;
                pos += len;
            }
            7 | 8 | 16 | 19 | 20 => pos += 2,
            15 => pos += 3,
            3 | 4 => pos += 4,
            5 | 6 => {
                pos += 8;
                i += 1;
            }
            9 | 10 | 11 | 12 | 17 | 18 => pos += 4,
            other => panic!("unknown cp tag {other}"),
        }
        i += 1;
    }
    let name_of = |idx: usize| -> &str { &cp_utf8[idx] };
    pos += 6;
    let ifaces = u2(&data, pos) as usize;
    pos += 2 + ifaces * 2;
    let fields = u2(&data, pos) as usize;
    pos += 2;
    for _ in 0..fields {
        pos += 6;
        let ac = u2(&data, pos) as usize;
        pos += 2;
        for _ in 0..ac {
            pos += 2;
            let len = u4(&data, pos) as usize;
            pos += 4 + len;
        }
    }
    let methods = u2(&data, pos) as usize;
    pos += 2;
    for _ in 0..methods {
        pos += 2;
        let name_idx = u2(&data, pos) as usize;
        pos += 2;
        let desc_idx = u2(&data, pos) as usize;
        pos += 2;
        let mname = name_of(name_idx).to_string();
        let mdesc = name_of(desc_idx).to_string();
        let ac = u2(&data, pos) as usize;
        pos += 2;
        for _ in 0..ac {
            let attr_idx = u2(&data, pos) as usize;
            pos += 2;
            let len = u4(&data, pos) as usize;
            pos += 4;
            let body = &data[pos..pos + len];
            pos += len;
            if cp_utf8[attr_idx] == "Code" {
                dump_code(body, &mname, &mdesc, &cp_utf8);
                dump_code_attrs(body, &mname, &mdesc, &cp_utf8);
            }
        }
    }
}


fn dump_code_attrs(body: &[u8], mname: &str, mdesc: &str, cp: &[String]) {
    let mut p = 4usize;
    let code_len = u4(body, p) as usize;
    p += 4 + code_len;
    let exc = u2(body, p) as usize;
    p += 2 + exc * 8;
    let ac = u2(body, p) as usize;
    p += 2;
    for _ in 0..ac {
        let nidx = u2(body, p) as usize;
        p += 2;
        let len = u4(body, p) as usize;
        p += 4;
        let attr = &body[p..p + len];
        p += len;
        let mut hex = String::new();
        for b in attr {
            hex.push_str(&format!("{:02x}", b));
        }
        println!("ATTR {} {} {} {}", mname, mdesc, cp[nidx], hex);
    }
}
fn dump_code(body: &[u8], mname: &str, mdesc: &str, cp: &[String]) {
    let mut p = 4usize;
    let code_len = u4(body, p) as usize;
    p += 4 + code_len;
    let exc = u2(body, p) as usize;
    p += 2 + exc * 8;
    let ac = u2(body, p) as usize;
    p += 2;
    for _ in 0..ac {
        let nidx = u2(body, p) as usize;
        p += 2;
        let len = u4(body, p) as usize;
        p += 4;
        let attr = &body[p..p + len];
        p += len;
        if cp[nidx] == "StackMapTable" {
            if let Some(frames) = decode_frames(attr) {
                println!("{}{} frames={:?}", mname, mdesc, frames);
            }
        }
    }
}

fn decode_frames(attr: &[u8]) -> Option<Vec<(u16, String, Vec<u16>)>> {
    if attr.len() < 2 {
        return None;
    }
    let n = u2(attr, 0) as usize;
    if n > 500 {
        return None;
    }
    let mut p = 2usize;
    let mut out = Vec::new();
    let mut pc = 0u16;
    for _ in 0..n {
        if p >= attr.len() {
            return None;
        }
        let ft = attr[p] as u16;
        p += 1;
        match ft {
            0..=63 => {
                pc += ft;
                out.push((pc, "same".into(), vec![]));
            }
            64..=127 => {
                if p + 2 > attr.len() {
                    return None;
                }
                pc += ft - 64;
                let t = u2(attr, p);
                p += 2;
                out.push((pc, "stack1".into(), vec![t]));
            }
            247 => {
                if p + 4 > attr.len() {
                    return None;
                }
                pc += u2(attr, p);
                p += 2;
                let t = u2(attr, p);
                p += 2;
                out.push((pc, "stack1ext".into(), vec![t]));
            }
            248..=250 => {
                if p + 2 > attr.len() {
                    return None;
                }
                pc += u2(attr, p);
                p += 2;
                out.push((pc, "chop".into(), vec![]));
            }
            251 => {
                if p + 2 > attr.len() {
                    return None;
                }
                pc += u2(attr, p);
                p += 2;
                out.push((pc, "sameext".into(), vec![]));
            }
            252..=254 => {
                let k = ft - 251;
                if p + 2 + 2 * k as usize > attr.len() {
                    return None;
                }
                pc += u2(attr, p);
                p += 2;
                let mut types = Vec::new();
                for _ in 0..k {
                    types.push(u2(attr, p));
                    p += 2;
                }
                out.push((pc, format!("append{k}"), types));
            }
            255 => {
                if p + 4 > attr.len() {
                    return None;
                }
                pc += u2(attr, p);
                p += 2;
                let nl = u2(attr, p) as usize;
                p += 2;
                let mut locals = Vec::new();
                for _ in 0..nl {
                    locals.push(u2(attr, p));
                    p += 2;
                }
                let ns = u2(attr, p) as usize;
                p += 2;
                let mut stack = Vec::new();
                for _ in 0..ns {
                    stack.push(u2(attr, p));
                    p += 2;
                }
                out.push((pc, format!("full {locals:?}"), stack));
            }
            _ => return None,
        }
    }
    Some(out)
}
