//! Build script: pre-compiles the built-in dictionary into a binary DARTS blob.
//!
//! At compile time this script reads `data/words_th.txt`, constructs the full
//! Double-Array Trie (same algorithm as `dict.rs`), serialises the two arrays
//! as little-endian `i32` values, and writes `$OUT_DIR/dict.bin`.
//!
//! At runtime `Dict::from_bytes` simply copies those arrays — O(S) instead of
//! the O(W×K) construction — turning a ~50 ms startup cost into microseconds.
//!
//! Binary layout:
//!   [0..4]   magic   = b"KDAM"
//!   [4]      version = 0x01
//!   [5..8]   reserved zeros
//!   [8..12]  base_len  as u32 LE
//!   [12..16] check_len as u32 LE
//!   [16..]   base[]  as i32 LE values, then check[] as i32 LE values

use std::collections::VecDeque;
use std::fs;
use std::path::Path;

const UNUSED: i32 = -1;
const TERM: u8 = 0;

// ---------------------------------------------------------------------------
// Intermediate trie
// ---------------------------------------------------------------------------

struct TrieNode {
    children: Vec<(u8, usize)>,
}

impl TrieNode {
    fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }

    fn get(&self, b: u8) -> Option<usize> {
        self.children
            .binary_search_by_key(&b, |&(k, _)| k)
            .ok()
            .map(|i| self.children[i].1)
    }

    fn insert(&mut self, b: u8, child_id: usize) {
        match self.children.binary_search_by_key(&b, |&(k, _)| k) {
            Ok(_) => {}
            Err(pos) => self.children.insert(pos, (b, child_id)),
        }
    }
}

fn build_trie(text: &str) -> Vec<TrieNode> {
    let mut nodes: Vec<TrieNode> = vec![TrieNode::new()];
    for line in text.lines() {
        let word = line.trim();
        if word.is_empty() || word.starts_with('#') {
            continue;
        }
        let mut state = 0usize;
        for b in word.bytes().chain(std::iter::once(TERM)) {
            if let Some(next) = nodes[state].get(b) {
                state = next;
            } else {
                let next_id = nodes.len();
                nodes.push(TrieNode::new());
                nodes[state].insert(b, next_id);
                state = next_id;
            }
        }
    }
    nodes
}

// ---------------------------------------------------------------------------
// Bitmap free-list (identical logic to dict.rs)
// ---------------------------------------------------------------------------

struct FreeBitmap {
    words: Vec<u64>,
    cap: usize,
}

impl FreeBitmap {
    fn new(cap: usize) -> Self {
        let n_words = cap.div_ceil(64);
        let mut words = vec![!0u64; n_words];
        let used = cap % 64;
        if used > 0 && n_words > 0 {
            words[n_words - 1] = (1u64 << used) - 1;
        }
        if n_words > 0 {
            words[0] &= !1u64;
        }
        FreeBitmap { words, cap }
    }

    fn occupy(&mut self, slot: usize) {
        self.words[slot / 64] &= !(1u64 << (slot % 64));
    }

    fn grow(&mut self, new_cap: usize) {
        let old_cap = self.cap;
        let old_words = self.words.len();
        let new_words = new_cap.div_ceil(64);
        let old_used = old_cap % 64;
        if old_used > 0 && old_words > 0 {
            self.words[old_words - 1] |= !((1u64 << old_used) - 1);
        }
        self.words.resize(new_words, !0u64);
        let new_used = new_cap % 64;
        if new_used > 0 && new_words > 0 {
            self.words[new_words - 1] = (1u64 << new_used) - 1;
        }
        self.cap = new_cap;
    }

    fn next_free_from(&self, start: usize) -> usize {
        if start >= self.cap {
            return self.cap;
        }
        let cap_word = self.cap.div_ceil(64);
        let word_idx = start / 64;
        let bit_idx = start % 64;
        let partial = self.words[word_idx] >> bit_idx;
        if partial != 0 {
            return (word_idx * 64 + bit_idx + partial.trailing_zeros() as usize).min(self.cap);
        }
        for i in (word_idx + 1)..cap_word {
            if self.words[i] != 0 {
                return (i * 64 + self.words[i].trailing_zeros() as usize).min(self.cap);
            }
        }
        self.cap
    }
}

fn find_base(check: &[i32], children: &[u8], bitmap: &FreeBitmap, min_free: usize) -> usize {
    let min_child = children[0] as usize;
    let mut b = min_free.saturating_sub(min_child + 1).max(1);
    'search: loop {
        for &c in children {
            let pos = b + c as usize + 1;
            if pos < check.len() && check[pos] != UNUSED {
                let nf = bitmap.next_free_from(pos + 1);
                b = nf.saturating_sub(c as usize + 1).max(b + 1);
                continue 'search;
            }
        }
        return b;
    }
}

// ---------------------------------------------------------------------------
// DARTS construction
// ---------------------------------------------------------------------------

fn build_darts(nodes: Vec<TrieNode>) -> (Vec<i32>, Vec<i32>) {
    let n = nodes.len();
    let cap = n * 2 + 512;
    let mut base: Vec<i32> = vec![0; cap];
    let mut check: Vec<i32> = vec![UNUSED; cap];
    check[0] = 0;

    let mut trie_to_darts: Vec<usize> = vec![0; n];
    let mut bitmap = FreeBitmap::new(cap);
    let mut min_free: usize = 1;
    let mut queue: VecDeque<usize> = VecDeque::new();
    queue.push_back(0);

    while let Some(trie_id) = queue.pop_front() {
        let darts_id = trie_to_darts[trie_id];
        let node = &nodes[trie_id];
        if node.children.is_empty() {
            continue;
        }
        while min_free < check.len() && check[min_free] != UNUSED {
            min_free += 1;
        }
        let child_bytes: Vec<u8> = node.children.iter().map(|&(b, _)| b).collect();
        let b = find_base(&check, &child_bytes, &bitmap, min_free);
        base[darts_id] = b as i32;

        for &(byte, child_trie_id) in &node.children {
            let child_darts = b + byte as usize + 1;
            if child_darts + 1 > check.len() {
                let new_cap = child_darts + 512;
                base.resize(new_cap, 0);
                check.resize(new_cap, UNUSED);
                bitmap.grow(new_cap);
            }
            check[child_darts] = darts_id as i32;
            bitmap.occupy(child_darts);
            trie_to_darts[child_trie_id] = child_darts;
            queue.push_back(child_trie_id);
        }
    }

    let last = check.iter().rposition(|&c| c != UNUSED).unwrap_or(0);
    base.truncate(last + 1);
    check.truncate(last + 1);

    (base, check)
}

// ---------------------------------------------------------------------------
// Serialisation
// ---------------------------------------------------------------------------

fn serialize_darts(base: &[i32], check: &[i32]) -> Vec<u8> {
    let base_len = base.len();
    let check_len = check.len();
    let total = 16 + base_len * 4 + check_len * 4;
    let mut out = Vec::with_capacity(total);

    // Header
    out.extend_from_slice(b"KDAM"); // magic
    out.push(0x01); // version
    out.extend_from_slice(&[0u8; 3]); // reserved
    out.extend_from_slice(&(base_len as u32).to_le_bytes());
    out.extend_from_slice(&(check_len as u32).to_le_bytes());

    // Arrays
    for &v in base {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for &v in check {
        out.extend_from_slice(&v.to_le_bytes());
    }

    out
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let words_path = Path::new(&manifest).join("data/words_th.txt");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("dict.bin");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", words_path.display());

    let text = fs::read_to_string(&words_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", words_path.display()));

    let trie = build_trie(&text);
    let (base, check) = build_darts(trie);
    let blob = serialize_darts(&base, &check);

    fs::write(&out_path, &blob)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", out_path.display()));

    println!(
        "cargo:warning=dict.bin: {} states, {} bytes",
        check.len(),
        blob.len()
    );
}
