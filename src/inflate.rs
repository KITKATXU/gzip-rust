use std::io;
use std::ptr::null_mut;
use crate::GzipState;
use crate::trees::Trees;
use crate::{OK, ERROR, STORED, WSIZE, INBUFSIZ};
use std::io::{stdout, Read, Write, Cursor};
use std::cmp::min;
use std::cmp::max;

#[derive(Debug)]
#[derive(Clone)]
struct Huft {
    v: HuftValue, // Pointer to next level of table or value
    e: u8, // Extra bits for the current table
    b: u8, // Number of bits for this code or subcode
}

#[derive(Debug)]
#[derive(Clone)]
enum HuftValue {
    N(u16),          // Literal, length base, or distance base
    T(Box<[Huft]>),  // Pointer to a fixed-size array (dynamic allocation)
}

impl Default for HuftValue {
    fn default() -> Self {
        HuftValue::N(0)
    }
}

impl Default for Huft {
    fn default() -> Self {
        Huft {
            v: HuftValue::default(),
            e: 0,
            b: 0,
        }
    }
}

fn huft_free(t: Option<&Huft>) -> usize {
    if let Some(huft) = t {
        match &huft.v {
            HuftValue::T(sub_table) => {
                // 遍历子表并递归计算释放的节点数量
                sub_table.iter().map(|sub_huft| huft_free(Some(sub_huft))).sum::<usize>() + 1
            }
            HuftValue::N(_) => 1, // 叶子节点直接返回 1
        }
    } else {
        0 // None 时返回 0
    }
}

fn print_huft(h: &Huft, level: usize) {
    let indentation = "  ".repeat(level); // Indentation based on level

    match &h.v {
        HuftValue::N(value) => {
            // Leaf node
            if h.e == 99 {
                println!("Invalid code (e=99)");
            }else if h.b > 0{
                println!(
                    "{}Leaf node, e={}, b={}, value={}",
                    indentation, h.e, h.b, value
                );
            }
        }
        HuftValue::T(table) => {
            // Internal node
            println!(
                "{}Internal node, e={}, b={}",
                indentation, h.e, h.b
            );
            // Recursively print all the child nodes in the table
            for subnode in table.iter() {
                print_huft(subnode, level + 1);
            }
        }
    }
}


fn find_huft_entry(current: &Huft, index: usize) -> Option<&Huft> {
    match &current.v {
        HuftValue::T(sub_table) => sub_table.get(index),
        HuftValue::N(_) => None,
    }
}


// Order of the bit length code lengths
static border: [u16; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

// Copy lengths for literal codes 257..285
static cplens: [u16; 31] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31,
    35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258, 0,
    0,
];

// Extra bits for literal codes 257..285
static cplext: [u16; 31] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2,
    3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0, 99, 99,
]; // 99==invalid

// Copy offsets for distance codes 0..29
static cpdist: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193,
    257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145,
    8193, 12289, 16385, 24577,
];

// Extra bits for distance codes
static cpdext: [u16; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6,
    7, 7, 8, 8, 9, 9, 10, 10, 11, 11,
    12, 12, 13, 13,
];

// Mask bits array equivalent in Rust
static mask_bits: [u32; 17] = [
    0x0000,
    0x0001, 0x0003, 0x0007, 0x000f, 0x001f, 0x003f, 0x007f, 0x00ff,
    0x01ff, 0x03ff, 0x07ff, 0x0fff, 0x1fff, 0x3fff, 0x7fff, 0xffff,
];



// Constants
const BMAX: i32 = 16;      // maximum bit length of any code (16 for explode)
const N_MAX: i32 = 288;    // maximum number of codes in any set

// Function prototypes
// static mut HUFT_FREE: fn(*mut Huft) -> i32 = huft_free;

pub struct Inflate {
    bb: u32,
    bk: u32,
    // wp: usize,
    lbits: i32,
    dbits: i32,
    hufts: u32,
    // slide: [u8; 2 * WSIZE],
}

impl Inflate {
    pub fn new() -> Self {
        Self {
            bb: 0,
            bk: 0,
            // wp: 0,
            lbits: 9,
            dbits: 6,
            hufts: 0,
            // slide: [0; 2 * WSIZE],
        }
    }

    pub fn fill_inbuf<R: Read>(&mut self, input: &mut R, eof_ok: bool, state: &mut GzipState) -> io::Result<u8> {
        state.insize = 0;
        loop {
            let len = self.read_buffer(input, state)?;
            if len == 0 {
                break;
            }
            state.insize += len;
            if state.insize >= INBUFSIZ {
                break;
            }
        }

        if state.insize == 0 {
            if eof_ok {
                return Ok(0xFF);
            }
            self.flush_window(state)?;
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOF"));
        }
        state.bytes_in += state.insize as i64;
        state.inptr = 1;
        Ok(state.inbuf[0])
    }

    pub fn read_buffer<R: Read>(&mut self, input: &mut R, state: &mut GzipState) -> io::Result<usize> {
        let buffer = &mut state.inbuf[state.insize..INBUFSIZ];
        let len = input.read(buffer)?;

        // Output the data read and its length for debugging
        if len > 0 {
            print!("Read {} bytes: ", len);
            for byte in &buffer[..len] {
                print!("{:02x} ", byte);
            }
            println!();
        } else if len == 0 {
            println!("Reached end of file");
        }

        Ok(len)
    }

    pub fn flush_window(&mut self, state: &mut GzipState) -> std::io::Result<()> {
        // println!("flush: outcnt={:?}",state.outcnt);
        if state.outcnt == 0 {
            return Ok(());
        }
        // println!("flush: outcnt={:?}",state.outcnt);

        state.updcrc(Some(&state.window.clone()), state.outcnt);

        if !state.test {
            state.ofd.as_mut().expect("REASON").write_all(&state.window[0..state.outcnt])?;
//             state.write_buf(&mut state.ofd, &state.window[0..state.outcnt], state.outcnt);
        }

        state.bytes_out += state.outcnt as i64;
        state.outcnt = 0;
        Ok(())
    }

    // Function to flush output (equivalent to macro flush_output in C)
    pub fn flush_output(&mut self, state: &mut GzipState, w: usize) {
        unsafe {
            state.outcnt = w;
        }
        self.flush_window(state);
    }

    pub fn get_byte(&mut self, state: &mut GzipState) -> io::Result<u8> {
        if state.inptr < state.insize {
            let byte = state.inbuf[state.inptr];  // Get the byte at the current pointer
            state.inptr += 1;                // Increment the pointer
            Ok(byte)
        } else {
            let mut input = Cursor::new(vec![0; 1]);
            Ok(self.fill_inbuf(&mut input, true, state)?)
        }
    }

    // `try_byte()` function
    pub fn try_byte(&mut self, state: &mut GzipState) -> io::Result<u8> {
        if state.inptr < state.insize {
            let byte = state.inbuf[state.inptr];  // Get the byte at the current pointer
            state.inptr += 1;                // Increment the pointer
            Ok(byte)
        } else {
            let mut input = Cursor::new(vec![1; 1]);
            Ok(self.fill_inbuf(&mut input, true, state)?)
        }
    }

    // Function to get a byte (equivalent to GETBYTE macro)
    pub fn Get_Byte(&mut self, state: &mut GzipState, w: usize) -> io::Result<u8> {
        if state.inptr < state.insize {
            let byte = state.inbuf[state.inptr];
            state.inptr += 1;
            // println!("gb:{:?} {:?}", state.inptr,state.inbuf[state.inptr]);
            Ok(byte)
        } else {
            state.outcnt = w; // This part needs clarification based on your code
//             let mut input = Cursor::new(vec![0; 1]);
//             self.fill_inbuf(&mut input, true, state)?;
                match state.fill_inbuf(false)? {
                    Some(byte) => Ok(byte as u8),
                    None => Ok(0), // EOF represented as -1
                }
                // Placeholder, adjust logic as per the context
        }
    }

    // Function to get the next byte
    pub fn next_byte(&mut self, state: &mut GzipState, w: usize) -> io::Result<u8> {
        self.Get_Byte(state, w)
    }

    // Equivalent to the NEEDBITS macro (requiring more information to be fully accurate)
    pub fn need_bits(&mut self, state: &mut GzipState,k: &mut u32, b: &mut u32, n: u32, w: usize)  {
        while *k < n {
                let byte = match self.next_byte(state, w) {
                    Ok(value) => value, Err(_) => todo!(),
                };
                *b |= (u32::from(byte)) << *k;

                *k += 8;
            }
    }

    // Equivalent to DUMPBITS macro
    pub fn dump_bits(&mut self, k: &mut u32, b: &mut u32, n: u32)  {
        *b = *b >> n;
        *k = *k - n;
    }

    

    pub fn huft_build(
        &mut self,
        b: &[u32],   // Code lengths in bits
        mut n: usize,    // Number of codes
        s: usize,    // Number of simple-valued codes (0..s-1)
        d: &[u16],   // List of base values for non-simple codes
        e: &[u16],   // List of extra bits for non-simple codes
        t: &mut Option<Box<Huft>>, // Result: starting table
        m: &mut i32, // Maximum lookup bits, returns actual
    ) -> u32 {
        let mut c = [0u32; BMAX as usize + 1];
        let mut x = [0u32; BMAX as usize + 1];
        let mut v = [0u32; N_MAX as usize];
        let mut u: Vec<Box<[Huft]>> = Vec::new();
        let mut q: Box<[Huft]> = Box::new([Huft { v: HuftValue::N(0), e: 0, b: 0 }]);
        let mut hpos = [0usize; BMAX as usize + 1];

        let mut a;
        let mut f: u32;
        let mut k: i32;
        let mut g: u32;
        let mut h: i32 = -1;
        let mut l = *m;
        let mut w = -l;
        let mut which_q = 0;
        let mut which_len = 0;
        let mut sub = 0;
        let mut r = Huft::default();
        let mut p = 0;

        

        // Debugging: Initial inputs
        // println!("huft_build called with:");
        // println!("b = {:?}, n = {}, s = {}, d = {:?}, e = {:?}", b, n, s, d, e);

        // Generate counts for each bit length
        for &bit in b.iter().take(n) {
            c[bit as usize] += 1;
        }

        // Debugging: After counting the bits
        // println!("Bit counts (c): {:?}", c);

        if c[0] == n as u32 {
            *t = Some(Box::new(Huft {
                v: HuftValue::T(Box::new([])),
                e: 99,
                b: 1,
            }));
            *m = 1;
            return 0;
        }

        // Find minimum and maximum length
        // let mut k = c.iter().position(|&x| x != 0).unwrap_or(0) as i32;
        
        // Find minimum code length (k), starting from index 1
        k = c.iter()
        .enumerate()
        .find(|&(idx, &x)| idx >= 1 && x != 0)
        .map(|(idx, _)| idx as i32)
        .unwrap_or(0);

        g = (1..=BMAX).rev().find(|&x| c[x as usize] != 0).unwrap_or(0) as u32 ;

    
        // Find maximum code length (g), starting from BMAX and going backwards
        // g = (1..=BMAX)
        // .rev()
        // .find(|&x| c[x as usize] != 0)
        // .map(|x| x as u32)
        // .unwrap_or(0);
        l = l.clamp(k, g as i32);
        *m = l;
        w = -l;

        // Debugging: Min and max lengths
        // println!("Minimum length k = {}, Maximum length g = {}, Clamped l = {}", k, g, l);

        // Adjust last length count
        let mut y: u32 = 1 << k;
        for j in k..g as i32 {
            match y.checked_sub(c[j as usize]) {
                Some(new_y) => {
                    y = new_y;
                }
                None => {
                    return 2;  // 如果发生溢出，返回 2
                }
            }
            if y < 0 {
                return 2;
            }
            y <<= 1;
        }

        y -= c[g as usize];
        if y < 0 {
            return 2;
        }
        c[g as usize] += y;

        // Generate starting offsets
        let mut j = 0;
        for i in 1..=BMAX {
            x[i as usize] = j;
            j += c[i as usize] as u32;
        }

        // Debugging: Offsets after initial calculation
        // println!("Offsets (x): {:?}", x);

        // Populate values array
        for (i, &bit) in b.iter().enumerate().take(n) {
            if bit != 0 {
                v[x[bit as usize] as usize] = i as u32;
                x[bit as usize] += 1;
            }
        }

        // Debugging: Values after population
        // println!("Values (v): {:?}", v);

        n = x[g as usize] as usize;

        // Generate Huffman codes and build tables
        x[0] = 0;
        let mut i = 0;
        let mut z = 0;

        for k in k..=g as i32 {
            a = c[k as usize];


            while a > 0 {
                a -= 1;
//                 println!("--- k: {}, a: {}, w: {}， l: {}", k, a, w, l);  // Debug: output current k and a
                
                while k > w + l {
                    h += 1;
                    
                    w += l;  // Previous table always l bits

                    // Compute minimum size table less than or equal to l bits
                    z = if (g as i32 - w ) as u32 > l as u32 { l as u32 } else { (g as i32 - w ) as u32 };
                    j = (k - w ) as u32;
                    let mut f = 1 << j; // Try a k-w bit table
                    // println!("    f: {}, z: {}", f, z);  // Debug: output f and z

                    if f > a + 1 {
                        f -= a + 1;  // Deduct codes from patterns left
                        let mut xp_index = k as usize;  // Start at index k
                        // let mut j = 0;

                        if j < z {

                            loop {
                                j += 1; // 相当于 C 中的 ++j
                                if j >= z as u32 {
                                    break;
                                }
                                f <<= 1;
                                xp_index += 1;
                                if f <= c[xp_index] {
                                    break;  // Enough codes to use up j bits
                                }
                                f -= c[xp_index];
                                // j += 1;
                                // xp_index += 1;  // Increment the index to move to the next element
                            }
                        }
                    }
                    z = 1 << j;  // Table entries for j-bit table
                    // println!("    New z: {}", z);  // Debug: output new z

                    // Allocate and link in new table
                    q = vec![Huft::default(); (z + 1) as usize].into_boxed_slice();
                    self.hufts += z + 1;  // Track memory usage

                    // Link to list for huft_free
                    *t = Some(Box::new(Huft {
                        v: HuftValue::T(q.clone()),  // Link to new table
                        e: 0,                         // Extra bits
                        b: 0,                         // Number of bits
                    }));
                    let mut  u_len = u.len() as i32;  // 计算长度并存储
                    while u.len() <= (u_len + h) as usize {
                        u.push(Box::new([]));  // 或者根据实际需要添加默认值
                        // println!("stuck");
                    }

                    which_q = 1;
                    which_len = h as usize;
                    u[h as usize] = q.clone();
                    // println!("    Table at h: {}, size: {}", h, q.len());  // Debug: output table size

                    u_len = u.len() as i32;
                    // Connect to last table, if there is one
                    if h > 0 {

                        x[h as usize] = i;  // Save pattern for backing up
                        // r = Huft {
                        //     b: l as u8,         // Bits to dump before this table
                        //     e: (16 + j) as u8,  // Bits in this table
                        //     v: HuftValue::T(q.clone()), // Pointer to this table
                        // };
                        r.b = l as u8;
                        r.e = (16 + j) as u8;
                        r.v = HuftValue::T(q.clone());
                        j = (i >> (w - l)) ;
                        // u[h as usize - 1][j as usize] = r.clone();  // Connect to last table

                        which_q = 2;
                        which_len = h as usize - 1;
                        hpos[which_len] = j as usize;


                        // 指针 tmp 指向 u[0]
                        let mut tmp = &mut u[0][..]; // mutable slice of [Huft]

                        // 遍历每个深度
                        for depth in 0..=which_len {

                            let pos = hpos[depth];

                            if depth == which_len {
                                // if let HuftValue::T(ref mut cloned_q) = tmp {
                                    // cloned_q 现在是 Box<[Huft]> 类型，可以修改其中的元素
                                    tmp[pos] = r.clone(); 
                                    // println!("q = 2: Subtable[{}] = {:?}", j, r);  // Debug: output subtable index and assignment
                                    break;
                                    // }
                            }
                            
                            
                            if let HuftValue::T(ref mut cloned_q) = tmp[pos].v {
                                tmp = cloned_q.as_mut();
                            }
                            // println!("tmp={:?}",tmp);
                            // println!("pos={:?} depth={:?}",pos,depth);
                          

                        }
                        
                        // println!("hpos={:?} which_len={:?} j={:?}",hpos,which_len,j);
                        sub = j as usize;
                        // println!("    Connected to previous table, u[{}][{}] updated, r = {:?}", h - 1, j, r);  // Debug
                    }
                }

                // let mut r = Huft::default();
                r.b = (k - w) as u8;
                // println!("v={:?}",v);

                if p < n {
                    if v[p] < s as u32 {
                        r.e = if v[p] < 256 { 16 } else { 15 };
                        r.v = HuftValue::N(v[p] as u16);
                        p = p + 1;
                    } else {
                        r.e = e[v[p] as usize - s] as u8;
                        r.v = HuftValue::N(d[v[p] as usize - s]);
                        p = p + 1;
                        
                        // println!("p={:?}, vp={:?}, s={:?} idx={:?}",p,v[p], s, v[p-1] as usize - s);
                        // println!("    r: {:?}, e: {}, b: {}, v: {:?}", r, r.e, r.b, r.v);  // Debug: output r
                    }
                } else {
                    r.e = 99;
                }
//                 println!("    r: {:?}, e: {}, v: {:?}", r, r.e, r.v);  // Debug: output r

                // let mut f = 1 << (k - w);
                // let mut i = i >> w;
//                 println!("    f: {}, i (after shift): {}", f, i);  // Debug: output f and shifted i
                
                j = i >> w;  // 初始化 j
                f = 1 << (k - w);

                while j < z {
                    if which_q == 1{
                        u[which_len][j as usize] = r.clone();
                        // println!("q = 1: Subtable[{}] = {:?}", j, r);  // Debug: output subtable index and assignment
                    }
                    if which_q == 2{

                        // 指针 tmp 指向 u[0]
                        let mut tmp = &mut u[0][..]; // mutable slice of [Huft]

                        // 遍历每个深度
                        for depth in 0..=which_len {
                            
                            let pos = hpos[depth];
                            if let HuftValue::T(ref mut cloned_q) = tmp[pos].v {
                                tmp = cloned_q.as_mut();
                            }
                            // println!("tmp={:?}",tmp);
                            // println!("pos={:?} depth={:?}",pos,depth);
                            
                            if depth == which_len {
                                // if let HuftValue::T(ref mut cloned_q) = tmp {
                                    // cloned_q 现在是 Box<[Huft]> 类型，可以修改其中的元素
                                    tmp[j as usize] = r.clone(); 
                                    // println!("q = 2: Subtable[{}] = {:?}", j, r);  // Debug: output subtable index and assignment
                                // }
                            }
                            // println!("aft_tmp={:?}",tmp);
                        }

                        // if let HuftValue::T(ref mut cloned_q) = u[depth][hpos[depth]].v {
                        //     // cloned_q 现在是 Box<[Huft]> 类型，可以修改其中的元素
                        //     cloned_q[j as usize] = r.clone(); 
                        //     // println!("q = 2: Subtable[{}] = {:?}", j, r);  // Debug: output subtable index and assignment
                        // }
                        // }
                    }
                    //     if let HuftValue::T(ref mut cloned_q) = u[which_len][sub].v {
                    //         // cloned_q 现在是 Box<[Huft]> 类型，可以修改其中的元素
                    //         cloned_q[j as usize] = r.clone(); 
                    //         // println!("q = 2: Subtable[{}] = {:?}", j, r);  // Debug: output subtable index and assignment
                    //     }
                    // }
                    // q[j] = r.clone();
                    // println!("        Subtable[{}] = {:?}", j,r);  // Debug: output subtable index and assignment
                    j += f;    // 更新 j
                }

                let mut j = 1 << (k - 1);
                while i & j != 0 {
                    i ^= j;
                    j >>= 1;
                }

                i ^= j;
//                 println!("    i (after code increment): {}", i);  // Debug: output updated i

                // Back up to the previous table if necessary
                let mut x_index = if h >= 0 {
                    h
                } else {
                    x.len() as i32 + h
                };
                while (i & ((1 << w) - 1)) != x[x_index as usize] {
                    h -= 1;
                    w -= l;
                    x_index = if h >= 0 {
                        h
                    } else {
                        x.len() as i32 + h
                    };
                    // println!("    Backing up: h = {}, w = {}", h, w);  // Debug: output h and w during backup
                }

                
            }
        }

        // Debugging: Final subtable count
        // println!("Final number of subtables: {}", u.len());
        // println!("u:{:?}", u);

        if let Some(subtable) = u.get(0) {  // 直接获取 u 的第一个元素
            *t = Some(Box::new(Huft {
                v: HuftValue::T(subtable.clone()),  // 复制 subtable 以避免借用问题
                e: 0,
                b: 0,
            }));
        }

        // Debugging: Final result table
        // println!("Final table: {:?}", t);

        (y != 0 && g != 1) as u32
    }



    // Function to inflate coded data
    pub fn inflate_codes(
        &mut self,
        state: &mut GzipState,
        tl: &Option<Box<Huft>>, // Literal/length table
        td: &Option<Box<Huft>>, // Distance table
        bl: &mut i32,                // Number of bits for literal/length table
        bd: &mut i32,                // Number of bits for distance table
    ) -> i32 {
        let mut b = self.bb; // Bit buffer
        let mut k = self.bk; // Number of bits in bit buffer
        let mut w = state.outcnt; // Current window position
        let mut e = 0;
        let mut d;

        let ml = mask_bits[*bl as usize]; // Mask for `bl` bits
        let md = mask_bits[*bd as usize]; // Mask for `bd` bits

        loop {
            // Get a literal/length code
            self.need_bits(state, &mut k, &mut b, *bl as u32, w);
            // let index = (b & ml) as usize;
            // println!("b={:?},ml={:?},b&ml={:?}",b, ml, index);
            // // println!("tl={:?}",tl);

            // // Traverse the literal/length table      
            // 计算初始的t
            let mut t = match tl {
                Some(t) => &**t,
                None => return 2,
            };
            if let HuftValue::T(ref table) = t.v {
                // Add bounds check
                if table.is_empty() || (b & ml) as usize >= table.len() {
                    return 2; // Invalid table structure
                }
                t = &table[(b & ml) as usize];
            } else {
                return 2; // Invalid structure
            }
            // t = &t[(b & ml) as usize];
            e = t.e;

            if e > 16 {
                loop {


                    // 检查e是否等于99
                    if e == 99 {
                        return 1;
                    }

                    // 调用DUMPBITS函数
                    self.dump_bits(&mut k, &mut b, t.b as u32);

                    // 减去16
                    e -= 16;

                    // 调用NEEDBITS函数
                    self.need_bits(state, &mut k, &mut b, e as u32, w);

                    // 更新t和e

                    t = match &t.v {
                        HuftValue::T(base) => &base[(b & mask_bits[e as usize]) as usize],
                        _ => panic!("预期 HuftValue::T 变体"),
                    };
                    
                    e = t.e;

                    if e <= 16 {
                        // println!("dbg:e={:?}",e);
                        break;
                    }
                }
            }

            self.dump_bits(&mut k, &mut b, t.b as u32);

            if e == 16 {
                // Literal
                let n = match t.v {
                    HuftValue::N(n) => n as usize,
                    _ => panic!("Expected HuftValue::N, but found HuftValue::T"),
                };
                state.window[w] = n as u8;
                // println!("slide[w]={:?}",state.window[w]);
                w += 1;
                if w == WSIZE {
                    // println!("slide={:?}",self.slide);
                    self.flush_output(state, w);
                    w = 0;
                }
            } else {
                // End of block or length
                if e == 15 {
                    // println!("found!");
                    break; // End of block
                }
                // println!("in e={:?}",e);

                // Get length of block to copy
                self.need_bits(state, &mut k, &mut b, e as u32, w);
                let mut n = match t.v {
                    HuftValue::N(n) => n as usize + (b & mask_bits[e as usize]) as usize,
                    _ => panic!("Expected HuftValue::N, but found HuftValue::T"),
                };
                // println!("n={:?}",n);
                self.dump_bits(&mut k, &mut b, e as u32);

                // Get distance of block to copy
                self.need_bits(state, &mut k, &mut b, *bd as u32, w);

                // let mut e;
                // 计算初始的t
                let mut t = match td {
                    Some(t) => &**t,
                    None => return 2,
                };
                if let HuftValue::T(ref table) = t.v {
                    // Add bounds check
                    if table.is_empty() || (b & md) as usize >= table.len() {
                        return 2; // Invalid table structure
                    }
                    t = &table[(b & md) as usize];
                } else {
                    return 2; // Invalid structure
                }
                // t = &t[(b & ml) as usize];
                e = t.e;
                // println!("in1 e={:?}",e);
                if e>16{
                    loop {
                        // 打印调试信息
                        // match &t.v {
                        //     HuftValue::N(n) => println!(
                        //         "tl + ((unsigned)b & ml))->e={} tl + ((unsigned)b & ml))->b={} tl + ((unsigned)b & ml))->n={}",
                        //         t.e, t.b, n
                        //     ),
                        //     HuftValue::T(_) => println!(
                        //         "tl + ((unsigned)b & ml))->e={} tl + ((unsigned)b & ml))->b={} tl + ((unsigned)b & ml))->n=0",
                        //         t.e, t.b
                        //     ),
                        // }

                        // 检查e是否等于99
                        if e == 99 {
                            return 1;
                        }

                        // 调用DUMPBITS函数
                        self.dump_bits(&mut k, &mut b, t.b as u32);

                        // 减去16
                        e -= 16;

                        // 调用NEEDBITS函数
                        self.need_bits(state, &mut k, &mut b, e as u32, w);

                        // 更新t和e
                        t = match &t.v {
                            HuftValue::T(base) => &base[(b & mask_bits[e as usize]) as usize],
                            _ => panic!("预期 HuftValue::T 变体"),
                        };
                        
                        e = t.e;

                        // 检查是否需要继续循环
                        if e <= 16 {
                            break;
                        }
                    }
                }

                self.dump_bits(&mut k, &mut b, t.b as u32);

                self.need_bits(state, &mut k, &mut b, e as u32, w);
                d = match t.v {
                    HuftValue::N(n) => w as isize  - n as isize  - (b & mask_bits[e as usize]) as isize ,
                    _ => panic!("Expected HuftValue::N, but found HuftValue::T"),
                };
                // println!("w={:?} b={:?} msk={:?} e={:?} d={:?} n={:?}",w, b, mask_bits[e as usize], e, d, n);
                
                self.dump_bits(&mut k, &mut b, e as u32);

                // println!("w={:?} b={:?} msk={:?} e={:?} d={:?} n={:?}",w, b, mask_bits[e as usize], e, d, n);
                // Copy block
                while n > 0 {
                    // let e = (if d >= 0 {
                    //     WSIZE - d as usize
                    // } else {
                    //     w - d as usize
                    // })
                    // .min(n);

                    // Step 1: Perform bitwise AND assignment on `d`
                    d = d & (WSIZE - 1) as isize;

                    // Step 2: Assign to `e` the result of `WSIZE - min(d, w)`
                    let mut e = (WSIZE - (max(d, w as isize)) as usize ) ;

                    // Step 3: Assign to `e` the minimum of `e` and `n`
                    e = min(e, n);

                    // Step 4: Subtract `e` from `n`
                    n -= e as usize;
                    // println!("n={:?} e={:?}",n ,e);
                    if e <= if d < w as isize { (w as isize - d) as usize } else { (d - w as isize) as usize} {
                        // Print debug information
                        // println!("dbg: n={:?} w={} d={} e={}", n, w, d, e);

                    // if d >= 0 && d + e as isize <= w as isize {
                        state.window.copy_within((d as usize)..(d as usize) + e, w);
                        w += e;
                        d += e as isize;
                    } else {
                    for _ in 0..e {
                            // println!("w={:?} d={:?} e={:?}",w,d, e);
                            state.window[w] = state.window[d as usize];
                            w += 1;
                            d += 1;
                    }
                    }
                    // n -= e;

                    if w == WSIZE {
                        // println!("flushed!");
                        self.flush_output(state, w);
                        w = 0;
                    }
                }
            }
        }

        // Restore globals
        state.outcnt = w;
        self.bb = b;
        self.bk = k;

        0 // Success
    }

    // Function to decompress an inflated type 0 (stored) block.
    pub fn inflate_stored(&mut self, state: &mut GzipState) -> i32 {
        let mut n: u32;          // number of bytes in block
        let mut w: usize;        // current window position
        let mut b: u32;          // bit buffer
        let mut k: u32;          // number of bits in bit buffer

        // make local copies of globals
        b = self.bb;  // initialize bit buffer
        k = self.bk;  // number of bits in bit buffer
        w = state.outcnt;  // initialize window position

        // go to byte boundary
        n = k & 7;
        self.dump_bits(&mut k, &mut b, n);

        // get the length and its complement
        self.need_bits(state, &mut k, &mut b, 16, w);
        n = (b & 0xffff) as u32;
        self.dump_bits(&mut k, &mut b, 16);
        self.need_bits(state, &mut k, &mut b, 16,w);

        if n != (!b & 0xffff) as u32 {
            return 1;  // error in compressed data
        }
        self.dump_bits(&mut k, &mut b, 16);

        // read and output the compressed data
        while n > 0 {
            self.need_bits(state, &mut k, &mut b, 8, w);
            state.window[w] = (b & 0xff) as u8;  // assuming slide is an array
            w += 1;

            if w == WSIZE {
                self.flush_output(state, w);
                w = 0;
            }
            self.dump_bits(&mut k, &mut b, 8);
            n -= 1;
        }

        // restore the globals from the locals
        state.outcnt = w;  // restore global window pointer
        self.bb = b;  // restore global bit buffer
        self.bk = k;

        return 0;
    }

    // Decompress an inflated type 1 (fixed Huffman codes) block
    pub fn inflate_fixed(&mut self, state: &mut GzipState) -> i32 {
        let mut tl: Option<Box<Huft>> = None; // Literal/length table
        let mut td: Option<Box<Huft>> = None; // Distance table
        let mut bl: i32 = 7;                 // Lookup bits for `tl`
        let mut bd: i32 = 5;                 // Lookup bits for `td`
        let mut l = [0u32; 288];             // Length list for `huft_build`

        // Set up literal table
        for i in 0..144 {
            l[i] = 8;
        }
        for i in 144..256 {
            l[i] = 9;
        }
        for i in 256..280 {
            l[i] = 7;
        }
        for i in 280..288 {
            l[i] = 8;
        }

        // Call huft_build for literal/length table
        let result = self.huft_build(&l, 288, 257, &cplens, &cplext, &mut tl, &mut bl);
        if result != 0 {
            return result as i32;
        }

        // Set up distance table
        let mut l = [0u32; 30]; // Length list for distance table
        for i in 0..30 {
            l[i] = 5;
        }

        // Call huft_build for distance table
        let result = self.huft_build(&l, 30, 0, &cpdist, &cpdext, &mut td, &mut bd);
        // println!("fixed!");
        if result > 1 {
            if let Some(ref tl) = tl {
                huft_free(Some(tl));
            }
            return result as i32;
        }

        if state.test_huft{
            // Debugging: Final result table
            println!("huft_build for literal/length table:");
            match tl {
                Some(ref tl_box) => print_huft(tl_box, 0),
                None => println!("tl is None"),
            }
            
            // print_huft(&tl, 0);
            print!("\n");

            println!("huft_build for distance table:");
            match td {
                Some(ref td_box) => print_huft(td_box, 0),
                None => println!("td is None"),
            }
            
            // print_huft(&td, 0);
            return 0
        }

        // Decompress until an end-of-block code
        if self.inflate_codes(state, &mut tl, &mut td, &mut bl, &mut bd) != 0 {
            return 1;
        }

        // Free the decoding tables
        if let Some(ref tl) = tl {
            huft_free(Some(tl));
        }
        if let Some(ref td) = td {
            huft_free(Some(td));
        }

        0
    }



    // Decompress an inflated type 2 (dynamic Huffman codes) block
    pub fn inflate_dynamic(&mut self, state: &mut GzipState) -> i32 {
        let mut tl: Option<Box<Huft>> = None; // Literal/length table
        let mut td: Option<Box<Huft>> = None; // Distance table
        let mut bl: i32 = 7;                 // Lookup bits for `tl`
        let mut bd: i32 = 5;                 // Lookup bits for `td`
        let mut b = self.bb;                 // Bit buffer
        let mut k = self.bk;                 // Number of bits in the bit buffer
        let mut w = state.outcnt as u32;          // Current window position
        // println!("ib={:?}",state.inbuf);

        // Read table lengths
        self.need_bits(state, &mut k, &mut b, 5, w as usize);
        let nl = 257 + (b & 0x1f); // Number of literal/length codes
        // println!("k={:?}, b={:?}",k,b);
        self.dump_bits(&mut k, &mut b, 5);
        // println!("k={:?}, b={:?}",k,b);
        self.need_bits(state, &mut k, &mut b, 5, w as usize);
        // println!("k={:?}, b={:?}",k,b);
        let nd = 1 + (b & 0x1f);   // Number of distance codes
        self.dump_bits(&mut k, &mut b, 5);
        // println!("k={:?}, b={:?}",k,b);
        self.need_bits(state, &mut k, &mut b, 4, w as usize);
        // println!("k={:?}, b={:?}",k,b);
        let nb = 4 + (b & 0xf);    // Number of bit length codes
        self.dump_bits(&mut k, &mut b, 4);
        // println!("k={:?}, b={:?}",k,b);

        if nl > 286 || nd > 30 {
            return 1; // Invalid code lengths
        }

        // Build bit-length table
        let mut bit_lengths = vec![0u32; 19];
        for j in 0..nb {
            self.need_bits(state, &mut k, &mut b, 3, w as usize);
            bit_lengths[border[j as usize] as usize] = b & 7;
            self.dump_bits(&mut k, &mut b, 3);
        }

        // Set remaining lengths to zero
        for j in nb..19 {
            bit_lengths[border[j as usize] as usize] = 0;
        }
        // println!("{:?}",bit_lengths );

        // Build the Huffman table for bit-length codes
        let mut result = self.huft_build(&bit_lengths, 19, 19, &[], &[], &mut tl, &mut bl);
        if result != 0 {
            if result == 1 {
                if let Some(ref tl) = tl {
                    huft_free(Some(tl));
                }
            }
            return result as i32;
        }

        if tl.is_none() {
            return 2; // Error in tree decoding
        }

        // Debugging: Final result table
        // println!("huft_build for literal/length table:");
        // match tl {
        //     Some(ref tl_box) => print_huft(tl_box, 0),
        //     None => println!("tl is None"),
        // }
        
        // print_huft(&tl, 0);
        // print!("\n");

        // Decode literal/length and distance code lengths
        let n = nl + nd;
        let mut literal_lengths = vec![0u32; n as usize];
        let mut i = 0;
        let mut l = 0;
        let mask = mask_bits[bl as usize];

        while i < n {
            self.need_bits(state, &mut k, &mut b, bl as u32, w as usize);
            let index = (b & mask) as usize;

            let entry = match tl.as_ref() {
                Some(table) => {
                    let mut t = &**table;
                    while let HuftValue::T(ref subtable) = t.v {
                        // Add bounds check
                        if subtable.is_empty() || index >= subtable.len() {
                            return 2; // Invalid table structure
                        }
                        t = &subtable[index];
                    }
                    t
                }
                None => return 2,
            };

            self.dump_bits(&mut k, &mut b, entry.b as u32);

            if entry.e == 99 {
                if let Some(ref tl) = tl {
                    huft_free(Some(tl));
                }
                return 2; // Invalid code
            }

            let j = match entry.v {
                HuftValue::N(value) => value as u32,
                _ => return 2, // Unexpected value type
            };

            if j < 16 {
                l = j;
                literal_lengths[i as usize] = l;
                i += 1;
            } else if j == 16 {
                self.need_bits(state, &mut k, &mut b, 2, w as usize);
                let repeat = 3 + (b & 3);
                self.dump_bits(&mut k, &mut b, 2);
                if i + repeat > n {
                    return 1; // Invalid repeat
                }
                for _ in 0..repeat {
                    literal_lengths[i as usize] = l;
                    i += 1;
                }
            } else if j == 17 {
                self.need_bits(state, &mut k, &mut b, 3, w as usize);
                let repeat = 3 + (b & 7);
                self.dump_bits(&mut k, &mut b, 3);
                if i + repeat > n {
                    return 1; // Invalid repeat
                }
                for _ in 0..repeat {
                    literal_lengths[i as usize] = 0;
                    i += 1;
                }
                l = 0;
            } else if j == 18 {
                self.need_bits(state, &mut k, &mut b, 7, w as usize);
                let repeat = 11 + (b & 0x7f);
                self.dump_bits(&mut k, &mut b, 7);
                if i + repeat > n {
                    return 1; // Invalid repeat
                }
                for _ in 0..repeat {
                    literal_lengths[i as usize] = 0;
                    i += 1;
                }
                l = 0;
            }
        }

        // Free the bit-length table
        if let Some(ref tl) = tl {
            huft_free(Some(tl));
        }

        // Restore the global bit buffer
        self.bb = b;
        self.bk = k;

        // Build literal/length and distance Huffman tables
        bl = self.lbits;
        result = self.huft_build(&literal_lengths, nl as usize, 257, &cplens, &cplext, &mut tl, &mut bl);
        if result != 0 {
            if result == 1 {
                if let Some(ref tl) = tl {
                    huft_free(Some(tl));
                }
            }
            return result as i32;
        }

        bd = self.dbits;
        result = self.huft_build(
            &literal_lengths[nl as usize..],
            nd as usize,
            0,
            &cpdist,
            &cpdext,
            &mut td,
            &mut bd,
        );
        if result != 0 {
            if result == 1 {
                if let Some(ref td) = td {
                    huft_free(Some(td));
                }
            }
            if let Some(ref tl) = tl {
                huft_free(Some(tl));
            }
            return result as i32;
        }

        if state.test_huft{
            // Debugging: Final result table
            println!("huft_build for literal/length table:");
            match tl {
                Some(ref tl_box) => print_huft(tl_box, 0),
                None => println!("tl is None"),
            }
            
            // print_huft(&tl, 0);
            print!("\n");

            println!("huft_build for distance table:");
            match td {
                Some(ref td_box) => print_huft(td_box, 0),
                None => println!("td is None"),
            }
            
            // print_huft(&td, 0);
            return 0
        }

        // Decompress until an end-of-block code
        // println!("dynamic!");
        let err = if self.inflate_codes(state, &mut tl, &mut td, &mut bl, &mut bd) > 0 {
            1
        } else {
            0
        };

        // Free decoding tables
        if let Some(ref tl) = tl {
            huft_free(Some(tl));
        }
        if let Some(ref td) = td {
            huft_free(Some(td));
        }
        err
    }




    // Decompress an inflated block
    // E is the last block flag
    pub fn inflate_block(&mut self, e: &mut i32, state: &mut GzipState) -> i32 {
        let mut t: u32;        // Block type
        let mut w: u32;        // Current window position
        let mut b: u32;        // Bit buffer
        let mut k: u32;        // Number of bits in the bit buffer

        // Initialize local variables
        b = self.bb;
        k = self.bk;
        w = state.outcnt as u32;

        // Read the last block bit
        self.need_bits(state, &mut k, &mut b, 1, w.try_into().unwrap());
        *e = (b & 1) as i32;
        self.dump_bits(&mut k, &mut b, 1);

        // Read the block type
        self.need_bits(state, &mut k, &mut b, 2, w.try_into().unwrap());
        t = (b & 3) as u32;
        self.dump_bits(&mut k, &mut b, 2);

        // Restore the global bit buffer
        self.bb = b;
        self.bk = k;

        // Decompress based on the block type
        match t {
            2 => return self.inflate_dynamic(state),
            0 => return self.inflate_stored(state),
            1 => return self.inflate_fixed(state),
            _ => return 2, // Invalid block type
        }
    }


    // Decompress an inflated entry
    pub fn inflate(&mut self, state: &mut GzipState) -> i32 {
        let mut e: i32 = 42; // Last block flag
        let mut r: i32; // Result code
        let mut h: u32; // Maximum number of `huft` structures allocated

        // Initialize the window and bit buffer
        state.outcnt = 0; // Current window position
        self.bk = 0; // Number of bits in the bit buffer
        self.bb = 0; // Bit buffer

        // Decompress until the last block
        h = 0;
        loop {
            self.hufts = 0; // Initialize `hufts`

            r = self.inflate_block(&mut e, state);
            if r != 0 {
                return r; // Return the error code
            }

            if self.hufts > h {
                h = self.hufts; // Update the maximum `hufts`
            }

            if e != 0 {
                break; // Exit the loop if this is the last block
            }
        }

        // Undo excess pre-reading. The next read will be byte-aligned,
        // so discard unused bits from the last meaningful byte.

        while self.bk >= 8 {
            self.bk -= 8;
            state.inptr -= 1; // Assume `inptr` is a global variable pointing to the input buffer
        }

        self.flush_output(state, state.outcnt); // Assume `flush_output` is a function that writes decompressed data to the output

        // Return success status
        // println!("{}", format!("<{}> ", h)); // Assume `trace` is a debugging output function
        0
    }
}
