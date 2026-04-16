//! Dictionary backed by a Double-Array Trie (DARTS).
//!
//! ## Structure
//!
//! A DARTS encodes a trie in two parallel arrays:
//!
//! ```text
//! base[s]  — offset used to compute the next state from state s
//! check[t] — parent state of state t; -1 means "unused"
//!
//! Transition:  next = base[s] + byte + 1
//! Valid iff:   check[next] == s
//! ```
//!
//! Adding 1 to every byte value ensures that even byte 0 (used as the
//! end-of-word sentinel) maps to index ≥ 1, so index 0 is never a child
//! and `check[0]` can serve as the root sentinel.
//!
//! ## Construction
//!
//! 1. Parse the word list into a regular trie (adjacency via `BTreeMap`).
//! 2. BFS over trie nodes. For each node, find the smallest `base` offset
//!    such that `base + byte + 1` is free for every child byte.
//! 3. Stamp `check[base + byte + 1] = current_darts_state` and record the
//!    mapping from trie-node-id → darts-state-id for children.
//!
//! ## Lookup
//!
//! - `contains(word)`: follow bytes then follow the NUL sentinel.
//! - `prefixes(text)`: walk bytes, emit a match whenever NUL sentinel is
//!   reachable at the current state.

use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use alloc::vec;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Marks an unused slot in the `check` array.
const UNUSED: i32 = -1;

/// Sentinel byte appended to every inserted word.
/// NUL (0x00) never appears in valid UTF-8 text, so it's a safe sentinel.
const TERM: u8 = 0;

// ---------------------------------------------------------------------------
// Intermediate trie (used only during construction)
// ---------------------------------------------------------------------------

struct TrieNode {
    /// byte → child trie-node index
    children: BTreeMap<u8, usize>,
}

impl TrieNode {
    fn new() -> Self {
        Self { children: BTreeMap::new() }
    }
}

/// Build a plain trie from a word list and return the nodes.
fn build_trie(text: &str) -> Vec<TrieNode> {
    let mut nodes: Vec<TrieNode> = vec![TrieNode::new()]; // nodes[0] = root

    for line in text.lines() {
        let word = line.trim();
        if word.is_empty() || word.starts_with('#') {
            continue;
        }
        let mut state = 0usize;
        // Append TERM so the terminal is just another trie node.
        for b in word.bytes().chain(core::iter::once(TERM)) {
            // Look up the child first to avoid holding a borrow when pushing.
            if !nodes[state].children.contains_key(&b) {
                let next_id = nodes.len();
                nodes.push(TrieNode::new());
                nodes[state].children.insert(b, next_id);
            }
            state = nodes[state].children[&b];
        }
    }

    nodes
}

// ---------------------------------------------------------------------------
// Base-allocation helper
// ---------------------------------------------------------------------------

/// Find the smallest offset `b ≥ 1` such that `check[b + byte + 1]` is free
/// (either `UNUSED` or out-of-bounds) for every byte in `children`.
///
/// Uses a free-list of known-unused slots so the scan advances in O(1) per
/// candidate rather than scanning the whole `check` array linearly.
/// `free_head` is the smallest index that is currently free; on return it is
/// updated to the next free slot so successive calls stay cheap.
fn find_base(check: &[i32], children: &[u8], free_head: &mut usize) -> usize {
    debug_assert!(!children.is_empty());
    // Anchor on the smallest child byte: the base must place that child at a
    // free slot, so we only need to try b values where b + min_child + 1 is free.
    let min_child = children[0] as usize; // children are sorted ascending

    // Advance free_head past any occupied slots.
    while *free_head < check.len() && check[*free_head] != UNUSED {
        *free_head += 1;
    }

    // Starting candidate: the smallest b that places min_child on free_head.
    let mut b = free_head.saturating_sub(min_child + 1).max(1);

    'search: loop {
        for &c in children {
            let pos = b + c as usize + 1;
            if pos < check.len() && check[pos] != UNUSED {
                b += 1;
                continue 'search;
            }
        }
        return b;
    }
}

// ---------------------------------------------------------------------------
// Public Dict type
// ---------------------------------------------------------------------------

/// Double-Array Trie dictionary.
///
/// Provides O(k) lookup where k is the number of **bytes** in the query.
/// Memory layout is two `Vec<i32>` (base + check), cache-friendly.
pub struct Dict {
    base: Vec<i32>,
    check: Vec<i32>,
}

impl Dict {
    // ── Construction ─────────────────────────────────────────────────────────

    /// Build a [`Dict`] from a newline-separated word list.
    ///
    /// Lines starting with `#` are treated as comments and skipped.
    /// Blank lines are skipped. Words are stored as raw UTF-8 bytes.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::dict::Dict;
    ///
    /// let dict = Dict::from_word_list("กิน\nข้าว\nปลา\n");
    /// assert!(dict.contains("กิน"));
    /// assert!(dict.contains("ข้าว"));
    /// assert!(!dict.contains("xyz"));
    /// ```
    pub fn from_word_list(text: &str) -> Self {
        let trie = build_trie(text);
        Self::from_trie(trie)
    }

    fn from_trie(nodes: Vec<TrieNode>) -> Self {
        let n = nodes.len();
        // Initial capacity: trie nodes * average fanout, plus breathing room.
        let cap = n * 2 + 512;
        let mut base: Vec<i32> = vec![0; cap];
        let mut check: Vec<i32> = vec![UNUSED; cap];

        // check[0] = 0 marks the root as "in use" (its own parent = itself).
        check[0] = 0;

        // trie_to_darts[trie_node_id] = darts_state_index
        let mut trie_to_darts: Vec<usize> = vec![0; n];
        // Root trie node (0) → root darts state (0).

        // Tracks the lowest known-free slot; passed into find_base so it
        // doesn't restart the scan from 1 on every call.
        let mut free_head: usize = 1;

        let mut queue: VecDeque<usize> = VecDeque::new();
        queue.push_back(0); // start BFS from root trie node

        while let Some(trie_id) = queue.pop_front() {
            let darts_id = trie_to_darts[trie_id];
            let node = &nodes[trie_id];

            if node.children.is_empty() {
                continue; // leaf node — nothing to allocate
            }

            // Collect sorted child bytes for base search.
            let child_bytes: Vec<u8> = node.children.keys().copied().collect();

            // Find a collision-free base offset.
            let b = find_base(&check, &child_bytes, &mut free_head);
            base[darts_id] = b as i32;

            // Stamp each child into the arrays.
            for (&byte, &child_trie_id) in &node.children {
                let child_darts = b + byte as usize + 1;

                // Grow arrays if the new child index exceeds current capacity.
                if child_darts + 1 > check.len() {
                    let new_cap = child_darts + 512;
                    base.resize(new_cap, 0);
                    check.resize(new_cap, UNUSED);
                }

                check[child_darts] = darts_id as i32;
                trie_to_darts[child_trie_id] = child_darts;
                queue.push_back(child_trie_id);
            }
        }

        // Trim trailing unused slots to reduce memory footprint.
        let last = check.iter().rposition(|&c| c != UNUSED).unwrap_or(0);
        base.truncate(last + 1);
        check.truncate(last + 1);

        Self { base, check }
    }

    // ── Lookup primitives ────────────────────────────────────────────────────

    /// Follow one transition from `state` on `byte`.
    /// Returns `Some(next_state)` if valid, `None` otherwise.
    #[inline]
    fn goto(&self, state: usize, byte: u8) -> Option<usize> {
        let next = (*self.base.get(state)?) + byte as i32 + 1;
        if next <= 0 {
            return None;
        }
        let next = next as usize;
        if self.check.get(next).copied()? == state as i32 {
            Some(next)
        } else {
            None
        }
    }

    // ── Public API ───────────────────────────────────────────────────────────

    /// Returns `true` if `word` is present in the dictionary.
    ///
    /// Lookup is O(n) where n is `word.len()` in bytes.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::dict::Dict;
    ///
    /// let dict = Dict::from_word_list("สวัสดี\nโลก\n");
    /// assert!(dict.contains("สวัสดี"));
    /// assert!(dict.contains("โลก"));
    /// assert!(!dict.contains("สวัส")); // prefix only — not a word
    /// assert!(!dict.contains(""));
    /// ```
    pub fn contains(&self, word: &str) -> bool {
        if word.is_empty() {
            return false;
        }
        let mut state = 0usize;
        for b in word.bytes() {
            match self.goto(state, b) {
                Some(s) => state = s,
                None => return false,
            }
        }
        self.goto(state, TERM).is_some()
    }

    /// Return all substrings of `text` (anchored at byte 0) that are present
    /// in the dictionary, ordered **longest first**.
    ///
    /// Only returns slices that are valid UTF-8 boundaries (which is always
    /// guaranteed when the dictionary contains valid UTF-8 words).
    ///
    /// # Example
    ///
    /// ```rust
    /// use kham_core::dict::Dict;
    ///
    /// let dict = Dict::from_word_list("กิน\nกิน\nกิน\nกินข้าว\n");
    /// let p = dict.prefixes("กินข้าวกับปลา");
    /// // Longest prefix is "กินข้าว", then "กิน"
    /// assert_eq!(p[0], "กินข้าว");
    /// assert_eq!(p[1], "กิน");
    /// ```
    pub fn prefixes<'t>(&self, text: &'t str) -> Vec<&'t str> {
        let mut result = Vec::new();
        let mut state = 0usize;
        let bytes = text.as_bytes();

        for i in 0..bytes.len() {
            match self.goto(state, bytes[i]) {
                Some(s) => {
                    state = s;
                    if self.goto(state, TERM).is_some() {
                        // i is the index of the last byte; the prefix is text[0..i+1].
                        // Since dictionary words are valid UTF-8, i+1 is always a
                        // char boundary.
                        debug_assert!(text.is_char_boundary(i + 1));
                        result.push(&text[..i + 1]);
                    }
                }
                None => break,
            }
        }

        // Reverse so the longest match comes first.
        result.reverse();
        result
    }

    /// Number of allocated DARTS states (informational / testing).
    #[inline]
    pub fn state_count(&self) -> usize {
        self.check.len()
    }
}

// ---------------------------------------------------------------------------
// Built-in word list
// ---------------------------------------------------------------------------

/// The built-in CC0 Thai word list, embedded at compile time.
///
/// Pass to [`Dict::from_word_list`] to build the default dictionary.
pub static BUILTIN_WORDS: &str = include_str!("../data/words_th.txt");

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn small_dict() -> Dict {
        Dict::from_word_list("กิน\nข้าว\nปลา\nน้ำ\nกินข้าว\n")
    }

    // ── contains ─────────────────────────────────────────────────────────────

    #[test]
    fn contains_exact_words() {
        let d = small_dict();
        assert!(d.contains("กิน"));
        assert!(d.contains("ข้าว"));
        assert!(d.contains("ปลา"));
        assert!(d.contains("น้ำ"));
        assert!(d.contains("กินข้าว"));
    }

    #[test]
    fn contains_rejects_prefix_only() {
        let d = small_dict();
        // "กิ" is a prefix of "กิน" but not itself a word
        assert!(!d.contains("กิ"));
    }

    #[test]
    fn contains_rejects_empty() {
        let d = small_dict();
        assert!(!d.contains(""));
    }

    #[test]
    fn contains_rejects_unknown() {
        let d = small_dict();
        assert!(!d.contains("xyz"));
        assert!(!d.contains("hello"));
    }

    #[test]
    fn contains_ascii() {
        let d = Dict::from_word_list("hello\nworld\n");
        assert!(d.contains("hello"));
        assert!(d.contains("world"));
        assert!(!d.contains("hell"));
        assert!(!d.contains("worlds"));
    }

    #[test]
    fn contains_comments_and_blanks_skipped() {
        let d = Dict::from_word_list("# comment\n\nกิน\n   \nข้าว\n");
        assert!(d.contains("กิน"));
        assert!(d.contains("ข้าว"));
        assert!(!d.contains("# comment"));
    }

    #[test]
    fn contains_single_char() {
        let d = Dict::from_word_list("ก\n");
        assert!(d.contains("ก"));
        assert!(!d.contains("กก"));
    }

    // ── prefixes ──────────────────────────────────────────────────────────────

    #[test]
    fn prefixes_longest_first() {
        let d = small_dict();
        let p = d.prefixes("กินข้าวกับปลา");
        // "กินข้าว" and "กิน" are both prefixes of the input
        assert!(p.contains(&"กินข้าว"));
        assert!(p.contains(&"กิน"));
        // Longest must come first
        assert_eq!(p[0], "กินข้าว");
    }

    #[test]
    fn prefixes_single_match() {
        let d = small_dict();
        let p = d.prefixes("ปลาทู");
        assert_eq!(p, vec!["ปลา"]);
    }

    #[test]
    fn prefixes_no_match() {
        let d = small_dict();
        assert!(d.prefixes("xyz").is_empty());
        assert!(d.prefixes("").is_empty());
    }

    #[test]
    fn prefixes_exact_match() {
        let d = small_dict();
        // "กิน" is exactly a word; text doesn't extend further
        let p = d.prefixes("กิน");
        assert_eq!(p, vec!["กิน"]);
    }

    // ── full-round-trip / structural ──────────────────────────────────────────

    #[test]
    fn builtin_words_parse() {
        // Built-in word list must parse without panic and contain a few known words
        let d = Dict::from_word_list(BUILTIN_WORDS);
        assert!(d.contains("กิน"));
        assert!(d.contains("ข้าว"));
    }

    #[test]
    fn large_word_list() {
        // Insert many words and verify all are retrievable
        let words = [
            "กิน", "ข้าว", "ปลา", "น้ำ", "คน", "ไป", "มา", "ที่", "นี่", "นั้น",
            "สวัสดี", "ธนาคาร", "แห่ง", "ชาวโลก", "ประเทศ", "ภาษา", "เมือง", "บ้าน",
        ];
        let list: alloc::string::String = words
            .iter()
            .map(|w| alloc::format!("{w}\n"))
            .collect();
        let d = Dict::from_word_list(&list);
        for &w in &words {
            assert!(d.contains(w), "missing word: {w}");
        }
        assert!(!d.contains("notaword"));
    }

    #[test]
    fn check_array_validity() {
        // Every non-UNUSED entry in check must point to a valid state index.
        let d = small_dict();
        let len = d.check.len() as i32;
        for &c in &d.check {
            if c != UNUSED {
                assert!(c >= 0 && c < len, "check entry {c} out of range");
            }
        }
    }
}
