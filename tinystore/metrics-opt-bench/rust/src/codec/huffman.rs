//! Huff0 uses the zstd Huffman table and single-stream byte format. Native
//! compression/table parsing come from the pinned zstd dependency, while the
//! dynamic-length decode below follows huff0's canonical table construction.
use super::Result;
use std::cell::RefCell;
use std::ffi::{c_int, c_uint, c_void};

struct Scratch {
    workspace: Vec<u64>,
    table: [usize; 257],
}
thread_local! {static SCRATCH:RefCell<Scratch>=RefCell::new(Scratch{workspace:vec![0;1088],table:[0;257]});}

unsafe extern "C" {
    fn HUF_compress1X_repeat(
        dst: *mut c_void,
        dst_size: usize,
        src: *const c_void,
        src_size: usize,
        maximum: c_uint,
        table_log: c_uint,
        workspace: *mut c_void,
        workspace_size: usize,
        table: *mut usize,
        repeat: *mut c_int,
        flags: c_int,
    ) -> usize;
    fn HUF_readStats(
        weights: *mut u8,
        weight_size: usize,
        ranks: *mut u32,
        symbols: *mut u32,
        table_log: *mut u32,
        src: *const c_void,
        src_size: usize,
    ) -> usize;
    fn HUF_isError(code: usize) -> c_uint;
}

pub fn compress(symbols: &[u8]) -> Option<Vec<u8>> {
    if symbols.is_empty() {
        return None;
    }
    let mut out = vec![0u8; symbols.len() + 1024];
    let size = SCRATCH.with(|slot| {
        let mut scratch = slot.borrow_mut();
        let mut repeat = 0;
        let Scratch { workspace, table } = &mut *scratch;
        // Buffers are owned, correctly aligned, and sized per zstd's huf.h.
        unsafe {
            HUF_compress1X_repeat(
                out.as_mut_ptr().cast(),
                out.len(),
                symbols.as_ptr().cast(),
                symbols.len(),
                255,
                11,
                workspace.as_mut_ptr().cast(),
                workspace.len() * 8,
                table.as_mut_ptr(),
                &mut repeat,
                0,
            )
        }
    });
    if size <= 1 || size >= symbols.len() || unsafe { HUF_isError(size) } != 0 {
        return None;
    }
    out.truncate(size);
    Some(out)
}

pub fn decode(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 2 {
        return Err("huffman table too small".into());
    }
    let mut weights = [0u8; 256];
    let mut ranks = [0u32; 16];
    let mut symbols = 0;
    let mut log = 0;
    // HUF_readStats bounds the output to the 256-byte weight buffer and checks
    // both raw and FSE-compressed weight tables, including the implied symbol.
    let header = unsafe {
        HUF_readStats(
            weights.as_mut_ptr(),
            weights.len(),
            ranks.as_mut_ptr(),
            &mut symbols,
            &mut log,
            data.as_ptr().cast(),
            data.len(),
        )
    };
    if unsafe { HUF_isError(header) } != 0 || header >= data.len() || log > 11 || symbols > 256 {
        return Err("huffman table".into());
    }
    let mut next = 0u32;
    for weight in 1..=log as usize {
        let current = next;
        next += ranks[weight] << (weight - 1);
        ranks[weight] = current;
    }
    let mut table = vec![(0u8, 0u8); 1usize << log];
    for (symbol, &weight) in weights[..symbols as usize].iter().enumerate() {
        if weight == 0 {
            continue;
        }
        let length = 1usize << (weight - 1);
        let at = ranks[weight as usize] as usize;
        if at + length > table.len() {
            return Err("huffman table bounds".into());
        }
        table[at..at + length].fill((symbol as u8, (log + 1 - u32::from(weight)) as u8));
        ranks[weight as usize] += length as u32;
    }
    let stream = &data[header..];
    let last = *stream.last().unwrap();
    if last == 0 {
        return Err("huffman missing end marker".into());
    }
    let mut left = (stream.len() - 1) * 8 + (7 - last.leading_zeros()) as usize;
    let mut out = Vec::new();
    while left > 0 {
        let mut code = 0usize;
        for offset in 0..log as usize {
            code <<= 1;
            if offset < left {
                let position = left - 1 - offset;
                code |= ((stream[position / 8] >> (position % 8)) & 1) as usize;
            }
        }
        let (symbol, width) = table[code];
        if width == 0 || width as usize > left || out.len() == 8192 {
            return Err("huffman stream bounds".into());
        }
        left -= width as usize;
        out.push(symbol);
    }
    Ok(out)
}
