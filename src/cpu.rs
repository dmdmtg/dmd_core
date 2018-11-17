use bus::{AccessCode, Bus};
use err::{CpuError, CpuException};

///
/// PSW Flags
///
#[allow(dead_code)]
const F_ET: u32 = 0x00000003;
#[allow(dead_code)]
const F_TM: u32 = 0x00000004;
const F_ISC: u32 = 0x00000078;
const F_I: u32 = 0x00000080;
#[allow(dead_code)]
const F_R: u32 = 0x00000100;
const F_PM: u32 = 0x00000600;
const F_CM: u32 = 0x00001800;
#[allow(dead_code)]
const F_IPL: u32 = 0x0001e000;
#[allow(dead_code)]
const F_TE: u32 = 0x00020000;
const F_C: u32 = 0x00040000;
const F_V: u32 = 0x00080000;
const F_Z: u32 = 0x00100000;
const F_N: u32 = 0x00200000;
#[allow(dead_code)]
const F_OE: u32 = 0x00400000;
#[allow(dead_code)]
const F_CD: u32 = 0x00800000;
#[allow(dead_code)]
const F_QIE: u32 = 0x01000000;
#[allow(dead_code)]
const F_CFD: u32 = 0x02000000;

///
/// Register Indexes
///
const R_FP: usize = 9;
const R_AP: usize = 10;
const R_PSW: usize = 11;
const R_SP: usize = 12;
const R_PCBP: usize = 13;
#[allow(dead_code)]
const R_ISP: usize = 14;
const R_PC: usize = 15;

#[allow(dead_code)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum AddrMode {
    None,
    Absolute,
    AbsoluteDeferred,
    ByteDisplacement,
    ByteDisplacementDeferred,
    HalfwordDisplacement,
    HalfwordDisplacementDeferred,
    WordDisplacement,
    WordDisplacementDeferred,
    APShortOffset,
    FPShortOffset,
    ByteImmediate,
    HalfwordImmediate,
    WordImmediate,
    PositiveLiteral,
    NegativeLiteral,
    Register,
    RegisterDeferred,
    Expanded,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum OpType {
    Lit,
    Src,
    Dest,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Data {
    None,
    Byte, // a.k.a. UByte
    Half, // a.k.a. SHalf
    Word, // a.k.a. SWord
    SByte,
    UHalf,
    UWord,
}

#[allow(dead_code)]
#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub enum CpuMode {
    User,
    Supervisor,
    Executive,
    Kernel,
}

#[derive(Eq, PartialEq, Debug)]
pub struct Operand {
    pub size: u8,
    pub mode: AddrMode,
    data_type: Data,
    expanded_type: Option<Data>,
    pub register: Option<usize>,
    pub embedded: u32,
}

impl Operand {
    fn new(
        size: u8,
        mode: AddrMode,
        data_type: Data,
        expanded_type: Option<Data>,
        register: Option<usize>,
        embedded: u32,
    ) -> Operand {
        Operand {
            size,
            mode,
            data_type,
            expanded_type,
            register,
            embedded,
        }
    }

    fn data_type(&self) -> Data {
        match self.expanded_type {
            Some(t) => t,
            None => self.data_type,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Mnemonic {
    opcode: usize,
    dtype: Data,
    name: &'static str,
    ops: Vec<OpType>,
}

#[allow(dead_code)]
#[derive(Debug, Eq, PartialEq)]
pub struct DecodedInstruction<'a> {
    mnemonic: &'a Mnemonic,
    bytes: u8,
    operands: Vec<Operand>,
}

macro_rules! mn {
    ($opcode:expr, $dtype:expr, $name:expr, $ops:expr) => {
        Mnemonic {
            opcode: $opcode,
            dtype: $dtype,
            name: $name,
            ops: $ops,
        }
    };
}

#[allow(dead_code)]
fn sign_extend_halfword(data: u16) -> u32 {
    ((data as i16) as i32) as u32
}

#[allow(dead_code)]
fn zero_extend_halfword(data: u16) -> u32 {
    data as u32
}

#[allow(dead_code)]
fn sign_extend_byte(data: u8) -> u32 {
    ((data as i8) as i32) as u32
}

#[allow(dead_code)]
fn zero_extend_byte(data: u8) -> u32 {
    data as u32
}

const HWORD_OP_COUNT: usize = 11;

#[allow(dead_code)]
lazy_static! {
    static ref HALFWORD_OPCODES: [Mnemonic; HWORD_OP_COUNT] = [
        mn!(0x09, Data::None, "MVERNO", vec!()),
        mn!(0x0d, Data::None, "ENBVJMP", vec!()),
        mn!(0x13, Data::None, "DISVJMP", vec!()),
        mn!(0x19, Data::None, "MOVBLW", vec!()),
        mn!(0x1f, Data::None, "STREND", vec!()),
        mn!(0x2f, Data::None, "INTACK", vec!()),
        mn!(0x3f, Data::None, "STRCPY", vec!()),
        mn!(0x45, Data::None, "RETG", vec!()),
        mn!(0x61, Data::None, "GATE", vec!()),
        mn!(0xac, Data::None, "CALLPS", vec!()),
        mn!(0xc8, Data::None, "RETPS", vec!()),
    ];
}

#[allow(dead_code)]
lazy_static! {
    static ref OPCODES: [Mnemonic; 256] = [
        // 0x00 - 0x07
        mn!(0x00, Data::None, "halt", vec!()),
        mn!(0x01, Data::None, "???", vec!()),
        mn!(0x02, Data::Word, "SPOPRD", vec!(OpType::Lit, OpType::Src)),
        mn!(0x03, Data::Word, "SPOPRD2", vec!(OpType::Lit, OpType::Src, OpType::Dest)),
        mn!(0x04, Data::Word, "MOVAW", vec!(OpType::Src, OpType::Dest)),
        mn!(0x05, Data::None, "???", vec!()),
        mn!(0x06, Data::Word, "SPOPRT", vec!(OpType::Lit, OpType::Src)),
        mn!(0x07, Data::Word, "SPOPT2", vec!(OpType::Lit, OpType::Src, OpType::Dest)),
        // 0x08 - 0x0F
        mn!(0x08, Data::None, "RET", vec!()),
        mn!(0x09, Data::None, "???", vec!()),
        mn!(0x0A, Data::None, "???", vec!()),
        mn!(0x0B, Data::None, "???", vec!()),
        mn!(0x0C, Data::Word, "MOVTRW", vec!(OpType::Src, OpType::Dest)),
        mn!(0x0D, Data::None, "???", vec!()),
        mn!(0x0E, Data::None, "???", vec!()),
        mn!(0x0F, Data::None, "???", vec!()),
        // 0x10 - 0x17
        mn!(0x10, Data::Word, "SAVE", vec!(OpType::Src)), // Register mode only
        mn!(0x11, Data::None, "???", vec!()),
        mn!(0x12, Data::None, "???", vec!()),
        mn!(0x13, Data::Word, "SPOPWD", vec!(OpType::Lit, OpType::Dest)),
        mn!(0x14, Data::Byte, "EXTOP", vec!()),   // Special Case: Reserved Opcode Exception.
        mn!(0x15, Data::None, "???", vec!()),
        mn!(0x16, Data::None, "???", vec!()),
        mn!(0x17, Data::Word, "SPOPWT", vec!(OpType::Lit, OpType::Dest)),
        // 0x18 - 0x1F
        mn!(0x18, Data::None, "RESTORE", vec!(OpType::Src)),
        mn!(0x19, Data::None, "???", vec!()),
        mn!(0x1A, Data::None, "???", vec!()),
        mn!(0x1B, Data::None, "???", vec!()),
        mn!(0x1C, Data::Word, "SWAPWI", vec!(OpType::Dest)),
        mn!(0x1D, Data::None, "???", vec!()),
        mn!(0x1E, Data::Half, "SWAPHI", vec!(OpType::Dest)),
        mn!(0x1F, Data::Byte, "SWAPBI", vec!(OpType::Dest)),
        // 0x20 - 0x27
        mn!(0x20, Data::Word, "POPW", vec!(OpType::Src)),
        mn!(0x21, Data::None, "???", vec!()),
        mn!(0x22, Data::Word, "SPOPRS", vec!(OpType::Lit, OpType::Src)),
        mn!(0x23, Data::Word, "SPOPS2", vec!(OpType::Lit, OpType::Src, OpType::Dest)),
        mn!(0x24, Data::Word, "JMP", vec!(OpType::Dest)),
        mn!(0x25, Data::None, "???", vec!()),
        mn!(0x26, Data::None, "???", vec!()),
        mn!(0x27, Data::None, "CFLUSH", vec!()),
        // 0x28 - 0x2F
        mn!(0x28, Data::Word, "TSTW", vec!(OpType::Src)),
        mn!(0x29, Data::None, "???", vec!()),
        mn!(0x2A, Data::Half, "TSTH", vec!(OpType::Src)),
        mn!(0x2B, Data::Byte, "TSTB", vec!(OpType::Src)),
        mn!(0x2C, Data::Word, "CALL", vec!(OpType::Src, OpType::Dest)),
        mn!(0x2D, Data::None, "???", vec!()),
        mn!(0x2E, Data::None, "BPT", vec!()),
        mn!(0x2F, Data::None, "WAIT", vec!()),
        // 0x30 - 0x37
        mn!(0x30, Data::None, "???", vec!()),
        mn!(0x31, Data::None, "???", vec!()),
        mn!(0x32, Data::Word, "SPOP", vec!(OpType::Lit)),
        mn!(0x33, Data::Word, "SPOPWS", vec!(OpType::Lit, OpType::Dest)),
        mn!(0x34, Data::Word, "JSB", vec!(OpType::Dest)),
        mn!(0x35, Data::None, "???", vec!()),
        mn!(0x36, Data::Half, "BSBH", vec!(OpType::Lit)),
        mn!(0x37, Data::Byte, "BSBB", vec!(OpType::Lit)),
        // 0x38 - 0x3F
        mn!(0x38, Data::Word, "BITW", vec!(OpType::Src, OpType::Src)),
        mn!(0x39, Data::None, "???", vec!()),
        mn!(0x3A, Data::Half, "BITH", vec!(OpType::Src, OpType::Src)),
        mn!(0x3B, Data::Byte, "BITB", vec!(OpType::Src, OpType::Src)),
        mn!(0x3C, Data::Word, "CMPW", vec!(OpType::Src, OpType::Src)),
        mn!(0x3D, Data::None, "???", vec!()),
        mn!(0x3E, Data::Half, "CMPH", vec!(OpType::Src, OpType::Src)),
        mn!(0x3F, Data::Byte, "CMPB", vec!(OpType::Src, OpType::Src)),
        // 0x40 - 0x47
        mn!(0x40, Data::None, "RGEQ", vec!()),
        mn!(0x41, Data::None, "???", vec!()),
        mn!(0x42, Data::Half, "BGEH", vec!(OpType::Lit)),
        mn!(0x43, Data::Byte, "BGEB", vec!(OpType::Lit)),
        mn!(0x44, Data::None, "RGTR", vec!()),
        mn!(0x45, Data::None, "???", vec!()),
        mn!(0x46, Data::Half, "BGH", vec!(OpType::Lit)),
        mn!(0x47, Data::Byte, "BGB", vec!(OpType::Lit)),
        // 0x48 - 0x4F
        mn!(0x48, Data::None, "RLSS", vec!()),
        mn!(0x49, Data::None, "???", vec!()),
        mn!(0x4A, Data::Half, "BLH", vec!(OpType::Lit)),
        mn!(0x4B, Data::Byte, "BLB", vec!(OpType::Lit)),
        mn!(0x4C, Data::None, "RLEQ", vec!()),
        mn!(0x4D, Data::None, "???", vec!()),
        mn!(0x4E, Data::Half, "BLEH", vec!(OpType::Lit)),
        mn!(0x4F, Data::Byte, "BLEB", vec!(OpType::Lit)),
        // 0x50 - 0x57
        mn!(0x50, Data::None, "RGEQU", vec!()),      // a.k.a. RCC
        mn!(0x51, Data::None, "???", vec!()),
        mn!(0x52, Data::Half, "BGEUH", vec!(OpType::Lit)),
        mn!(0x53, Data::Byte, "BGEUB", vec!(OpType::Lit)),
        mn!(0x54, Data::None, "RGTRU", vec!()),
        mn!(0x55, Data::None, "???", vec!()),
        mn!(0x56, Data::Half, "BGUH", vec!(OpType::Lit)),
        mn!(0x57, Data::Byte, "BGUB", vec!(OpType::Lit)),
        // 0x58 - 0x5F
        mn!(0x58, Data::None, "RLSSU", vec!()),      // a.k.a. RCS
        mn!(0x59, Data::None, "???", vec!()),
        mn!(0x5A, Data::Half, "BLUH", vec!(OpType::Lit)),
        mn!(0x5B, Data::Byte, "BLUB", vec!(OpType::Lit)),
        mn!(0x5C, Data::None, "RLEQU", vec!()),
        mn!(0x5D, Data::None, "???", vec!()),
        mn!(0x5E, Data::Half, "BLEUH", vec!(OpType::Lit)),
        mn!(0x5F, Data::Byte, "BLEUB", vec!(OpType::Lit)),
        // 0x60 - 0x67
        mn!(0x60, Data::None, "RVC", vec!()),
        mn!(0x61, Data::None, "???", vec!()),
        mn!(0x62, Data::Half, "BVCH", vec!(OpType::Lit)),
        mn!(0x63, Data::Byte, "BVCB", vec!(OpType::Lit)),
        mn!(0x64, Data::None, "RNEQU", vec!()),
        mn!(0x65, Data::None, "???", vec!()),
        mn!(0x66, Data::Half, "BNEH", vec!(OpType::Lit)),
        mn!(0x67, Data::Byte, "BNEB", vec!(OpType::Lit)),
        // 0x68 - 0x6F
        mn!(0x68, Data::None, "RVS", vec!()),
        mn!(0x69, Data::None, "???", vec!()),
        mn!(0x6A, Data::Half, "BVSH", vec!(OpType::Lit)),
        mn!(0x6B, Data::Byte, "BVSB", vec!(OpType::Lit)),
        mn!(0x6C, Data::None, "REQLU", vec!()),
        mn!(0x6D, Data::None, "???", vec!()),
        mn!(0x6E, Data::Half, "BEH", vec!(OpType::Lit)),
        mn!(0x6F, Data::Byte, "BEB", vec!(OpType::Lit)),
        // 0x70 - 0x77
        mn!(0x70, Data::None, "NOP", vec!()),
        mn!(0x71, Data::None, "???", vec!()),
        mn!(0x72, Data::None, "NOP3", vec!()),
        mn!(0x73, Data::None, "NOP2", vec!()),
        mn!(0x74, Data::None, "RNEQ", vec!()),
        mn!(0x75, Data::None, "???", vec!()),
        mn!(0x76, Data::Half, "BNEH", vec!(OpType::Lit)),
        mn!(0x77, Data::Byte, "BNEB", vec!(OpType::Lit)),
        // 0x78 - 0x7F
        mn!(0x78, Data::None, "RSB", vec!()),
        mn!(0x79, Data::None, "???", vec!()),
        mn!(0x7A, Data::Half, "BRH", vec!(OpType::Lit)),
        mn!(0x7B, Data::Byte, "BRB", vec!(OpType::Lit)),
        mn!(0x7C, Data::None, "REQL", vec!()),
        mn!(0x7D, Data::None, "???", vec!()),
        mn!(0x7E, Data::Half, "BEH", vec!(OpType::Lit)),
        mn!(0x7F, Data::Byte, "BEB", vec!(OpType::Lit)),
        // 0x80 - 0x87
        mn!(0x80, Data::Word, "CLRW", vec!(OpType::Dest)),
        mn!(0x81, Data::None, "???", vec!()),
        mn!(0x82, Data::Half, "CLRH", vec!(OpType::Dest)),
        mn!(0x83, Data::Byte, "CLRB", vec!(OpType::Dest)),
        mn!(0x84, Data::Word, "MOVW", vec!(OpType::Src, OpType::Dest)),
        mn!(0x85, Data::None, "???", vec!()),
        mn!(0x86, Data::Half, "MOVH", vec!(OpType::Src, OpType::Dest)),
        mn!(0x87, Data::Byte, "MOVB", vec!(OpType::Src, OpType::Dest)),
        // 0x88 - 0x8F
        mn!(0x88, Data::Word, "MCOMW", vec!(OpType::Src, OpType::Dest)),
        mn!(0x89, Data::None, "???", vec!()),
        mn!(0x8A, Data::Half, "MCOMH", vec!(OpType::Src, OpType::Dest)),
        mn!(0x8B, Data::Byte, "MCOMB", vec!(OpType::Src, OpType::Dest)),
        mn!(0x8C, Data::Word, "MNEGW", vec!(OpType::Src, OpType::Dest)),
        mn!(0x8D, Data::None, "???", vec!()),
        mn!(0x8E, Data::Half, "MNEGH", vec!(OpType::Src, OpType::Dest)),
        mn!(0x8F, Data::Byte, "MNEGB", vec!(OpType::Src, OpType::Dest)),
        // 0x90 - 0x97
        mn!(0x90, Data::Word, "INCW", vec!(OpType::Dest)),
        mn!(0x91, Data::None, "???", vec!()),
        mn!(0x92, Data::Half, "INCH", vec!(OpType::Dest)),
        mn!(0x93, Data::Byte, "INCB", vec!(OpType::Dest)),
        mn!(0x94, Data::Word, "DECW", vec!(OpType::Dest)),
        mn!(0x95, Data::None, "???", vec!()),
        mn!(0x96, Data::Half, "DECH", vec!(OpType::Dest)),
        mn!(0x97, Data::Byte, "DECB", vec!(OpType::Dest)),
        // 0x98 - 0x9F
        mn!(0x98, Data::None, "???", vec!()),
        mn!(0x99, Data::None, "???", vec!()),
        mn!(0x9A, Data::None, "???", vec!()),
        mn!(0x9B, Data::None, "???", vec!()),
        mn!(0x9C, Data::Word, "ADDW2", vec!(OpType::Src, OpType::Dest)),
        mn!(0x9D, Data::None, "???", vec!()),
        mn!(0x9E, Data::Half, "ADDH2", vec!(OpType::Src, OpType::Dest)),
        mn!(0x9F, Data::Byte, "ADDB2", vec!(OpType::Src, OpType::Dest)),
        // 0xA0 - 0xA7
        mn!(0xA0, Data::Word, "PUSHW", vec!(OpType::Src)),
        mn!(0xA1, Data::None, "???", vec!()),
        mn!(0xA2, Data::None, "???", vec!()),
        mn!(0xA3, Data::None, "???", vec!()),
        mn!(0xA4, Data::Word, "MODW2", vec!(OpType::Src, OpType::Dest)),
        mn!(0xA5, Data::None, "???", vec!()),
        mn!(0xA6, Data::Half, "MODH2", vec!(OpType::Src, OpType::Dest)),
        mn!(0xA7, Data::Byte, "MODB2", vec!(OpType::Src, OpType::Dest)),
        // 0xA8 - 0xAF
        mn!(0xA8, Data::Word, "MULW2", vec!(OpType::Src, OpType::Dest)),
        mn!(0xA9, Data::None, "???", vec!()),
        mn!(0xAA, Data::Half, "MULH2", vec!(OpType::Src, OpType::Dest)),
        mn!(0xAB, Data::Byte, "MULB2", vec!(OpType::Src, OpType::Dest)),
        mn!(0xAC, Data::Word, "DIVW2", vec!(OpType::Src, OpType::Dest)),
        mn!(0xAD, Data::None, "???", vec!()),
        mn!(0xAE, Data::Half, "DIVH2", vec!(OpType::Src, OpType::Dest)),
        mn!(0xAF, Data::Byte, "DIVB2", vec!(OpType::Src, OpType::Dest)),
        // 0xB0 - 0xB7
        mn!(0xB0, Data::Word, "ORW2", vec!(OpType::Src, OpType::Dest)),
        mn!(0xB1, Data::None, "???", vec!()),
        mn!(0xB2, Data::Half, "ORH2", vec!(OpType::Src, OpType::Dest)),
        mn!(0xB3, Data::Byte, "ORB2", vec!(OpType::Src, OpType::Dest)),
        mn!(0xB4, Data::Word, "XORW2", vec!(OpType::Src, OpType::Dest)),
        mn!(0xB5, Data::None, "???", vec!()),
        mn!(0xB6, Data::Half, "XORH2", vec!(OpType::Src, OpType::Dest)),
        mn!(0xB7, Data::Byte, "XORB2", vec!(OpType::Src, OpType::Dest)),
        // 0xB8 - 0xBF
        mn!(0xB8, Data::Word, "ANDW2", vec!(OpType::Src, OpType::Dest)),
        mn!(0xB9, Data::None, "???", vec!()),
        mn!(0xBA, Data::Half, "ANDH2", vec!(OpType::Src, OpType::Dest)),
        mn!(0xBB, Data::Byte, "ANDB2", vec!(OpType::Src, OpType::Dest)),
        mn!(0xBC, Data::Word, "SUBW2", vec!(OpType::Src, OpType::Dest)),
        mn!(0xBD, Data::None, "???", vec!()),
        mn!(0xBE, Data::Half, "SUBH2", vec!(OpType::Src, OpType::Dest)),
        mn!(0xBF, Data::Byte, "SUBB2", vec!(OpType::Src, OpType::Dest)),
        // 0xC0 - 0xC7
        mn!(0xC0, Data::Word, "ALSW3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xC1, Data::None, "???", vec!()),
        mn!(0xC2, Data::None, "???", vec!()),
        mn!(0xC3, Data::None, "???", vec!()),
        mn!(0xC4, Data::Word, "ARSW3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xC5, Data::None, "???", vec!()),
        mn!(0xC6, Data::Half, "ARSH3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xC7, Data::Byte, "ARSB3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        // 0xC8 - 0xCF
        mn!(0xC8, Data::Word, "INSFW", vec!(OpType::Src, OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xC9, Data::None, "???", vec!()),
        mn!(0xCA, Data::Half, "INSFH", vec!(OpType::Src, OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xCB, Data::Byte, "INSFB", vec!(OpType::Src, OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xCC, Data::Word, "EXTFW", vec!(OpType::Src, OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xCD, Data::None, "???", vec!()),
        mn!(0xCE, Data::Half, "EXTFH", vec!(OpType::Src, OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xCF, Data::Byte, "EXTFB", vec!(OpType::Src, OpType::Src, OpType::Src, OpType::Dest)),
        // 0xD0 - 0xD7
        mn!(0xD0, Data::Word, "LLSW3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xD1, Data::None, "???", vec!()),
        mn!(0xD2, Data::Half, "LLSH3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xD3, Data::Byte, "LLSB3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xD4, Data::Word, "LRSW3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xD5, Data::None, "???", vec!()),
        mn!(0xD6, Data::None, "???", vec!()),
        mn!(0xD7, Data::None, "???", vec!()),
        // 0xD8 - 0xDF
        mn!(0xD8, Data::Word, "ROTW", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xD9, Data::None, "???", vec!()),
        mn!(0xDA, Data::None, "???", vec!()),
        mn!(0xDB, Data::None, "???", vec!()),
        mn!(0xDC, Data::Word, "ADDW3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xDD, Data::None, "???", vec!()),
        mn!(0xDE, Data::Half, "ADDH3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xDF, Data::Byte, "ADDB3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        // 0xE0 - 0xE7
        mn!(0xE0, Data::Word, "PUSHAW", vec!(OpType::Src)),
        mn!(0xE1, Data::None, "???", vec!()),
        mn!(0xE2, Data::None, "???", vec!()),
        mn!(0xE3, Data::None, "???", vec!()),
        mn!(0xE4, Data::Word, "MODW3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xE5, Data::None, "???", vec!()),
        mn!(0xE6, Data::Half, "MODH3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xE7, Data::Byte, "MODB3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        // 0xE8 - 0xEF
        mn!(0xE8, Data::Word, "MULW3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xE9, Data::None, "???", vec!()),
        mn!(0xEA, Data::Half, "MULH3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xEB, Data::Byte, "MULB3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xEC, Data::Word, "DIVW3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xED, Data::None, "???", vec!()),
        mn!(0xEE, Data::Half, "DIVH3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xEF, Data::Byte, "DIVB3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        // 0xF0 - 0xF7
        mn!(0xF0, Data::Word, "ORW3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xF1, Data::None, "???", vec!()),
        mn!(0xF2, Data::Half, "ORH3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xF3, Data::Byte, "ORB3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xF4, Data::Word, "XORW3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xF5, Data::None, "???", vec!()),
        mn!(0xF6, Data::Half, "XORH3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xF7, Data::Byte, "XORB3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        // 0xF8 - 0xFF
        mn!(0xF8, Data::Word, "ANDW3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xF9, Data::None, "???", vec!()),
        mn!(0xFA, Data::Half, "ANDH3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xFB, Data::Byte, "ANDB3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xFC, Data::Word, "SUBW3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xFD, Data::None, "???", vec!()),
        mn!(0xFE, Data::Half, "SUBH3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
        mn!(0xFF, Data::Byte, "SUBB3", vec!(OpType::Src, OpType::Src, OpType::Dest)),
    ];
}

///
/// Note that we store registers as an array of type u32 because
/// we often need to reference registers by index (0-15) when decoding
/// and executing instructions.
///
#[allow(dead_code)]
pub struct Cpu<'a> {
    r: [u32; 16],
    ir: Option<DecodedInstruction<'a>>,
}

#[allow(dead_code)]
impl<'a> Cpu<'a> {
    pub fn new() -> Cpu<'a> {
        Cpu {
            r: [0; 16],
            ir: None,
        }
    }

    pub fn reset(&mut self, bus: &mut Bus) -> Result<(), CpuError> {
        //
        // The WE32100 Manual, Page 2-52, describes the reset process
        //
        //  1. Change to physical address mode
        //  2. Fetch the word at physical address 0x80 and store it in
        //     the PCBP register.
        //  3. Fetch the word at the PCB address and store it in the
        //     PSW.
        //  4. Fetch the word at PCB address + 4 bytes and store it
        //     in the PC.
        //  5. Fetch the word at PCB address + 8 bytes and store it
        //     in the SP.
        //  6. Fetch the word at PCB address + 12 bytes and store it
        //     in the PCB, if bit I in PSW is set.
        //

        self.r[R_PCBP] = bus.read_word(0x80, AccessCode::AddressFetch)?;
        self.r[R_PSW] = bus.read_word(self.r[R_PCBP] as usize, AccessCode::AddressFetch)?;
        self.r[R_PC] = bus.read_word(self.r[R_PCBP] as usize + 4, AccessCode::AddressFetch)?;
        self.r[R_SP] = bus.read_word(self.r[R_PCBP] as usize + 8, AccessCode::AddressFetch)?;

        if self.r[R_PSW] & F_I != 0 {
            self.r[R_PSW] &= !F_I;
            self.r[R_PCBP] += 12;
        }

        self.set_isc(3);

        Ok(())
    }

    pub fn effective_address(&self, bus: &mut Bus, op: &Operand) -> Result<u32, CpuError> {
        match op.mode {
            AddrMode::RegisterDeferred => {
                let r = match op.register {
                    Some(v) => v,
                    None => return Err(CpuError::Exception(CpuException::IllegalOpcode)),
                };
                Ok(self.r[r])
            }
            AddrMode::Absolute => Ok(op.embedded),
            AddrMode::AbsoluteDeferred => {
                Ok(bus.read_word(op.embedded as usize, AccessCode::AddressFetch)?)
            }
            AddrMode::FPShortOffset => Ok(self.r[R_FP] + sign_extend_byte(op.embedded as u8)),
            AddrMode::APShortOffset => Ok(self.r[R_AP] + sign_extend_byte(op.embedded as u8)),
            AddrMode::WordDisplacement => {
                let r = match op.register {
                    Some(v) => v,
                    None => return Err(CpuError::Exception(CpuException::IllegalOpcode)),
                };
                Ok(self.r[r] + op.embedded)
            }
            AddrMode::WordDisplacementDeferred => {
                let r = match op.register {
                    Some(v) => v,
                    None => return Err(CpuError::Exception(CpuException::IllegalOpcode)),
                };
                Ok(bus.read_word((self.r[r] + op.embedded) as usize, AccessCode::AddressFetch)?)
            }
            AddrMode::HalfwordDisplacement => {
                let r = match op.register {
                    Some(v) => v,
                    None => return Err(CpuError::Exception(CpuException::IllegalOpcode)),
                };
                Ok(self.r[r] + sign_extend_halfword(op.embedded as u16))
            }
            AddrMode::HalfwordDisplacementDeferred => {
                let r = match op.register {
                    Some(v) => v,
                    None => return Err(CpuError::Exception(CpuException::IllegalOpcode)),
                };
                Ok(bus.read_word(
                    (self.r[r] + sign_extend_halfword(op.embedded as u16)) as usize,
                    AccessCode::AddressFetch,
                )?)
            }
            AddrMode::ByteDisplacement => {
                let r = match op.register {
                    Some(v) => v,
                    None => return Err(CpuError::Exception(CpuException::IllegalOpcode)),
                };
                Ok(self.r[r] + sign_extend_byte(op.embedded as u8))
            }
            AddrMode::ByteDisplacementDeferred => {
                let r = match op.register {
                    Some(v) => v,
                    None => return Err(CpuError::Exception(CpuException::IllegalOpcode)),
                };
                Ok(bus.read_word(
                    (self.r[r] + sign_extend_byte(op.embedded as u8)) as usize,
                    AccessCode::AddressFetch,
                )?)
            }
            _ => Err(CpuError::Exception(CpuException::IllegalOpcode)),
        }
    }

    pub fn read_op(&self, bus: &mut Bus, op: &Operand) -> Result<u32, CpuError> {
        match op.mode {
            AddrMode::Register => {
                let r = match op.register {
                    Some(v) => v,
                    None => return Err(CpuError::Exception(CpuException::IllegalOpcode)),
                };

                match op.data_type() {
                    Data::Word | Data::UWord => Ok(self.r[r]),
                    Data::Half => Ok(sign_extend_halfword(self.r[r] as u16)),
                    Data::UHalf => Ok((self.r[r] as u16) as u32),
                    Data::Byte => Ok((self.r[r] as u8) as u32),
                    Data::SByte => Ok(sign_extend_byte(self.r[r] as u8)),
                    _ => Err(CpuError::Exception(CpuException::IllegalOpcode)),
                }
            }
            AddrMode::PositiveLiteral | AddrMode::NegativeLiteral => {
                Ok(sign_extend_byte(op.embedded as u8))
            }
            AddrMode::WordImmediate => Ok(op.embedded),
            AddrMode::HalfwordImmediate => Ok(sign_extend_halfword(op.embedded as u16)),
            AddrMode::ByteImmediate => Ok(sign_extend_byte(op.embedded as u8)),
            _ => {
                let eff = self.effective_address(bus, op)?;
                match op.data_type() {
                    Data::UWord | Data::Word => {
                        Ok(bus.read_word(eff as usize, AccessCode::InstrFetch)?)
                    }
                    Data::Half => Ok(sign_extend_halfword(
                        bus.read_half(eff as usize, AccessCode::InstrFetch)?,
                    )),
                    Data::UHalf => Ok(bus.read_half(eff as usize, AccessCode::InstrFetch)? as u32),
                    Data::Byte => Ok(bus.read_byte(eff as usize, AccessCode::InstrFetch)? as u32),
                    Data::SByte => Ok(sign_extend_byte(
                        bus.read_byte(eff as usize, AccessCode::InstrFetch)?,
                    )),
                    _ => return Err(CpuError::Exception(CpuException::IllegalOpcode)),
                }
            }
        }
    }

    pub fn write_op(&mut self, bus: &mut Bus, op: &Operand, val: u32) -> Result<(), CpuError> {
        match op.mode {
            AddrMode::Register => match op.register {
                Some(r) => self.r[r] = val,
                None => return Err(CpuError::Exception(CpuException::IllegalOpcode)),
            },
            AddrMode::NegativeLiteral
            | AddrMode::PositiveLiteral
            | AddrMode::ByteImmediate
            | AddrMode::HalfwordImmediate
            | AddrMode::WordImmediate => {
                return Err(CpuError::Exception(CpuException::IllegalOpcode))
            }
            _ => {
                let eff = self.effective_address(bus, op)?;
                match op.data_type() {
                    Data::UWord | Data::Word => bus.write_word(eff as usize, val)?,
                    Data::Half | Data::UHalf => bus.write_half(eff as usize, val as u16)?,
                    Data::Byte | Data::SByte => bus.write_byte(eff as usize, val as u8)?,
                    _ => return Err(CpuError::Exception(CpuException::IllegalOpcode)),
                }
            }
        }
        Ok(())
    }

    pub fn step(&mut self, bus: &mut Bus) -> Result<(), CpuError> {
        let instr = self.decode_instruction(bus)?;

        match instr.mnemonic.opcode {
            0x84|0x86|0x87 => { // MOVW, MOVH, MOVB
                let val = self.read_op(bus, &instr.operands[0])?;
                self.write_op(bus, &instr.operands[1], val)
            }
            _ => return Err(CpuError::Exception(CpuException::IllegalOpcode)),
        }
    }

    pub fn set_pc(&mut self, val: u32) {
        self.r[R_PC] = val;
    }

    fn decode_operand_literal(
        &self,
        bus: &mut Bus,
        mn: &Mnemonic,
        addr: usize,
    ) -> Result<Operand, CpuError> {
        match mn.dtype {
            Data::Byte => {
                let b: u8 = bus.read_byte(addr, AccessCode::OperandFetch)?;
                Ok(Operand::new(
                    1,
                    AddrMode::None,
                    Data::Byte,
                    None,
                    None,
                    b as u32,
                ))
            }
            Data::Half => {
                let h: u16 = bus.read_half_unaligned(addr, AccessCode::OperandFetch)?;
                Ok(Operand::new(
                    2,
                    AddrMode::None,
                    Data::Half,
                    None,
                    None,
                    h as u32,
                ))
            }
            Data::Word => {
                let w: u32 = bus.read_word_unaligned(addr, AccessCode::OperandFetch)?;
                Ok(Operand::new(4, AddrMode::None, Data::Word, None, None, w))
            }
            _ => Err(CpuError::Exception(CpuException::IllegalOpcode)),
        }
    }

    fn decode_operand_descriptor(
        &self,
        bus: &mut Bus,
        dtype: Data,
        etype: Option<Data>,
        addr: usize,
        recur: bool,
    ) -> Result<Operand, CpuError> {
        let descriptor_byte: u8 = bus.read_byte(addr, AccessCode::OperandFetch)?;

        let m = (descriptor_byte & 0xf0) >> 4;
        let r = descriptor_byte & 0xf;

        // The descriptor is either 1 or 2 bytes, depending on whether this is a recursive
        // call or not.
        let dsize = if recur { 2 } else { 1 };

        match m {
            0 | 1 | 2 | 3 => {
                // Positive Literal
                Ok(Operand::new(
                    dsize,
                    AddrMode::PositiveLiteral,
                    dtype,
                    etype,
                    None,
                    descriptor_byte as u32,
                ))
            }
            4 => {
                match r {
                    15 => {
                        // Word Immediate
                        let w = bus.read_word_unaligned(addr + 1, AccessCode::OperandFetch)?;
                        Ok(Operand::new(
                            dsize + 4,
                            AddrMode::WordImmediate,
                            dtype,
                            etype,
                            None,
                            w,
                        ))
                    }
                    _ => {
                        // Register
                        Ok(Operand::new(
                            dsize,
                            AddrMode::Register,
                            dtype,
                            etype,
                            Some(r as usize),
                            0,
                        ))
                    }
                }
            }
            5 => {
                match r {
                    15 => {
                        // Halfword Immediate
                        let h = bus.read_half_unaligned(addr + 1, AccessCode::OperandFetch)?;
                        Ok(Operand::new(
                            dsize + 2,
                            AddrMode::HalfwordImmediate,
                            dtype,
                            etype,
                            None,
                            h as u32,
                        ))
                    }
                    11 => {
                        // Illegal
                        Err(CpuError::Exception(CpuException::IllegalOpcode))
                    }
                    _ => {
                        // Register Deferred Mode
                        Ok(Operand::new(
                            dsize,
                            AddrMode::RegisterDeferred,
                            dtype,
                            etype,
                            Some(r as usize),
                            0,
                        ))
                    }
                }
            }
            6 => {
                match r {
                    15 => {
                        // Byte Immediate
                        let b = bus.read_byte(addr + 1, AccessCode::OperandFetch)?;
                        Ok(Operand::new(
                            dsize + 1,
                            AddrMode::ByteImmediate,
                            dtype,
                            etype,
                            None,
                            b as u32,
                        ))
                    }
                    _ => {
                        // FP Short Offset
                        Ok(Operand::new(
                            dsize,
                            AddrMode::FPShortOffset,
                            dtype,
                            etype,
                            Some(R_FP),
                            r as u32,
                        ))
                    }
                }
            }
            7 => {
                match r {
                    15 => {
                        // Absolute
                        let w = bus.read_word_unaligned(addr + 1, AccessCode::OperandFetch)?;
                        Ok(Operand::new(
                            dsize + 4,
                            AddrMode::Absolute,
                            dtype,
                            etype,
                            None,
                            w,
                        ))
                    }
                    _ => {
                        // AP Short Offset
                        Ok(Operand::new(
                            dsize,
                            AddrMode::APShortOffset,
                            dtype,
                            etype,
                            Some(R_AP),
                            r as u32,
                        ))
                    }
                }
            }
            8 => {
                match r {
                    11 => Err(CpuError::Exception(CpuException::IllegalOpcode)),
                    _ => {
                        // Word Displacement
                        let disp = bus.read_word_unaligned(addr + 1, AccessCode::OperandFetch)?;
                        Ok(Operand::new(
                            dsize + 4,
                            AddrMode::WordDisplacement,
                            dtype,
                            etype,
                            Some(r as usize),
                            disp,
                        ))
                    }
                }
            }
            9 => {
                match r {
                    11 => Err(CpuError::Exception(CpuException::IllegalOpcode)),
                    _ => {
                        // Word Displacement Deferred
                        let disp = bus.read_word_unaligned(addr + 1, AccessCode::OperandFetch)?;
                        Ok(Operand::new(
                            dsize + 4,
                            AddrMode::WordDisplacementDeferred,
                            dtype,
                            etype,
                            Some(r as usize),
                            disp,
                        ))
                    }
                }
            }
            10 => {
                match r {
                    11 => Err(CpuError::Exception(CpuException::IllegalOpcode)),
                    _ => {
                        // Halfword Displacement
                        let disp = bus.read_half_unaligned(addr + 1, AccessCode::OperandFetch)?;
                        Ok(Operand::new(
                            dsize + 2,
                            AddrMode::HalfwordDisplacement,
                            dtype,
                            etype,
                            Some(r as usize),
                            disp as u32,
                        ))
                    }
                }
            }
            11 => {
                match r {
                    11 => Err(CpuError::Exception(CpuException::IllegalOpcode)),
                    _ => {
                        // Halfword Displacement Deferred
                        let disp = bus.read_half_unaligned(addr + 1, AccessCode::OperandFetch)?;
                        Ok(Operand::new(
                            dsize + 2,
                            AddrMode::HalfwordDisplacementDeferred,
                            dtype,
                            etype,
                            Some(r as usize),
                            disp as u32,
                        ))
                    }
                }
            }
            12 => {
                match r {
                    11 => Err(CpuError::Exception(CpuException::IllegalOpcode)),
                    _ => {
                        // Byte Displacement
                        let disp = bus.read_byte(addr + 1, AccessCode::OperandFetch)?;
                        Ok(Operand::new(
                            dsize + 1,
                            AddrMode::ByteDisplacement,
                            dtype,
                            etype,
                            Some(r as usize),
                            disp as u32,
                        ))
                    }
                }
            }
            13 => {
                match r {
                    11 => Err(CpuError::Exception(CpuException::IllegalOpcode)),
                    _ => {
                        // Byte Displacement Deferred
                        let disp = bus.read_byte(addr + 1, AccessCode::OperandFetch)?;
                        Ok(Operand::new(
                            dsize + 1,
                            AddrMode::ByteDisplacementDeferred,
                            dtype,
                            etype,
                            Some(r as usize),
                            disp as u32,
                        ))
                    }
                }
            }
            14 => match r {
                0 => self.decode_operand_descriptor(bus, dtype, Some(Data::UWord), addr + 1, true),
                2 => self.decode_operand_descriptor(bus, dtype, Some(Data::UHalf), addr + 1, true),
                3 => self.decode_operand_descriptor(bus, dtype, Some(Data::Byte), addr + 1, true),
                4 => self.decode_operand_descriptor(bus, dtype, Some(Data::Word), addr + 1, true),
                6 => self.decode_operand_descriptor(bus, dtype, Some(Data::Half), addr + 1, true),
                7 => self.decode_operand_descriptor(bus, dtype, Some(Data::SByte), addr + 1, true),
                _ => Err(CpuError::Exception(CpuException::IllegalOpcode)),
            },
            15 => {
                // Negative Literal
                Ok(Operand::new(
                    1,
                    AddrMode::NegativeLiteral,
                    dtype,
                    etype,
                    None,
                    descriptor_byte as u32,
                ))
            }
            _ => Err(CpuError::Exception(CpuException::IllegalOpcode)),
        }
    }

    fn decode_operand(
        &self,
        bus: &mut Bus,
        mn: &Mnemonic,
        ot: &OpType,
        etype: Option<Data>,
        addr: usize,
    ) -> Result<Operand, CpuError> {
        match *ot {
            OpType::Lit => self.decode_operand_literal(bus, mn, addr),
            OpType::Src | OpType::Dest => {
                self.decode_operand_descriptor(bus, mn.dtype, etype, addr, false)
            }
        }
    }

    /// Decode the instruction currently pointed at by the Program Counter.
    /// Returns the number of bytes consumed, or a CpuError.
    fn decode_instruction(&self, bus: &mut Bus) -> Result<DecodedInstruction, CpuError> {
        // The next address to read from is pointed to by the PC
        let mut addr = self.r[R_PC] as usize;

        // Read a byte from memory
        let b1 = bus.read_byte(addr, AccessCode::InstrFetch)?;
        addr += 1;

        let mn: &Mnemonic = if b1 == 0x30 {
            // Special case for half-word opcodes
            let b2 = bus.read_byte(addr, AccessCode::InstrFetch)?;
            addr += 1;

            &OPCODES[b2 as usize]
        } else {
            &OPCODES[b1 as usize]
        };

        let mut operands: Vec<Operand> = Vec::new();
        let mut etype: Option<Data> = None;

        for ot in &mn.ops {
            // Push a decoded operand
            let o = self.decode_operand(bus, mn, ot, etype, addr)?;
            etype = o.expanded_type;
            addr += o.size as usize;
            operands.push(o);
        }

        let total_operand_bytes: u8 = operands.iter().map(|o: &Operand| o.size).sum();

        Ok(DecodedInstruction {
            bytes: total_operand_bytes + 1,
            mnemonic: mn,
            operands,
        })
    }

    /// Convenience operations on flags.
    fn set_c_flag(&mut self, set: bool) {
        if set {
            self.r[R_PSW] |= F_C;
        } else {
            self.r[R_PSW] &= !F_C;
        }
    }

    fn set_v_flag(&mut self, set: bool) {
        if set {
            self.r[R_PSW] |= F_V;
        } else {
            self.r[R_PSW] &= !F_V;
        }
    }

    fn set_z_flag(&mut self, set: bool) {
        if set {
            self.r[R_PSW] |= F_Z;
        } else {
            self.r[R_PSW] &= !F_Z;
        }
    }

    fn set_n_flag(&mut self, set: bool) {
        if set {
            self.r[R_PSW] |= F_N;
        } else {
            self.r[R_PSW] &= !F_N;
        }
    }

    pub fn set_isc(&mut self, val: u32) {
        self.r[R_PSW] &= !F_ISC; // Clear existing value
        self.r[R_PSW] |= (val & 0xf) << 3; // Set new value
    }

    pub fn set_priv_level(&mut self, val: u32) {
        let old_level = (self.r[R_PSW] & F_CM) >> 11;
        self.r[R_PSW] &= !F_PM; // Clear PM
        self.r[R_PSW] |= (old_level & 3) << 9; // Set PM
        self.r[R_PSW] &= !F_CM; // Clear CM
        self.r[R_PSW] |= (val & 3) << 11; // Set CM
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bus::Bus;
    use mem::Mem;

    /// Helper function to set up and prepare a cpu and bus
    /// with a supplied program.
    fn do_with_program<F>(program: &[u8], test: F)
    where
        F: Fn(&mut Cpu, &mut Bus),
    {
        let mut cpu: Cpu = Cpu::new();
        let mut mem: Mem = Mem::new(0, 0x10000, false);
        let mut bus: Bus = Bus::new(0x10000);
        bus.add_device(&mut mem).unwrap();
        bus.load(0, &program).unwrap();

        test(&mut cpu, &mut bus);
    }

    #[test]
    fn zero_and_sign_extension() {
        assert_eq!(0xffff8000, sign_extend_halfword(0x8000));
        assert_eq!(0x00008000, zero_extend_halfword(0x8000));
        assert_eq!(0xffffff80, sign_extend_byte(0x80));
        assert_eq!(0x00000080, zero_extend_byte(0x80));
    }

    #[test]
    fn step_is_ok() {
        let program: [u8; 10] = [
            0x87, 0xe7, 0x40, 0xe2, 0xc1, 0x04, // MOVB {sbyte}%r0,{uhalf}4(%r1)
            0x87, 0xd2, 0x30, 0x43, // MOVB *0x30(%r2),%r3
        ];

        do_with_program(&program, |cpu, bus| {
            cpu.r[0] = 0x1f;
            cpu.r[1] = 0x300;
            assert!(cpu.step(bus).is_ok());
            assert_eq!(0x1f, bus.read_byte(0x304, AccessCode::AddressFetch).unwrap());
        });
    }

    #[test]
    fn can_set_and_clear_nzvc_flags() {
        let mut cpu = Cpu::new();
        cpu.set_c_flag(true);
        assert_eq!(cpu.r[R_PSW], F_C);
        cpu.set_v_flag(true);
        assert_eq!(cpu.r[R_PSW], F_C | F_V);
        cpu.set_z_flag(true);
        assert_eq!(cpu.r[R_PSW], F_C | F_V | F_Z);
        cpu.set_n_flag(true);
        assert_eq!(cpu.r[R_PSW], F_C | F_V | F_Z | F_N);
        cpu.set_c_flag(false);
        assert_eq!(cpu.r[R_PSW], F_V | F_Z | F_N);
        cpu.set_v_flag(false);
        assert_eq!(cpu.r[R_PSW], F_Z | F_N);
        cpu.set_z_flag(false);
        assert_eq!(cpu.r[R_PSW], F_N);
        cpu.set_n_flag(false);
        assert_eq!(cpu.r[R_PSW], 0);
    }

    #[test]
    fn can_set_isc_flag() {
        let mut cpu = Cpu::new();

        for i in 0..15 {
            cpu.set_isc(i);
            assert_eq!(i << 3, cpu.r[R_PSW]);
        }

        cpu.set_isc(16); // Out of range, should fail
        assert_eq!(0, cpu.r[R_PSW]);
    }

    #[test]
    fn decodes_byte_literal_operand() {
        let program: [u8; 2] = [0x4f, 0x06]; // BLEB 0x6

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_literal(&mut bus, &OPCODES[0x4F], 1)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(1, AddrMode::None, Data::Byte, None, None, 6)
            );
        })
    }

    #[test]
    fn decodes_halfword_literal_operand() {
        let program: [u8; 3] = [0x4e, 0xff, 0x0f]; // BLEH 0xfff

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_literal(&mut bus, &OPCODES[0x4e], 1)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(2, AddrMode::None, Data::Half, None, None, 0xfff)
            );
        })
    }

    #[test]
    fn decodes_word_literal_operand() {
        let program: [u8; 5] = [0x32, 0xff, 0x4f, 0x00, 0x00]; // SPOP 0x4fff

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_literal(&mut bus, &OPCODES[0x32], 1)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(4, AddrMode::None, Data::Word, None, None, 0x4fff)
            );
        });
    }

    #[test]
    fn decodes_positive_literal_operand() {
        let program: [u8; 3] = [0x87, 0x04, 0x44]; // MOVB &4,%r4

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_descriptor(&mut bus, Data::Byte, None, 1, false)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(1, AddrMode::PositiveLiteral, Data::Byte, None, None, 0x04)
            );
        });
    }

    #[test]
    fn decodes_word_immediate_operand() {
        let program = [0x84, 0x4f, 0x78, 0x56, 0x34, 0x12, 0x43]; // MOVW &0x12345678,%r3

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_descriptor(&mut bus, Data::Word, None, 1, false)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(
                    5,
                    AddrMode::WordImmediate,
                    Data::Word,
                    None,
                    None,
                    0x12345678
                )
            );
        });
    }

    #[test]
    fn decodes_register_operand() {
        let program: [u8; 3] = [0x87, 0x04, 0x44]; // MOVB &4,%r4

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_descriptor(&mut bus, Data::Byte, None, 2, false)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(1, AddrMode::Register, Data::Byte, None, Some(4), 0)
            );
        });
    }

    #[test]
    fn decodes_halfword_immediate_operand() {
        let program = [0x84, 0x5f, 0x34, 0x12, 0x42]; // MOVW &0x1234,%r2

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_descriptor(&mut bus, Data::Word, None, 1, false)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(
                    3,
                    AddrMode::HalfwordImmediate,
                    Data::Word,
                    None,
                    None,
                    0x1234
                )
            );
        });
    }

    #[test]
    fn decodes_register_deferred_operand() {
        let program: [u8; 3] = [0x86, 0x52, 0x41]; // MOVH (%r2),%r1

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_descriptor(&mut bus, Data::Half, None, 1, false)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(1, AddrMode::RegisterDeferred, Data::Half, None, Some(2), 0)
            );
        });
    }

    #[test]
    fn decodes_byte_immediate_operand() {
        let program: [u8; 4] = [0x84, 0x6f, 0x28, 0x46]; // MOVW &40,%r6

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_descriptor(&mut bus, Data::Word, None, 1, false)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(2, AddrMode::ByteImmediate, Data::Word, None, None, 40)
            );
        });
    }

    #[test]
    fn decodes_fp_short_offset_operand() {
        let program: [u8; 3] = [0x84, 0x6C, 0x40]; // MOVW 12(%fp),%r0

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_descriptor(&mut bus, Data::Word, None, 1, false)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(1, AddrMode::FPShortOffset, Data::Word, None, Some(R_FP), 12)
            );
        });
    }

    #[test]
    fn decodes_absolute_operand() {
        let program: [u8; 7] = [0x87, 0x7f, 0x00, 0x01, 0x00, 0x00, 0x40]; // MOVB $0x100, %r0

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_descriptor(&mut bus, Data::Byte, None, 1, false)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(5, AddrMode::Absolute, Data::Byte, None, None, 0x00000100)
            );
        });
    }

    #[test]
    fn decodes_ap_short_offset_operand() {
        let program: [u8; 3] = [0x84, 0x74, 0x43]; // MOVW 4(%ap),%r3

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_descriptor(&mut bus, Data::Word, None, 1, false)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(1, AddrMode::APShortOffset, Data::Word, None, Some(R_AP), 4)
            );
        });
    }

    #[test]
    fn decodes_word_displacement_operand() {
        let program: [u8; 7] = [0x87, 0x82, 0x34, 0x12, 0x00, 0x00, 0x44]; // MOVB 0x1234(%r2),%r4

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_descriptor(&mut bus, Data::Byte, None, 1, false)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(
                    5,
                    AddrMode::WordDisplacement,
                    Data::Byte,
                    None,
                    Some(2),
                    0x1234
                )
            );
        });
    }

    #[test]
    fn decodes_word_displacement_deferred_operand() {
        let program: [u8; 7] = [0x87, 0x92, 0x50, 0x40, 0x00, 0x00, 0x40]; // MOVB *0x4050(%r2),%r0

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_descriptor(&mut bus, Data::Byte, None, 1, false)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(
                    5,
                    AddrMode::WordDisplacementDeferred,
                    Data::Byte,
                    None,
                    Some(2),
                    0x4050
                )
            );
        });
    }

    #[test]
    fn decodes_halfword_displacement_operand() {
        let program: [u8; 5] = [0x87, 0xa2, 0x34, 0x12, 0x44]; // MOVB 0x1234(%r2),%r4

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_descriptor(&mut bus, Data::Byte, None, 1, false)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(
                    3,
                    AddrMode::HalfwordDisplacement,
                    Data::Byte,
                    None,
                    Some(2),
                    0x1234
                )
            );
        });
    }

    #[test]
    fn decodes_halfword_displacement_deferred_operand() {
        let program: [u8; 5] = [0x87, 0xb2, 0x50, 0x40, 0x40]; // MOVB *0x4050(%r2),%r0

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_descriptor(&mut bus, Data::Byte, None, 1, false)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(
                    3,
                    AddrMode::HalfwordDisplacementDeferred,
                    Data::Byte,
                    None,
                    Some(2),
                    0x4050
                )
            );
        });
    }

    #[test]
    fn decodes_byte_displacement_operand() {
        let program: [u8; 4] = [0x87, 0xc1, 0x06, 0x40]; // MOVB 6(%r1),%r0

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_descriptor(&mut bus, Data::Byte, None, 1, false)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(2, AddrMode::ByteDisplacement, Data::Byte, None, Some(1), 6)
            );
        });
    }

    #[test]
    fn decodes_byte_displacement_deferred_operand() {
        let program: [u8; 4] = [0x87, 0xd2, 0x30, 0x43]; // MOVB *0x30(%r2),%r3

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_descriptor(&mut bus, Data::Byte, None, 1, false)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(
                    2,
                    AddrMode::ByteDisplacementDeferred,
                    Data::Byte,
                    None,
                    Some(2),
                    0x30
                )
            );
        });
    }

    #[test]
    fn decodes_expanded_type_operand() {
        let program: [u8; 6] = [0x87, 0xe7, 0x40, 0xe2, 0xc1, 0x04]; // MOVB {sbyte}%r0,{uhalf}4(%r1)

        do_with_program(&program, |cpu, mut bus| {
            let op1 = cpu
                .decode_operand_descriptor(&mut bus, Data::Byte, None, 1, false)
                .unwrap();
            let op2 = cpu
                .decode_operand_descriptor(&mut bus, Data::Byte, None, 3, false)
                .unwrap();

            assert_eq!(
                op1,
                Operand::new(
                    2,
                    AddrMode::Register,
                    Data::Byte,
                    Some(Data::SByte),
                    Some(0),
                    0
                )
            );
            assert_eq!(
                op2,
                Operand::new(
                    3,
                    AddrMode::ByteDisplacement,
                    Data::Byte,
                    Some(Data::UHalf),
                    Some(1),
                    4
                )
            );
        });
    }

    #[test]
    fn decodes_negative_literal_operand() {
        let program: [u8; 3] = [0x87, 0xff, 0x40]; // MOVB &-1,%r0

        do_with_program(&program, |cpu, mut bus| {
            let operand = cpu
                .decode_operand_descriptor(&mut bus, Data::Byte, None, 1, false)
                .unwrap();
            assert_eq!(
                operand,
                Operand::new(1, AddrMode::NegativeLiteral, Data::Byte, None, None, 0xff)
            );
        });
    }

    #[test]
    fn decodes_instructions() {
        let program: [u8; 10] = [
            0x87, 0xe7, 0x40, 0xe2, 0xc1, 0x04, // MOVB {sbyte}%r0,{uhalf}4(%r1)
            0x87, 0xd2, 0x30, 0x43, // MOVB *0x30(%r2),%r3
        ];

        do_with_program(&program, |cpu, bus| {
            {
                cpu.set_pc(0);
                let inst = cpu.decode_instruction(bus).unwrap();
                let expected_operands = vec![
                    Operand::new(
                        2,
                        AddrMode::Register,
                        Data::Byte,
                        Some(Data::SByte),
                        Some(0),
                        0,
                    ),
                    Operand::new(
                        3,
                        AddrMode::ByteDisplacement,
                        Data::Byte,
                        Some(Data::UHalf),
                        Some(1),
                        4,
                    ),
                ];
                assert_eq!(
                    inst,
                    DecodedInstruction {
                        bytes: 6,
                        mnemonic: &OPCODES[0x87],
                        operands: expected_operands
                    }
                );
            }
            {
                cpu.set_pc(6);
                let inst = cpu.decode_instruction(bus).unwrap();
                let expected_operands = vec![
                    Operand::new(
                        2,
                        AddrMode::ByteDisplacementDeferred,
                        Data::Byte,
                        None,
                        Some(2),
                        0x30,
                    ),
                    Operand::new(1, AddrMode::Register, Data::Byte, None, Some(3), 0),
                ];
                assert_eq!(
                    inst,
                    DecodedInstruction {
                        bytes: 4,
                        mnemonic: &OPCODES[0x87],
                        operands: expected_operands
                    }
                );
            }
        })
    }

    #[test]
    fn reads_register_operand_data() {
        {
            let program = [0x87, 0xe7, 0x40, 0xe2, 0x41]; // MOVB {sbyte}%r0,{uhalf}%r1
            do_with_program(&program, |cpu, mut bus| {
                cpu.r[0] = 0xff;
                let op = cpu
                    .decode_operand_descriptor(&mut bus, Data::Byte, None, 1, false)
                    .unwrap();
                assert_eq!(0xffffffff, cpu.read_op(bus, &op).unwrap());
            });
        }

        {
            let program = [0x87, 0x40, 0x41]; // MOVB %r0,%r1
            do_with_program(&program, |cpu, mut bus| {
                cpu.r[0] = 0xff;
                let op = cpu
                    .decode_operand_descriptor(&mut bus, Data::Byte, None, 1, false)
                    .unwrap();
                assert_eq!(0xff, cpu.read_op(bus, &op).unwrap());
            });
        }
    }

    #[test]
    fn reads_positive_literal_operand_data() {
        let program = [0x87, 0x04, 0x44];
        do_with_program(&program, |cpu, mut bus| {
            let op = cpu
                .decode_operand_descriptor(&mut bus, Data::Byte, None, 1, false)
                .unwrap();
            assert_eq!(4, cpu.read_op(bus, &op).unwrap() as i8);
        });
    }

    #[test]
    fn reads_negative_literal_operand_data() {
        let program = [0x87, 0xff, 0x44];
        do_with_program(&program, |cpu, mut bus| {
            let op = cpu
                .decode_operand_descriptor(&mut bus, Data::Byte, None, 1, false)
                .unwrap();
            assert_eq!(-1, cpu.read_op(bus, &op).unwrap() as i8);
        });
    }

    #[test]
    fn reads_word_immediate_operand_data() {
        let program = [0x84, 0x4f, 0x78, 0x56, 0x34, 0x12, 0x43]; // MOVW &0x12345678,%r3
        do_with_program(&program, |cpu, mut bus| {
            let op = cpu
                .decode_operand_descriptor(&mut bus, Data::Word, None, 1, false)
                .unwrap();
            assert_eq!(0x12345678, cpu.read_op(bus, &op).unwrap())
        });
    }

    #[test]
    fn reads_halfword_immediate_operand_data() {
        let program = [0x84, 0x5f, 0x34, 0x12, 0x42]; // MOVW &0x1234,%r2
        do_with_program(&program, |cpu, mut bus| {
            let op = cpu
                .decode_operand_descriptor(&mut bus, Data::Word, None, 1, false)
                .unwrap();
            assert_eq!(0x1234, cpu.read_op(bus, &op).unwrap())
        });
    }

    #[test]
    fn reads_negative_halfword_immediate_operand_data() {
        let program = [0x84, 0x5f, 0x00, 0x80, 0x42]; // MOVW &0x8000,%r2
        do_with_program(&program, |cpu, mut bus| {
            let op = cpu
                .decode_operand_descriptor(&mut bus, Data::Word, None, 1, false)
                .unwrap();
            assert_eq!(0xffff8000, cpu.read_op(bus, &op).unwrap())
        });
    }

    #[test]
    fn reads_byte_immediate_operand_data() {
        let program = [0x84, 0x6f, 0x28, 0x42]; // MOVW &40,%r2
        do_with_program(&program, |cpu, mut bus| {
            let op = cpu
                .decode_operand_descriptor(&mut bus, Data::Word, None, 1, false)
                .unwrap();
            assert_eq!(40, cpu.read_op(bus, &op).unwrap())
        });
    }

    #[test]
    fn reads_negative_byte_immediate_operand_data() {
        let program = [0x84, 0x6f, 0xff, 0x42]; // MOVW &-1,%r2
        do_with_program(&program, |cpu, mut bus| {
            let op = cpu
                .decode_operand_descriptor(&mut bus, Data::Word, None, 1, false)
                .unwrap();
            assert_eq!(-1, cpu.read_op(bus, &op).unwrap() as i32)
        });
    }

    #[test]
    fn reads_absolute_operand_data() {
        // TODO: Implement
    }
}
