use crate::deflate::{Deflate, MAX_DIST, MAX_MATCH, MIN_MATCH};
use crate::{GzipState, STORED};
use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

const MAX_BITS: usize = 15;
const MAX_BL_BITS: usize = 7;
const LENGTH_CODES: usize = 29;
const LITERALS: usize = 256;
const LIT_BUFSIZE: usize = 0x8000;
const DIST_BUFSIZE: usize = 0x8000;
const L_CODES: usize = LITERALS + 1 + LENGTH_CODES;
const D_CODES: usize = 30;
const BL_CODES: usize = 19;
const HEAP_SIZE: usize = 2 * L_CODES + 1;
const END_BLOCK: usize = 256;
const STORED_BLOCK: usize = 0;
const STATIC_TREES: usize = 1;
const DYN_TREES: usize = 2;
const SMALLEST: usize = 1;
const BINARY: u16 = 0;
const ASCII: u16 = 1;
const REP_3_6: usize = 16;
/* repeat previous bit length 3-6 times (2 bits of repeat count) */

const REPZ_3_10: usize = 17;
/* repeat a zero length 3-10 times  (3 bits of repeat count) */

const REPZ_11_138: usize = 18;
/* repeat a zero length 11-138 times  (7 bits of repeat count) */

const EXTRA_LBITS: [i32; LENGTH_CODES] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1,
    2, 2, 2, 2, 3, 3, 3, 3,
    4, 4, 4, 4, 5, 5, 5, 5, 0,
];

const EXTRA_DBITS: [i32; D_CODES] = [
    0, 0, 0, 0, 1, 1, 2, 2,
    3, 3, 4, 4, 5, 5, 6, 6,
    7, 7, 8, 8, 9, 9, 10, 10,
    11, 11, 12, 12, 13, 13,
];

const EXTRA_BLBITS: [i32; BL_CODES] = [
    0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0,
    0, 0, 2, 3, 7
];

const BL_ORDER: [usize; BL_CODES] = [
    16, 17, 18, 0, 8, 7, 9, 6,
    10, 5, 11, 4, 12, 3, 13, 2,
    14, 1, 15,
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum TreeType {
    Literal,
    Distance,
    BitLength,
}

#[derive(Default, Copy, Clone, Debug)]
pub struct CtData {
    freq: u16,
    len: u16,
    code: u16,
    dad: u16
}

pub struct Trees<'a> {
    pub file_type: Option<&'a mut u16>,
    pub file_method: i32,
    pub compressed_len: u64,
    pub input_len: u64,
    pub base_length: [i32; LENGTH_CODES],
    pub base_dist: [i32; D_CODES],
    pub length_code: [u8; 256],
    pub dist_code: [u8; 512],
    pub bl_count: [i32; MAX_BITS + 1],
    pub static_ltree: Vec<CtData>,
    pub static_dtree: Vec<CtData>,
    pub bltree: Vec<CtData>,
    pub dyn_ltree: Vec<CtData>,
    pub dyn_dtree: Vec<CtData>,
    pub bl_tree: Vec<CtData>,
    pub opt_len: u64,
    pub static_len: u64,
    pub last_lit: i32,
    pub last_dist: i32,
    pub last_flags: i32,
    pub flags: u8,
    pub flag_bit: u8,
    pub l_buf: Box<[usize; LIT_BUFSIZE]>,
    pub d_buf: Box<[usize; DIST_BUFSIZE]>,
    pub flag_buf: Box<[usize; LIT_BUFSIZE/8]>,
    pub l_desc: TreeDesc<'a>,
    pub d_desc: TreeDesc<'a>,
    pub bl_desc: TreeDesc<'a>,
    pub heap: [i32; 2*L_CODES+1],
    pub depth: [i32; 2*L_CODES+1],
    pub heap_len: usize,
    pub heap_max: usize
}

#[derive(Clone)]
struct TreeDesc<'a> {
    tree_type: TreeType,  // 用于标识使用哪个树
    extra_bits: Option<&'a [i32]>,    // Extra bits for each code or None
    extra_base: usize,
    elems: usize,                    // Number of elements in the tree
    max_length: usize,               // Maximum bit length for the codes
    max_code: i32,                   // Largest code with non-zero frequency
}

// 添加一个新的枚举来区分静态树和动态树
enum TreeKind {
    Static,
    Dynamic,
}

impl<'a> Trees<'a> {
    pub fn new() -> Self {
        let static_ltree = vec![CtData::default(); L_CODES + 2];
        let static_dtree = vec![CtData::default(); D_CODES];
        let bltree = vec![CtData::default(); 2 * BL_CODES + 1];
        let dyn_ltree = vec![CtData::default(); HEAP_SIZE];
        let dyn_dtree = vec![CtData::default(); 2 * D_CODES + 1];
        let bl_tree = vec![CtData::default(); 2 * BL_CODES + 1];
        Self {
            file_type: None,
            file_method: 0,
            compressed_len: 0,
            input_len: 0,
            base_length: [0; LENGTH_CODES],
            base_dist: [0; D_CODES],
            length_code: [0; 256],
            dist_code: [0; 512],
            bl_count: [0; MAX_BITS + 1],
            static_ltree,
            static_dtree,
            bltree,
            dyn_ltree,
            dyn_dtree,
            bl_tree,
            opt_len: 0,
            static_len: 0,
            last_lit: 0,
            last_dist: 0,
            last_flags: 0,
            flags: 0,
            flag_bit: 1,
            l_buf: Box::new([0; LIT_BUFSIZE]),
            d_buf: Box::new([0; DIST_BUFSIZE]),
            flag_buf: Box::new([0; LIT_BUFSIZE/8]),
            l_desc: TreeDesc {
                tree_type: TreeType::Literal,
                // dyn_tree: dyn_ltree,
                // static_tree: Some(static_ltree),
                extra_bits: Some(&EXTRA_LBITS),
                extra_base: LITERALS+1,
                elems: L_CODES,
                max_length: MAX_BITS,
                max_code: 0,
            },
            d_desc: TreeDesc {
                tree_type: TreeType::Distance,
                // dyn_tree: dyn_dtree,
                // static_tree: Some(static_dtree),
                extra_bits: Some(&EXTRA_DBITS),
                extra_base: 0,
                elems: D_CODES,
                max_length: MAX_BITS,
                max_code: 0,
            },
            bl_desc: TreeDesc {
                tree_type: TreeType::BitLength,
                // dyn_tree: bltree,
                // static_tree: None,
                extra_bits: Some(&EXTRA_BLBITS),
                extra_base: 0,
                elems: BL_CODES,
                max_length: MAX_BL_BITS,
                max_code: 0,
            },
            heap: [0; 2*L_CODES+1],
            depth: [0; 2*L_CODES+1],
            heap_len: 0,
            heap_max: 0
        }
    }



    // 根据树类型获取对应的树数据的可变引用
    fn get_tree_mut(&mut self, tree_type: TreeType) -> &mut [CtData] {
        match tree_type {
            TreeType::Literal => &mut self.dyn_ltree,
            TreeType::Distance => &mut self.dyn_dtree,
            TreeType::BitLength => &mut self.bl_tree,
        }
    }

    // 根据树类型获取对应的静态树的可变引用
    fn get_static_tree_mut(&mut self, tree_type: TreeType) -> &mut [CtData] {
        match tree_type {
            TreeType::Literal => &mut self.static_ltree,
            TreeType::Distance => &mut self.static_dtree,
            TreeType::BitLength => &mut self.bl_tree,  // BitLength 树没有静态版本
        }
    }

    pub(crate) fn ct_init(&mut self, attr: &'a mut u16, methodp: i32) {
        let mut n: i32;
        let mut length: i32;
        let mut code: i32;
        let mut dist: i32;

        self.file_type = Some(attr);
        self.file_method = methodp;
        self.compressed_len = 0;
        self.input_len = 0;

        if self.static_dtree[0].len != 0 {
            return; // ct_init already called
        }

        // Initialize the mapping length (0..255) -> length code (0..28)
        length = 0;
        code = 0;
        while code < (LENGTH_CODES - 1) as i32 {
            self.base_length[code as usize] = length;
            n = 0;
            while n < (1 << EXTRA_LBITS[code as usize]) {
                self.length_code[length as usize] = code as u8;
                length += 1;
                n += 1;
            }
            code += 1;
        }
        assert!(length == 256, "ct_init: length != 256");

        // Overwrite length_code[255] to use the best encoding
        self.length_code[(length - 1) as usize] = code as u8;

        // Initialize the mapping dist (0..32K) -> dist code (0..29)
        dist = 0;
        code = 0;
        while code < 16 {
            self.base_dist[code as usize] = dist;
            n = 0;
            while n < (1 << EXTRA_DBITS[code as usize]) {
                self.dist_code[dist as usize] = code as u8;
                dist += 1;
                n += 1;
            }
            code += 1;
        }
        assert!(dist == 256, "ct_init: dist != 256");

        dist >>= 7; // From now on, all distances are divided by 128
        while code < D_CODES as i32 {
            self.base_dist[code as usize] = dist << 7;
            n = 0;
            while n < (1 << (EXTRA_DBITS[code as usize] - 7)) {
                self.dist_code[(256 + dist) as usize] = code as u8;
                dist += 1;
                n += 1;
            }
            code += 1;
        }
        assert!(dist == 256, "ct_init: 256+dist != 512");

        // Construct the codes of the static literal tree
        for bits in 0..=MAX_BITS as i32 {
            self.bl_count[bits as usize] = 0;
        }

        n = 0;
        while n <= 143 {
            self.static_ltree[n as usize].len = 8;
            self.bl_count[8] += 1;
            n += 1;
        }
        while n <= 255 {
            self.static_ltree[n as usize].len = 9;
            self.bl_count[9] += 1;
            n += 1;
        }
        while n <= 279 {
            self.static_ltree[n as usize].len = 7;
            self.bl_count[7] += 1;
            n += 1;
        }
        while n <= 287 {
            self.static_ltree[n as usize].len = 8;
            self.bl_count[8] += 1;
            n += 1;
        }

        // Generate the codes
        self.l_desc.max_code = (L_CODES+1) as i32;
        self.gen_codes(TreeType::Literal, TreeKind::Static);

        // The static distance tree is trivial
        for n in 0..D_CODES as i32 {
            self.static_dtree[n as usize].len = 5;
            self.static_dtree[n as usize].code = Self::bi_reverse(n as u16, 5);
        }

        // Initialize the first block of the first file
        self.init_block();
    }

    fn init_block(&mut self) {
        // Initialize the dynamic literal tree frequencies
        for n in 0..L_CODES {
            self.dyn_ltree[n].freq = 0;
        }

        // Initialize the dynamic distance tree frequencies
        for n in 0..D_CODES {
            self.dyn_dtree[n].freq = 0;
        }

        // Initialize the bit length tree frequencies
        for n in 0..BL_CODES {
            self.bl_tree[n].freq = 0;
        }

        // Set the frequency of the END_BLOCK symbol to 1
        self.dyn_ltree[END_BLOCK].freq = 1;

        // Reset all counters and flags
        self.opt_len = 0;
        self.static_len = 0;
        self.last_lit = 0;
        self.last_dist = 0;
        self.last_flags = 0;
        self.flags = 0;
        self.flag_bit = 1;
    }

    // 修改 gen_codes 为实例方法
    fn gen_codes(&mut self, tree_type: TreeType, kind: TreeKind) {
        // 根据树类型和种类选择相应的树和最大代码值
        let (tree, max_code) = match (tree_type, kind) {
            // 静态树
            (TreeType::Literal, TreeKind::Static) => (&mut self.static_ltree, (L_CODES + 1) as i32),
            (TreeType::Distance, TreeKind::Static) => (&mut self.static_dtree, D_CODES as i32),
            
            // 动态树
            (TreeType::Literal, TreeKind::Dynamic) => (&mut self.dyn_ltree, self.l_desc.max_code),
            (TreeType::Distance, TreeKind::Dynamic) => (&mut self.dyn_dtree, self.d_desc.max_code),
            (TreeType::BitLength, _) => (&mut self.bl_tree, self.bl_desc.max_code),
        };

        let mut next_code = [0u16; MAX_BITS + 1];
        let mut code = 0u16;

        // println!("\n=== Starting gen_codes ===");
        // println!("tree_type: {:?}, max_code: {}", tree_type, max_code);

        // 生成 next_code 数组
        for bits in 1..=MAX_BITS {
            code = ((code + self.bl_count[bits - 1] as u16) << 1) as u16;
            next_code[bits] = code;
            // println!("bits: {}, bl_count[{}]: {}, code: {}, next_code[{}]: {}", 
            //     bits, 
            //     bits-1, 
            //     self.bl_count[bits - 1], 
            //     code, 
            //     bits, 
            //     next_code[bits]
            // );
        }
        
        // println!("\n=== Assigning codes to tree nodes ===");
        // 为树节点分配编码
        for n in 0..=max_code as usize {
            let len = tree[n].len as usize;
            // println!("node: {}, len: {}", n, len);
            if len != 0 {
                let reversed = Self::bi_reverse(next_code[len], len);
                // println!("  code before reverse: {}, after reverse: {}", next_code[len], reversed);
                tree[n].code = reversed;
                next_code[len] += 1;
                // println!("  updated next_code[{}]: {}", len, next_code[len]);
            }
        }
        // println!("=== Finished gen_codes ===\n");
    }


    fn bi_reverse(code: u16, len: usize) -> u16 {
        let mut code = code;
        let mut res = 0u16;
        for _ in 0..len {
            res = (res << 1) | (code & 1);
            code >>= 1;
        }
        res
    }

    pub fn ct_tally(&mut self, deflate: &mut Deflate, state: &mut GzipState, dist: usize, lc: usize) -> bool {
        let mut dist = dist;
        let mut lc = lc;
        // println!("last_lit: {}", self.last_lit);
        // println!("dist: {}", dist);
        // println!("lc: {}", lc);
        
        // Add the character or match length to the literal buffer
        state.inbuf[self.last_lit as usize] = lc as u8 ;
        // self.l_buf[self.last_lit as usize] = lc as usize ;

        self.last_lit += 1;
        
        if dist == 0 {
            // lc is the unmatched character (literal)
            self.dyn_ltree[lc].freq += 1;
            // println!("literal: {} freq: {}", lc, self.dyn_ltree.borrow_mut()[lc].freq);
        } else {
            // lc is the match length - MIN_MATCH
            dist = dist - 1; // Adjust distance
            assert!(
                dist < MAX_DIST
                    && lc <= MAX_MATCH - MIN_MATCH
                    && self.d_code(dist) < D_CODES,
                "ct_tally: bad match"
            );

            // 先计算所有需要的索引
            let length_index = self.length_code[lc] as usize + LITERALS + 1;
            let dist_index = self.d_code(dist);

            // 然后一次性更新频率
            self.dyn_ltree[length_index].freq += 1;
            self.dyn_dtree[dist_index].freq += 1;

            self.d_buf[self.last_dist as usize] = dist as u16 as usize;
            self.last_dist += 1;
            self.flags |= self.flag_bit;
            // println!("dist: {}", dist);
            // println!("lidx:{:?} dyn_ltree: {:?}", self.length_code[lc] as usize + LITERALS + 1, self.dyn_ltree[self.length_code[lc] as usize + LITERALS + 1].freq);
            // println!("didx:{:?} dyn_dtree: {:?}", self.d_code(dist), self.dyn_dtree[self.d_code(dist)].freq);
            
        }

        self.flag_bit <<= 1;

        // Output the flags if they fill a byte
        if (self.last_lit & 7) == 0 {
            self.flag_buf[self.last_flags as usize] = self.flags as usize;
            self.last_flags += 1;
            self.flags = 0;
            self.flag_bit = 1;
        }


        // Try to guess if it is profitable to stop the current block here
        if state.level > 2 && (self.last_lit & 0xfff) == 0 {
            // Compute an upper bound for the compressed length
            let mut out_length = self.last_lit as u64 * 8;
            let in_length = deflate.strstart - deflate.block_start as usize;

            for dcode in 0..D_CODES {
                out_length += self.dyn_dtree[dcode].freq as u64
                    * (5 + EXTRA_DBITS[dcode] as u64);
            }

            out_length >>= 3; // Divide by 8

            if state.verbose > 0 {
                eprintln!(
                    "\nlast_lit {}, last_dist {}, in {}, out ~{}({}%)",
                    self.last_lit,
                    self.last_dist,
                    in_length,
                    out_length,
                    100 - out_length * 100 / in_length as u64
                );
            }

            if self.last_dist < self.last_lit / 2 && out_length < (in_length / 2) as u64 {
                return true;
            }
        }


        // Return true if the buffer is full
        self.last_lit == (LIT_BUFSIZE - 1) as i32 || self.last_dist == DIST_BUFSIZE as i32
    }

    fn d_code(&self, dist: usize) -> usize {
        if dist < 256 {
            self.dist_code[dist] as usize
        } else {
            self.dist_code[256 + (dist >> 7)] as usize
        }
    }

    pub(crate) fn flush_block(
        &mut self,
        state: &mut GzipState,
        buf: Option<&[u8]>,
        stored_len: u64,
        eof: bool,
    ) -> i64 {
        let mut opt_lenb: u64;
        let static_lenb: u64;
        let max_blindex: i32;
        // println!("flush_block");

        // Save the flags for the last 8 items
        self.flag_buf[self.last_flags as usize] = self.flags as usize;

        // Check if the file is ASCII or binary
        if self.file_type == None {
            self.set_file_type();
        }
        // println!("flush_block: stored_len: {}", stored_len);
        // Special handling for empty files
        if stored_len == 0 && eof {
            // Use stored block format for empty files
            state.send_bits((STORED_BLOCK << 1) as u16 + (if eof { 1 } else { 0 }) as u16, 3);
            self.compressed_len = 0;
            self.file_method = STORED;
            return 0; // Return compressed length (0 for empty file)
        }
        // println!("flush_block: stored_len: {}", stored_len);
        
        // Construct the literal and distance trees
        self.build_tree(state,  TreeType::Literal);
        // if state.verbose > 1 {
        // if true {
        //     eprintln!(
        //         "\nlit data: dyn {}, stat {}",
        //         self.opt_len, self.static_len
        //     );
        // }

        self.build_tree(state, TreeType::Distance);
        // if state.verbose > 1 {
        // if true {
        //     eprintln!(
        //         "\ndist data: dyn {}, stat {}",
        //         self.opt_len, self.static_len
        //     );
        // }

        // Build the bit length tree and get the index of the last bit length code to send
        max_blindex = self.build_bl_tree(state);
        // println!("max_blindex: {}", max_blindex);

        // Determine the best encoding. Compute the block length in bytes
        opt_lenb = (self.opt_len + 3 + 7) >> 3;
        static_lenb = (self.static_len.wrapping_add(3 + 7)) >> 3;
        self.input_len += stored_len; // For debugging only

        if state.verbose > 0 {
            eprintln!(
                "\nopt {}({}) stat {}({}) stored {} lit {} dist {}",
                opt_lenb,
                self.opt_len,
                static_lenb,
                self.static_len,
                stored_len,
                self.last_lit,
                self.last_dist
            );
        }

        if static_lenb <= opt_lenb {
            opt_lenb = static_lenb;
        }

        fn seekable() -> bool {
            false // 确保返回 false
        }

        if stored_len <= opt_lenb && eof && self.compressed_len == 0 && seekable() {
            // Since LIT_BUFSIZE <= 2*WSIZE, the input data must be there
            if buf.is_none() {
                state.gzip_error("block vanished");
            }

            self.copy_block(state, buf.unwrap(), stored_len as usize, false); // Without header
            self.compressed_len = stored_len << 3;
            self.file_method = STORED as i32;
        } else if stored_len + 4 <= opt_lenb && buf.is_some() {
            // 4: two words for the lengths
            let eof_flag = if eof { 1 } else { 0 };
            state.send_bits(((STORED_BLOCK << 1) + eof_flag) as u16, 3); // Send block type
            self.compressed_len = (self.compressed_len + 3 + 7) & !7u64;
            self.compressed_len += (stored_len + 4) << 3;

            self.copy_block(state, buf.unwrap(), stored_len as usize, true); // With header
        } else if static_lenb == opt_lenb {
            let eof_flag = if eof { 1 } else { 0 };
            state.send_bits(((STATIC_TREES << 1) + eof_flag) as u16, 3);
            self.compress_block(state, true);  
            self.compressed_len += 3 + self.static_len;
        } else {
            // println!("lbf");
            let eof_flag = if eof { 1 } else { 0 };
            state.send_bits(((DYN_TREES << 1) + eof_flag) as u16, 3);
            self.send_all_trees(
                state,
                (self.l_desc.max_code + 1) as usize,
                (self.d_desc.max_code + 1) as usize,
                (max_blindex + 1) as usize,
            );
            self.compress_block(state, false); 
            self.compressed_len += 3 + self.opt_len;
        }

        self.init_block();

        if eof {
            //assert!(self.input_len as i64 == state.bytes_in, "bad input size");
            state.bi_windup();
            self.compressed_len = self.compressed_len.wrapping_add(7); // Align on byte boundary
        }

        (self.compressed_len >> 3) as i64
    }

    /// Send the header for a block using dynamic Huffman trees:
    /// the counts, the lengths of the bit length codes, the literal tree, and the distance tree.
    /// IN assertion: lcodes >= 257, dcodes >= 1, blcodes >= 4.
    fn send_all_trees(&mut self, state: &mut GzipState, lcodes: usize, dcodes: usize, blcodes: usize) {
        // Assertions to ensure we have the correct number of codes
        assert!(
            lcodes >= 257 && dcodes >= 1 && blcodes >= 4,
            "not enough codes"
        );
        assert!(
            lcodes <= L_CODES && dcodes <= D_CODES && blcodes <= BL_CODES,
            "too many codes"
        );

        // Optional debugging output
        if state.verbose > 1 {
            eprintln!("\nbl counts:");
        }

        // Send the number of literal codes, distance codes, and bit length codes
        state.send_bits((lcodes - 257) as u16, 5); // lcodes - 257 in 5 bits
        state.send_bits((dcodes - 1) as u16, 5);   // dcodes - 1 in 5 bits
        state.send_bits((blcodes - 4) as u16, 4);  // blcodes - 4 in 4 bits

        // Send the bit length codes in the order specified by bl_order
        for rank in 0..blcodes {
            let bl_code = BL_ORDER[rank];

            if state.verbose > 1 {
                eprintln!("\nbl code {:2}", bl_code);
            }

            // Send the bit length for the current code in 3 bits
            state.send_bits(self.bl_tree[bl_code].len as u16, 3);
        }

        // Send the literal tree
        self.send_tree(state, TreeType::Literal);

        // Send the distance tree
        self.send_tree(state, TreeType::Distance);
    }

    fn send_tree(&mut self, state: &mut GzipState, tree_type: TreeType) {
        // 根据树类型选择相应的树和最大代码值
        let (tree, max_code) = match tree_type {
            TreeType::Literal => (&self.dyn_ltree, self.l_desc.max_code as usize),
            TreeType::Distance => (&self.dyn_dtree, self.d_desc.max_code as usize),
            TreeType::BitLength => (&self.bl_tree, self.bl_desc.max_code as usize),
        };

        let mut prevlen: i32 = -1; // Last emitted length
        let mut curlen: i32; // Length of current code
        let mut nextlen: i32 = tree[0].len as i32; // Length of next code
        let mut count: i32 = 0; // Repeat count of the current code length
        let mut max_count: i32 = 7; // Max repeat count
        let mut min_count: i32 = 4; // Min repeat count

        // If the first code length is zero, adjust max and min counts
        if nextlen == 0 {
            max_count = 138;
            min_count = 3;
        }

        for n in 0..=max_code {
            curlen = nextlen;
            if n + 1 <= max_code {
                nextlen = tree[n + 1].len as i32;
            } else {
                nextlen = -1;
            }

            count += 1;

            if count < max_count && curlen == nextlen {
                continue;
            } else {
                if count < min_count {
                    // Send the code 'count' times
                    for _ in 0..count {
                        self.send_code(state, curlen as usize, &self.bl_tree);
                    }
                } else if curlen != 0 {
                    if curlen != prevlen {
                        self.send_code(state, curlen as usize, &self.bl_tree);
                        count -= 1;
                    }
                    assert!(
                        count >= 3 && count <= 6,
                        "Invalid count for REP_3_6: count = {}",
                        count
                    );
                    self.send_code(state, REP_3_6, &self.bl_tree);
                    state.send_bits((count - 3) as u16, 2);
                } else if count <= 10 {
                    self.send_code(state, REPZ_3_10, &self.bl_tree);
                    state.send_bits((count - 3) as u16, 3);
                } else {
                    self.send_code(state, REPZ_11_138, &self.bl_tree);
                    state.send_bits((count - 11) as u16, 7);
                }

                count = 0;
                prevlen = curlen;

                if nextlen == 0 {
                    max_count = 138;
                    min_count = 3;
                } else if curlen == nextlen {
                    max_count = 6;
                    min_count = 3;
                } else {
                    max_count = 7;
                    min_count = 4;
                }
            }
        }
    }        
    


    fn set_file_type(&mut self) {
        let mut n = 0;
        let mut ascii_freq: u32 = 0;
        let mut bin_freq: u32 = 0;

        while n < 7 {
            bin_freq += self.dyn_ltree[n].freq as u32;
            n += 1;
        }
        while n < 128 {
            ascii_freq += self.dyn_ltree[n].freq as u32;
            n += 1;
        }
        while n < LITERALS {
            bin_freq += self.dyn_ltree[n].freq as u32;
            n += 1;
        }

        **self.file_type.as_mut().unwrap() = if bin_freq > (ascii_freq >> 2) {
            BINARY
        } else {
            ASCII
        };
    }

    fn warning(&self, msg: &str) {
        eprintln!("Warning: {}", msg);
    }

    // 修改 compress_block 的签名，使用 TreeType 来指定使用哪个树
    fn compress_block(&mut self, state: &mut GzipState, use_static: bool) {
        let (ltree, dtree) = if use_static {
            (&self.static_ltree, &self.static_dtree)
        } else {
            (&self.dyn_ltree, &self.dyn_dtree)
        };

        let mut dist: u32;      // 匹配字符串的距离
        let mut lc: i32;        // 匹配长度或未匹配字符(如果 dist == 0)
        let mut lx: usize = 0;  // l_buf 的运行索引
        let mut dx: usize = 0;  // d_buf 的运行索引
        let mut fx: usize = 0;  // flag_buf 的运行索引
        let mut flag: u8 = 0;   // 当前标志
        let mut code: usize;    // 要发送的代码
        let mut extra: u8;      // 要发送的额外位数

        // 检查是否有任何字面值要处理
        if self.last_lit != 0 {
            while lx < self.last_lit as usize {
                // 每8个字面值加载一个新的标志字节
                if (lx & 7) == 0 {
                    flag = self.flag_buf[fx] as u8;
                    fx += 1;
                }

                lc = state.inbuf[lx] as i32;
                lx += 1;

                if (flag & 1) == 0 {
                    // 发送字面字节
                    self.send_code(state, lc as usize, ltree);
                } else {
                    // 这是一个匹配
                    let lc_usize = lc as usize;
                    code = self.length_code[lc_usize] as usize;
                    self.send_code(state, code + LITERALS + 1, ltree); // 发送长度代码
                    extra = EXTRA_LBITS[code] as u8;

                    if extra != 0 {
                        let base_len = self.base_length[code] as i32;
                        let lc_adjusted = lc - base_len;
                        state.send_bits(lc_adjusted as u16, extra); // 发送额外的长度位
                    }

                    dist = self.d_buf[dx] as u32;
                    dx += 1;

                    // dist 是匹配距离减1
                    code = self.d_code(dist as usize);
                    assert!(code < D_CODES, "bad d_code");

                    self.send_code(state, code, dtree); // 发送距离代码
                    extra = EXTRA_DBITS[code] as u8;

                    if extra != 0 {
                        let base_dist = self.base_dist[code] as u32;
                        let dist_adjusted = dist - base_dist;
                        state.send_bits(dist_adjusted as u16, extra); // 发送额外的距离位
                    }
                }

                flag >>= 1; // 移动到下一个标志位
            }
        }

        // 发送块结束代码
        self.send_code(state, END_BLOCK, ltree);
    }    
    


    fn send_code(&self, state: &mut GzipState, c: usize, tree: &[CtData]) {
        // Debugging output if verbose > 1
        if state.verbose > 1 {
            eprintln!("\ncd {:3}", c);
        }

         // Output the code and its length in hexadecimal
        let code = tree[c].code;
        let length = tree[c].len;

//         eprintln!("Code: {:X}, Length: {}", code, length);

        // Send the code and its length using the send_bits function
        state.send_bits(code, length as u8);

    }

    fn copy_block(&mut self, state: &mut GzipState, buf: &[u8], len: usize, header: bool) {
        // Align on byte boundary
        state.bi_windup();

        if header {
            state.put_short(len as u16);
            state.put_short(!len as u16);
        }

        // Iterate over the buffer and output each byte
        // If encryption is needed, handle it here
        for &byte in buf.iter().take(len) {
            #[cfg(feature = "encryption")]
            {
                // Placeholder for encryption logic
                let encrypted_byte = if self.key.is_some() {
                    self.zencode(byte)
                } else {
                    byte
                };
                self.put_byte(encrypted_byte);
            }
            #[cfg(not(feature = "encryption"))]
            {
                state.put_byte(byte).expect("Failed");
            }
        }
    }
         
    fn build_tree(&mut self, state: &GzipState, tree_type: TreeType) {
        // 先获取必要的常量值
        let elems = match tree_type {
            TreeType::Literal => self.l_desc.elems,
            TreeType::Distance => self.d_desc.elems,
            TreeType::BitLength => self.bl_desc.elems,
        };

        let mut max_code = -1;
        let mut node = elems;  // 下一个内部节点的索引
        // println!("elems={:?}",elems);

        // 构建初始堆，频率最小的元素在 heap[SMALLEST]
        self.heap_len = 0;
        self.heap_max = HEAP_SIZE;

        // 遍历树节点，初始化堆
        for n in 0..elems {
            let freq = match tree_type {
                TreeType::Literal => self.dyn_ltree[n].freq,
                TreeType::Distance => self.dyn_dtree[n].freq,
                TreeType::BitLength => self.bl_tree[n].freq,
            };

            if freq != 0 {
                self.heap_len += 1;
                max_code = n as i32;
                self.heap[self.heap_len] = n as i32;
                self.depth[n] = 0;
                // println!("n={:?} hplen={:?}",n,self.heap_len);
            } else {
                // 设置长度为0
                match tree_type {
                    TreeType::Literal => self.dyn_ltree[n].len = 0,
                    TreeType::Distance => self.dyn_dtree[n].len = 0,
                    TreeType::BitLength => self.bl_tree[n].len = 0,
                }
            }
        }

        // 确保至少有两个非零频率的码
        while self.heap_len < 2 {
            let new_node = if max_code < 2 {
                max_code += 1;
                max_code
            } else {
                0
            } as usize;
            
            self.heap_len += 1;
            self.heap[self.heap_len] = new_node as i32;
            
            // 更新频率和深度
            match tree_type {
                TreeType::Literal => self.dyn_ltree[new_node].freq = 1,
                TreeType::Distance => self.dyn_dtree[new_node].freq = 1,
                TreeType::BitLength => self.bl_tree[new_node].freq = 1,
            }
            self.depth[new_node] = 0;
            self.opt_len = self.opt_len.wrapping_sub(1);
            
            // 如果是字面树或距离树，更新静态长度
            if tree_type != TreeType::BitLength {
                let static_len = match tree_type {
                    TreeType::Literal => self.static_ltree[new_node].len,
                    TreeType::Distance => self.static_dtree[new_node].len,
                    TreeType::BitLength => 0,
                };
                self.static_len = self.static_len.wrapping_sub(static_len as u64);
            }
        }

        // 更新最大代码值
        match tree_type {
            TreeType::Literal => self.l_desc.max_code = max_code,
            TreeType::Distance => self.d_desc.max_code = max_code,
            TreeType::BitLength => self.bl_desc.max_code = max_code,
        }
        // println!("upd={:?} upd_l={:?}",max_code, self.l_desc.max_code);

        // 堆的元素 heap[heap_len/2+1 .. heap_len] 是叶子节点
        // 建立子堆
        for n in (1..=(self.heap_len / 2)).rev() {
            self.pq_down_heap(tree_type, n);
        }

        // 通过重复组合频率最小的两个节点来构建霍夫曼树
        while self.heap_len >= 2 {
            // 移除堆中频率最小的两个节点
            let n = self.pq_remove(tree_type) as usize;
            let m = self.heap[SMALLEST] as usize;

            self.heap_max -= 1;
            self.heap[self.heap_max] = n as i32;
            self.heap_max -= 1;
            self.heap[self.heap_max] = m as i32;

            // 创建新节点作为它们的父节点
            let freq_sum = match tree_type {
                TreeType::Literal => self.dyn_ltree[n].freq + self.dyn_ltree[m].freq,
                TreeType::Distance => self.dyn_dtree[n].freq + self.dyn_dtree[m].freq,
                TreeType::BitLength => self.bl_tree[n].freq + self.bl_tree[m].freq,
            };

            // 更新新节点的频率和深度
            match tree_type {
                TreeType::Literal => {
                    self.dyn_ltree[node].freq = freq_sum;
                    self.dyn_ltree[n].dad = node as u16;
                    self.dyn_ltree[m].dad = node as u16;
                },
                TreeType::Distance => {
                    self.dyn_dtree[node].freq = freq_sum;
                    self.dyn_dtree[n].dad = node as u16;
                    self.dyn_dtree[m].dad = node as u16;
                },
                TreeType::BitLength => {
                    self.bl_tree[node].freq = freq_sum;
                    self.bl_tree[n].dad = node as u16;
                    self.bl_tree[m].dad = node as u16;
                },
            }

            self.depth[node] = (self.depth[n].max(self.depth[m]) + 1) as i32;

            // 将新节点放入堆中
            self.heap[SMALLEST] = node as i32;
            self.pq_down_heap(tree_type, SMALLEST);

            node += 1;
        }

        self.heap_max -= 1;
        self.heap[self.heap_max] = self.heap[SMALLEST];

        // 生成位长度
        self.gen_bitlen(state, tree_type);

        // 生成所有树节点的编码
        self.gen_codes(tree_type, TreeKind::Dynamic);
    }       
    


    /// Remove the smallest element from the heap and adjust the heap.
    /// Returns the index of the smallest node.
    fn pq_remove(&mut self, tree_type: TreeType) -> usize {
        // The smallest item is at the root of the heap
        let top = self.heap[SMALLEST];

        // Move the last item to the root and reduce the heap size
        self.heap[SMALLEST] = self.heap[self.heap_len];
        self.heap_len -= 1;

        // Restore the heap property by moving down from the root
        self.pq_down_heap(tree_type, SMALLEST);

        top as usize // Return the index of the smallest node
    }

    /// Compute the optimal bit lengths for a tree and update the total bit length
    /// for the current block.
    /// IN assertion: the fields freq and dad are set, heap[heap_max] and
    /// above are the tree nodes sorted by increasing frequency.
    /// OUT assertions: the field len is set to the optimal bit length, the
    /// array bl_count contains the frequencies for each bit length.
    /// The length opt_len is updated; static_len is also updated if stree is
    /// not null.
    fn gen_bitlen(&mut self, state: &GzipState, tree_type: TreeType) {
        // 先获取所需的基本信息
        let (max_code, max_length, extra_base) = match tree_type {
            TreeType::Literal => (self.l_desc.max_code, self.l_desc.max_length, self.l_desc.extra_base),
            TreeType::Distance => (self.d_desc.max_code, self.d_desc.max_length, self.d_desc.extra_base),
            TreeType::BitLength => (self.bl_desc.max_code, self.bl_desc.max_length, self.bl_desc.extra_base),
        };

        let mut overflow = 0;

        // 初始化 bl_count
        for bits in 0..=MAX_BITS {
            self.bl_count[bits] = 0;
        }

        // 在第一遍中，计算最优位长度
        let heap_max_idx = self.heap[self.heap_max] as usize;
        // println!("hpidx={:?} hpmx={:?}",heap_max_idx, self.heap_max);
        match tree_type {
            TreeType::Literal => self.dyn_ltree[heap_max_idx].len = 0,
            TreeType::Distance => self.dyn_dtree[heap_max_idx].len = 0,
            TreeType::BitLength => self.bl_tree[heap_max_idx].len = 0,
        }

        // 第一遍：计算最优位长度
        for h in (self.heap_max + 1)..HEAP_SIZE {
            let n = self.heap[h] as usize;
            
            // 获取父节点的长度并加1
            let dad_len = match tree_type {
                TreeType::Literal => self.dyn_ltree[self.dyn_ltree[n].dad as usize].len,
                TreeType::Distance => self.dyn_dtree[self.dyn_dtree[n].dad as usize].len,
                TreeType::BitLength => self.bl_tree[self.bl_tree[n].dad as usize].len,
            };
            let mut bits = dad_len + 1;

            // 检查是否超过最大长度
            if bits > max_length as u16 {
                bits = max_length as u16;
                overflow += 1;
            }

            // 设置节点的长度
            match tree_type {
                TreeType::Literal => self.dyn_ltree[n].len = bits,
                TreeType::Distance => self.dyn_dtree[n].len = bits,
                TreeType::BitLength => self.bl_tree[n].len = bits,
            }

            // 如果不是叶子节点，继续
            if n > max_code as usize {
                continue;
            }

            // 更新计数和长度
            self.bl_count[bits as usize] += 1;
            
            // 计算额外位
            let mut xbits = 0;
            if n >= extra_base {
                xbits = match tree_type {
                    TreeType::Literal => EXTRA_LBITS[n - extra_base],
                    TreeType::Distance => EXTRA_DBITS[n - extra_base],
                    TreeType::BitLength => EXTRA_BLBITS[n - extra_base],
                };
            }

            // 获取频率
            let freq = match tree_type {
                TreeType::Literal => self.dyn_ltree[n].freq,
                TreeType::Distance => self.dyn_dtree[n].freq,
                TreeType::BitLength => self.bl_tree[n].freq,
            } as u64;

            // 更新优化长度
            self.opt_len += freq * (bits as u64 + xbits as u64);

            // 更新静态长度（如果不是位长度树）
            if tree_type != TreeType::BitLength {
                let static_len = match tree_type {
                    TreeType::Literal => self.static_ltree[n].len,
                    TreeType::Distance => self.static_dtree[n].len,
                    TreeType::BitLength => 0,
                } as u64;
                self.static_len += freq * (static_len + xbits as u64);
            }
        }

        if overflow == 0 {
            return;
        }

        // 处理溢出情况
        if state.verbose > 0 {
            eprintln!("\nbit length overflow");
        }

        // 调整溢出的位长度
        loop {
            let mut bits = max_length as usize - 1;
            while self.bl_count[bits] == 0 {
                bits -= 1;
            }
            self.bl_count[bits] -= 1;      // 将一个叶子节点下移
            self.bl_count[bits + 1] += 2;  // 将一个溢出项作为其兄弟移动
            self.bl_count[max_length as usize] -= 1;
            overflow -= 2;
            
            if overflow <= 0 {
                break;
            }
        }

        // 重新计算所有位长度
        let mut h = HEAP_SIZE;
        for bits in (1..=max_length as usize).rev() {
            let mut n = self.bl_count[bits];
            while n > 0 {
                h -= 1;
                let m = self.heap[h] as usize;
                if m > max_code as usize {
                    continue;
                }

                let current_len = match tree_type {
                    TreeType::Literal => self.dyn_ltree[m].len,
                    TreeType::Distance => self.dyn_dtree[m].len,
                    TreeType::BitLength => self.bl_tree[m].len,
                };

                if current_len != bits as u16 {
                    if state.verbose > 1 {
                        eprintln!("code {} bits {}->{}", m, current_len, bits);
                    }
                    
                    // 更新优化长度
                    let freq = match tree_type {
                        TreeType::Literal => self.dyn_ltree[m].freq,
                        TreeType::Distance => self.dyn_dtree[m].freq,
                        TreeType::BitLength => self.bl_tree[m].freq,
                    } as u64;
                    self.opt_len += ((bits as i64 - current_len as i64) * freq as i64) as u64;
                    
                    // 更新长度
                    match tree_type {
                        TreeType::Literal => self.dyn_ltree[m].len = bits as u16,
                        TreeType::Distance => self.dyn_dtree[m].len = bits as u16,
                        TreeType::BitLength => self.bl_tree[m].len = bits as u16,
                    }
                }
                n -= 1;
            }
        }
    }


    /// Adjust bit lengths to eliminate overflow
    fn adjust_bit_lengths(&mut self, state: &GzipState, mut overflow: i32, max_length: i32) {
        // This happens for example on obj2 and pic of the Calgary corpus
        if state.verbose > 0 {
            eprintln!("\nbit length overflow");
        }

        // Find the first bit length which could increase
        loop {
            let mut bits = max_length - 1;
            while self.bl_count[bits as usize] == 0 {
                bits -= 1;
            }

            // Decrease count of bit length `bits`
            self.bl_count[bits as usize] -= 1;

            // Increase count of bit length `bits + 1` by 2
            self.bl_count[(bits + 1) as usize] += 2;

            // Decrease count of bit length `max_length`
            self.bl_count[max_length as usize] -= 1;

            overflow -= 2;

            if overflow <= 0 {
                break;
            }
        }
    }

    /// Recompute all bit lengths, scanning in increasing frequency
    fn recompute_bit_lengths(&mut self, state: &GzipState, tree: &mut [CtData], max_code: i32, max_length: i32) {
        let mut h = self.heap_len as usize;
        // Start from the largest bit length
        for bits in (1..=max_length).rev() {
            let n = self.bl_count[bits as usize];
            for _ in 0..n {
                h -= 1;
                let m = self.heap[h] as usize;

                if m > max_code as usize {
                    continue;
                }

                if tree[m].len != bits as u16 {
                    if state.verbose > 1 {
                        eprintln!(
                            "code {} bits {}->{}",
                            m,
                            tree[m].len,
                            bits
                        );
                    }
                    let freq = tree[m].freq as u64;
                    self.opt_len += (bits as u64 - tree[m].len as u64) * freq;
                    tree[m].len = bits as u16;
                }
            }
        }
    }

    /// Restore the heap property by moving down the tree starting at node `k`,
    /// exchanging a node with the smallest of its two children if necessary,
    /// stopping when the heap property is re-established (each parent smaller than its two children).
    fn pq_down_heap(&mut self, tree_type: TreeType, mut k: usize) {
        let v = self.heap[k];
        let mut j = k << 1;

        while j <= self.heap_len {
            if j < self.heap_len && self.smaller(tree_type, self.heap[j + 1] as usize, self.heap[j] as usize) {
                j += 1;
            }

            if self.smaller(tree_type, v as usize, self.heap[j] as usize) {
                break;
            }

            self.heap[k] = self.heap[j];
            k = j;
            j <<= 1;
        }

        self.heap[k] = v;
    }

    /// Compare two nodes in the heap based on frequencies and depths.
    /// Returns true if node `n` is "smaller" than node `m`.
    fn smaller(&self, tree_type: TreeType, n: usize, m: usize) -> bool {
        let (freq_n, freq_m) = match tree_type {
            TreeType::Literal => (self.dyn_ltree[n].freq, self.dyn_ltree[m].freq),
            TreeType::Distance => (self.dyn_dtree[n].freq, self.dyn_dtree[m].freq),
            TreeType::BitLength => (self.bl_tree[n].freq, self.bl_tree[m].freq),
        };

        freq_n < freq_m || (freq_n == freq_m && self.depth[n] <= self.depth[m])
    }

    fn build_bl_tree(&mut self, state: &GzipState) -> i32 {
        let mut max_blindex: i32;

        // Determine the bit length frequencies for literal and distance trees
        self.scan_tree(TreeType::Literal);
        self.scan_tree(TreeType::Distance);
        // Build the bit length tree
        self.build_tree(state,TreeType::BitLength);

        // At this point, opt_len includes the length of the tree representations,
        // except the lengths of the bit lengths codes and the 5+5+4 bits for the counts.

        // Determine the number of bit length codes to send.
        // The PKZIP format requires that at least 4 bit length codes be sent.
        max_blindex = (BL_CODES - 1) as i32;
        while max_blindex >= 3 {
            let code = BL_ORDER[max_blindex as usize];
            if self.bl_tree[code].len != 0 {
                break;
            }
            max_blindex -= 1;
        }

        // Update opt_len to include the bit length tree and counts
        self.opt_len = self.opt_len.wrapping_add(3 * ((max_blindex as u64) + 1) + 5 + 5 + 4);

        if state.verbose > 1 {
            eprintln!("\ndyn trees: dyn {}, stat {}", self.opt_len, self.static_len);
        }

        max_blindex
    }

    fn scan_tree(&mut self, tree_type: TreeType) {
        // 先获取需要的值，避免后续重复借用
        let (tree_data, max_code) = match tree_type {
            TreeType::Literal => (&self.dyn_ltree[..], self.l_desc.max_code),
            TreeType::Distance => (&self.dyn_dtree[..], self.d_desc.max_code),
            TreeType::BitLength => (&self.bl_tree[..], self.bl_desc.max_code),
        };

        // 创建一个临时数组来存储需要的长度值
        let mut lengths: Vec<u16> = tree_data.iter().map(|node| node.len).collect();
        
        let mut prevlen: i32 = -1;           // Last emitted length
        let mut curlen: i32;                 // Length of current code
        let mut nextlen: i32 = lengths[0] as i32; // Length of next code
        let mut count: i32 = 0;              // Repeat count of the current code
        let mut max_count: i32;              // Max repeat count
        let mut min_count: i32;              // Min repeat count

        if nextlen == 0 {
            max_count = 138;
            min_count = 3;
        } else {
            max_count = 7;
            min_count = 4;
        }

        // Set a guard value
        if (max_code + 1) as usize >= lengths.len() {
            panic!("Tree array is too small");
        }

        for n in 0..=max_code {
            let n = n as usize;
            curlen = nextlen;
            nextlen = if n + 1 <= max_code as usize {
                lengths[n + 1] as i32
            } else {
                0
            };

            count += 1;

            if count < max_count && curlen == nextlen {
                continue;
            } else {
                if count < min_count {
                    // Update the frequency for the current code length
                    self.bl_tree[curlen as usize].freq += count as u16;
                } else if curlen != 0 {
                    if curlen != prevlen {
                        self.bl_tree[curlen as usize].freq += 1;
                    }
                    self.bl_tree[REP_3_6].freq += 1;
                } else if count <= 10 {
                    self.bl_tree[REPZ_3_10].freq += 1;
                } else {
                    self.bl_tree[REPZ_11_138].freq += 1;
                }

                count = 0;
                prevlen = curlen;

                if nextlen == 0 {
                    max_count = 138;
                    min_count = 3;
                } else if curlen == nextlen {
                    max_count = 6;
                    min_count = 3;
                } else {
                    max_count = 7;
                    min_count = 4;
                }
            }
        }
    }
    
}