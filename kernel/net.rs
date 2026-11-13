// ASPECT OS - own network stack (M6): ARP, IPv4, ICMP echo. Frames move
// through the e1000 driver; all state is static memory, no heap. The
// test configuration is QEMU's user network: our address 10.0.2.15/24,
// gateway 10.0.2.2 (which answers ICMP echo).
#![allow(unknown_lints, static_mut_refs, dead_code)]

use crate::e1000;
use crate::pit;
use crate::serial;

// ethertypes
const ET_ARP: u16 = 0x0806;
const ET_IPV4: u16 = 0x0800;
// IP protocols
const IP_ICMP: u8 = 1;
// ARP opcodes
const ARP_REQ: u16 = 1;
const ARP_REP: u16 = 2;
// ICMP types
const ICMP_ECHO_REQ: u8 = 8;
const ICMP_ECHO_REP: u8 = 0;

pub const OUR_IP: [u8; 4] = [10, 0, 2, 15];
pub const GW_IP: [u8; 4] = [10, 0, 2, 2];

const ARP_MAX: usize = 8;

static mut ARP_IP: [[u8; 4]; ARP_MAX] = [[0; 4]; ARP_MAX];
static mut ARP_MAC: [[u8; 6]; ARP_MAX] = [[0; 6]; ARP_MAX];
static mut ARP_N: usize = 0;

static mut TXF: [u8; 1536] = [0; 1536]; // frame build buffer

// pending ping (one at a time, the terminal sends and waits)
static mut PING_WAIT: bool = false;
static mut PING_ID: u16 = 0x4153; // "AS"
static mut PING_SEQ: u16 = 0;
static mut PING_START: u64 = 0;
static mut PING_RTT: u64 = 0;
static mut PING_GOT: bool = false;

// stats
static mut RX_OK: u32 = 0;
static mut TX_OK: u32 = 0;

pub fn init() -> bool {
    unsafe {
        let bar = crate::pci::nic_bar();
        if !e1000::init(bar) {
            return false;
        }
        serial::log("net: stack up (10.0.2.15, gw 10.0.2.2)\n");
        true
    }
}

pub fn ready() -> bool {
    e1000::ready()
}

pub fn stats() -> (u32, u32) {
    unsafe { (RX_OK, TX_OK) }
}

// ---- checksums ----

/// 16-bit ones-complement checksum over `data` (even length assumed;
/// a trailing odd byte is folded in)
fn checksum16(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 1 < data.len() {
        sum += ((data[i] as u32) << 8) | data[i + 1] as u32;
        i += 2;
    }
    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }
    while sum > 0xFFFF {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

// ---- ARP ----

fn arp_learn(ip: &[u8; 4], mac: &[u8; 6]) {
    unsafe {
        for i in 0..ARP_N {
            if ARP_IP[i] == *ip {
                ARP_MAC[i] = *mac;
                return;
            }
        }
        if ARP_N < ARP_MAX {
            ARP_IP[ARP_N] = *ip;
            ARP_MAC[ARP_N] = *mac;
            ARP_N += 1;
        }
    }
}

fn arp_lookup(ip: &[u8; 4]) -> Option<[u8; 6]> {
    unsafe {
        for i in 0..ARP_N {
            if ARP_IP[i] == *ip {
                return Some(ARP_MAC[i]);
            }
        }
    }
    None
}

/// broadcast an ARP request for `ip` and poll for the reply
fn arp_resolve(ip: &[u8; 4]) -> Option<[u8; 6]> {
    if let Some(m) = arp_lookup(ip) {
        return Some(m);
    }
    unsafe {
        let mac = e1000::mac();
        let f = &mut TXF;
        f[..42].fill(0);
        f[0..6].fill(0xFF); // broadcast
        f[6..12].copy_from_slice(&mac);
        f[12] = (ET_ARP >> 8) as u8;
        f[13] = (ET_ARP & 0xFF) as u8;
        f[14] = 0;
        f[15] = 1; // ethernet
        f[16] = (ET_IPV4 >> 8) as u8;
        f[17] = (ET_IPV4 & 0xFF) as u8;
        f[18] = 6; // hlen
        f[19] = 4; // plen
        f[20] = (ARP_REQ >> 8) as u8;
        f[21] = (ARP_REQ & 0xFF) as u8;
        f[22..28].copy_from_slice(&mac); // sender hardware
        f[28..32].copy_from_slice(&OUR_IP); // sender protocol
        f[32..38].fill(0); // target hardware (unknown)
        f[38..42].copy_from_slice(ip); // target protocol
        if !e1000::send(&f[..42]) {
            return None;
        }
        TX_OK += 1;
    }
    // wait up to ~300 ms for the reply (poll drains incoming frames)
    let start = pit::ticks();
    loop {
        poll();
        if let Some(m) = arp_lookup(ip) {
            unsafe {
                let mut b = [0u8; 20];
                let n = ip_str(ip, &mut b);
                serial::log("net: arp ");
                serial::log_bytes(&b[..n]);
                serial::log(" resolved\n");
            }
            return Some(m);
        }
        if pit::ticks() - start > 300 {
            unsafe {
                let mut b = [0u8; 20];
                let n = ip_str(ip, &mut b);
                serial::log("net: arp ");
                serial::log_bytes(&b[..n]);
                serial::log(" timeout\n");
            }
            return None;
        }
    }
}

/// handle one ARP frame (starts at the ARP payload)
unsafe fn arp_in(f: &[u8]) {
    if f.len() < 28 {
        return;
    }
    let oper = ((f[6] as u16) << 8) | f[7] as u16;
    let mut sha = [0u8; 6];
    sha.copy_from_slice(&f[8..14]);
    let mut spa = [0u8; 4];
    spa.copy_from_slice(&f[14..18]);
    let mut tpa = [0u8; 4];
    tpa.copy_from_slice(&f[24..28]);
    arp_learn(&spa, &sha);
    if oper == ARP_REQ && tpa == OUR_IP {
        // answer: our address is the requested one
        let mac = e1000::mac();
        let r = &mut TXF;
        r[..42].fill(0);
        r[0..6].copy_from_slice(&sha); // reply goes to the requester
        r[6..12].copy_from_slice(&mac);
        r[12] = (ET_ARP >> 8) as u8;
        r[13] = (ET_ARP & 0xFF) as u8;
        r[14] = 0;
        r[15] = 1;
        r[16] = (ET_IPV4 >> 8) as u8;
        r[17] = (ET_IPV4 & 0xFF) as u8;
        r[18] = 6;
        r[19] = 4;
        r[20] = (ARP_REP >> 8) as u8;
        r[21] = (ARP_REP & 0xFF) as u8;
        r[22..28].copy_from_slice(&mac);
        r[28..32].copy_from_slice(&OUR_IP);
        r[32..38].copy_from_slice(&sha);
        r[38..42].copy_from_slice(&spa);
        if e1000::send(&r[..42]) {
            TX_OK += 1;
        }
    }
}

// ---- IPv4 ----

/// send an IP packet; routes through the gateway for off-subnet targets
unsafe fn ip_send(proto: u8, dst: &[u8; 4], payload: &[u8]) -> bool {
    // destination MAC: the host itself, or the gateway
    let local = dst[0] == OUR_IP[0]
        && dst[1] == OUR_IP[1]
        && dst[2] == OUR_IP[2];
    let hop = if local { *dst } else { GW_IP };
    let dmac = match arp_resolve(&hop) {
        Some(m) => m,
        None => return false,
    };
    let mac = e1000::mac();
    let f = &mut TXF;
    let ihl = 20usize;
    // frame length = ethernet header (14) + IP header + payload
    let total = 14 + ihl + payload.len();
    if total > 1500 || payload.is_empty() {
        return false;
    }
    f[..total].fill(0);
    f[0..6].copy_from_slice(&dmac);
    f[6..12].copy_from_slice(&mac);
    f[12] = (ET_IPV4 >> 8) as u8;
    f[13] = (ET_IPV4 & 0xFF) as u8;
    f[14] = 0x45; // version 4, IHL 5
    f[15] = 0; // DSCP
    // IP total length: header + payload
    f[16] = ((total - 14) >> 8) as u8;
    f[17] = ((total - 14) & 0xFF) as u8;
    f[18] = 0; // id
    f[19] = 0;
    f[20] = 0x40; // flags: don't fragment
    f[21] = 0;
    f[22] = 64; // ttl
    f[23] = proto;
    f[26..30].copy_from_slice(&OUR_IP);
    f[30..34].copy_from_slice(dst);
    // checksum over the whole header; the checksum field is still zero
    let sum = checksum16(&f[14..14 + ihl]);
    f[24] = (sum >> 8) as u8;
    f[25] = (sum & 0xFF) as u8;
    f[14 + ihl..total].copy_from_slice(payload);
    if e1000::send(&f[..total]) {
        TX_OK += 1;
        true
    } else {
        false
    }
}

/// handle one IPv4 datagram (starts at the IP header)
unsafe fn ip_in(f: &[u8]) {
    if f.len() < 20 {
        return;
    }
    if f[0] >> 4 != 4 {
        return;
    }
    let ihl = (f[0] & 0xF) as usize * 4;
    if ihl < 20 || f.len() < ihl {
        return;
    }
    let mut dst = [0u8; 4];
    dst.copy_from_slice(&f[16..20]);
    if dst != OUR_IP {
        return;
    }
    let mut src = [0u8; 4];
    src.copy_from_slice(&f[12..16]);
    let proto = f[9];
    let body = &f[ihl..];
    if proto == IP_ICMP {
        icmp_in(body, &src);
    }
}

// ---- ICMP ----

/// handle an ICMP message (starts at the ICMP header)
unsafe fn icmp_in(f: &[u8], src: &[u8; 4]) {
    if f.len() < 8 {
        return;
    }
    let ty = f[0];
    if ty == ICMP_ECHO_REQ {
        // echo reply: same bytes, type 0, fresh checksum
        let mut r = [0u8; 72];
        let n = f.len().min(r.len());
        r[..n].copy_from_slice(&f[..n]);
        r[0] = ICMP_ECHO_REP;
        r[2] = 0;
        r[3] = 0;
        let sum = checksum16(&r[..n]);
        r[2] = (sum >> 8) as u8;
        r[3] = (sum & 0xFF) as u8;
        ip_send(IP_ICMP, src, &r[..n]);
    } else if ty == ICMP_ECHO_REP {
        let id = ((f[4] as u16) << 8) | f[5] as u16;
        let seq = ((f[6] as u16) << 8) | f[7] as u16;
        if PING_WAIT && id == PING_ID && seq == PING_SEQ && *src == PING_TARGET {
            PING_RTT = pit::ticks().saturating_sub(PING_START);
            PING_GOT = true;
            PING_WAIT = false;
        }
    }
}

static mut PING_TARGET: [u8; 4] = [0; 4];

/// send one ICMP echo request; returns once the reply is matched or the
/// ~400 ms window passes. Some(0) means reply received.
pub fn ping_once(target: &[u8; 4]) -> Option<u64> {
    unsafe {
        if !ready() {
            return None;
        }
        // resolve the MAC first (direct or via gateway)
        let local = target[0] == OUR_IP[0] && target[1] == OUR_IP[1] && target[2] == OUR_IP[2];
        let hop = if local { *target } else { GW_IP };
        if arp_resolve(&hop).is_none() {
            return None;
        }
        PING_SEQ = PING_SEQ.wrapping_add(1);
        PING_TARGET = *target;
        PING_GOT = false;
        PING_WAIT = false;
        let mut payload = [0u8; 32];
        let t = pit::ticks();
        for i in 0..8 {
            payload[i] = (t >> (i * 8)) as u8;
        }
        for i in 0..24 {
            payload[8 + i] = b'A';
        }
        let mut msg = [0u8; 40];
        msg[0] = ICMP_ECHO_REQ;
        msg[1] = 0;
        msg[2] = 0;
        msg[3] = 0;
        msg[4] = (PING_ID >> 8) as u8;
        msg[5] = (PING_ID & 0xFF) as u8;
        msg[6] = (PING_SEQ >> 8) as u8;
        msg[7] = (PING_SEQ & 0xFF) as u8;
        msg[8..40].copy_from_slice(&payload);
        let sum = checksum16(&msg);
        msg[2] = (sum >> 8) as u8;
        msg[3] = (sum & 0xFF) as u8;
        if !ip_send(IP_ICMP, target, &msg) {
            return None;
        }
        PING_START = pit::ticks();
        PING_WAIT = true;
        let start = PING_START;
        loop {
            poll();
            if !PING_WAIT {
                break;
            }
            if pit::ticks() - start > 400 {
                PING_WAIT = false;
                serial::log("net: ping timeout\n");
                return None; // timeout
            }
        }
        if PING_GOT {
            serial::log("net: ping reply\n");
            Some(PING_RTT)
        } else {
            None
        }
    }
}

// ---- frame dispatch ----

/// drain the NIC and dispatch every received frame
pub fn poll() {
    e1000::poll(|frame| {
        // the controller keeps the 4-byte CRC: strip it before parsing
        let n = frame.len().saturating_sub(4);
        if n < 14 {
            return;
        }
        let f = &frame[..n];
        unsafe { RX_OK += 1 };
        let et = ((f[12] as u16) << 8) | f[13] as u16;
        match et {
            ET_ARP => unsafe { arp_in(&f[14..]) },
            ET_IPV4 => unsafe { ip_in(&f[14..]) },
            _ => {}
        }
    });
}

// ---- formatting helpers for the terminal ----

/// "10.0.2.15" into out, returns length
pub fn ip_str(ip: &[u8; 4], out: &mut [u8]) -> usize {
    let mut n = 0;
    for (i, b) in ip.iter().enumerate() {
        let v = *b;
        if v >= 100 {
            push(out, &mut n, (b'0' + v / 100) as u8);
            push(out, &mut n, (b'0' + (v / 10) % 10) as u8);
            push(out, &mut n, (b'0' + v % 10) as u8);
        } else if v >= 10 {
            push(out, &mut n, (b'0' + v / 10) as u8);
            push(out, &mut n, (b'0' + v % 10) as u8);
        } else {
            push(out, &mut n, (b'0' + v) as u8);
        }
        if i < 3 {
            push(out, &mut n, b'.');
        }
    }
    n
}

/// "52:54:00:12:34:56" into out, returns length
pub fn mac_str(mac: &[u8; 6], out: &mut [u8]) -> usize {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut n = 0;
    for (i, b) in mac.iter().enumerate() {
        push(out, &mut n, HEX[(b >> 4) as usize]);
        push(out, &mut n, HEX[(b & 0xF) as usize]);
        if i < 5 {
            push(out, &mut n, b':');
        }
    }
    n
}

#[inline]
fn push(out: &mut [u8], n: &mut usize, b: u8) {
    if *n < out.len() {
        out[*n] = b;
        *n += 1;
    }
}
