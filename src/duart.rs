use crate::bus::AccessCode;
use crate::bus::Device;
use crate::err::BusError;
use std::fmt::Debug;
use std::fmt::Error;
use std::fmt::Formatter;
use std::ops::Range;
use std::time::Duration;
use std::time::Instant;
use crate::err::DuartError;

const START_ADDR: usize = 0x200000;
const END_ADDR: usize = 0x2000040;
const ADDRESS_RANGE: Range<usize> = START_ADDR..END_ADDR;

// Vertical blanks should occur at 60Hz. This value is in nanoseconds
const VERTICAL_BLANK_DELAY: u32 = 16_666_666;  // 60 Hz

// Delay rates selected when ACR[7] = 0
const DELAY_RATES_A: [u32;13] = [
    200000000, 90909096, 74074072, 50000000,
    33333336, 16666668, 8333334, 9523810,
    4166667, 2083333, 1388888, 1041666, 260416,
];

// Delay rates selected when ACR[7] = 1
const DELAY_RATES_B: [u32;13] = [
    133333344, 90909096, 74074072, 66666672,
    33333336, 16666668, 8333334, 5000000,
    4166667, 205338, 5555555, 1041666, 520833,
];

const PORT_0: usize = 0;
const PORT_1: usize = 1;

//
// Registers
//
const MR12A: u8 = 0x03;
const CSRA: u8 = 0x07;
const CRA: u8 = 0x0b;
const THRA: u8 = 0x0f;
const IPCR_ACR: u8 = 0x13;
const ISR_MASK: u8 = 0x17;
const MR12B: u8 = 0x23;
const CSRB: u8 = 0x27;
const CRB: u8 = 0x2b;
const THRB: u8 = 0x2f;
const IP_OPCR: u8 = 0x37;


//
// Port Configuration Bits
//
const CNF_ETX: u8 = 0x01;
const CNF_ERX: u8 = 0x02;

//
// Status Flags
//
const STS_RXR: u8 = 0x01;
const STS_TXR: u8 = 0x04;
const STS_TXE: u8 = 0x08;
const STS_OER: u8 = 0x10;
const STS_PER: u8 = 0x20;
const STS_FER: u8 = 0x40;

//
// Commands
//
const CMD_ERX: u8 = 0x01;
const CMD_DRX: u8 = 0x02;
const CMD_ETX: u8 = 0x04;
const CMD_DTX: u8 = 0x08;

//
// Interrupt Status Register
//
const ISTS_TAI: u8 = 0x01;
const ISTS_RAI: u8 = 0x02;
const ISTS_RBI: u8 = 0x20;
const ISTS_IPC: u8 = 0x80;

//
// Interrupt Masks
//
const KEYBOARD_INT: u8 = 0x04;
const MOUSE_BLANK_INT: u8 = 0x02;
const TX_INT: u8 = 0x10;
const RX_INT: u8 = 0x20;

struct Port {
    mode: [u8;2],
    stat: u8,
    conf: u8,
    rx_data: u8,
    tx_data: u8,
    mode_ptr: usize,
    rx_pending: bool,
    tx_pending: bool,
    char_delay: Duration,
    next_rx: Instant,
    next_tx: Instant,
}

pub struct Duart {
    ports: [Port; 2],
    acr: u8,
    ipcr: u8,
    inprt: u8,
    istat: u8,
    imr: u8,
    ivec: u8,
    last_vblank: Instant,
    tx_callback: Option<Box<FnMut(u8) + Send + Sync>>,
}

impl Duart {
    pub fn new<CB: 'static + FnMut(u8) + Send + Sync>(tx_callback: CB) -> Duart {
        Duart {
            ports: [
                Port {
                    mode: [0; 2],
                    stat: 0,
                    conf: 0,
                    rx_data: 0,
                    tx_data: 0,
                    mode_ptr: 0,
                    rx_pending: false,
                    tx_pending: false,
                    char_delay: Duration::new(0, 1_000_000),
                    next_rx: Instant::now(),
                    next_tx: Instant::now(),
                },
                Port {
                    mode: [0; 2],
                    stat: 0,
                    conf: 0,
                    rx_data: 0,
                    tx_data: 0,
                    mode_ptr: 0,
                    rx_pending: false,
                    tx_pending: false,
                    char_delay: Duration::new(0, 1_000_000),
                    next_rx: Instant::now(),
                    next_tx: Instant::now(),
                },
            ],
            acr: 0,
            ipcr: 0x40,
            inprt: 0xb,
            istat: 0,
            imr: 0,
            ivec: 0,
            last_vblank: Instant::now(),
            tx_callback: Some(Box::new(tx_callback)),
        }
    }

    pub fn get_interrupt(&mut self) -> Option<u8> {
        let new_vblank_time: Instant = self.last_vblank + Duration::new(0, VERTICAL_BLANK_DELAY);

        if Instant::now() > new_vblank_time {
            self.last_vblank = Instant::now();
            self.vertical_blank();
        }

        let val = self.ivec;

        if val == 0 {
            None
        } else {
            Some(val)
        }
    }

    pub fn service(&mut self) {
        let mut ctx = &mut self.ports[PORT_0];

        if ctx.tx_pending && Instant::now() >= ctx.next_tx {
            // Finish our transmit.
            let c = ctx.tx_data;
            ctx.conf |= CNF_ETX;
            ctx.stat |= STS_TXR;
            ctx.stat |= STS_TXE;
            self.istat |= ISTS_TAI;
            self.ivec |= TX_INT;
            ctx.tx_pending = false;
            if (ctx.mode[1] >> 6) & 3 == 0x2 {
                // Loopback Mode.
                ctx.rx_data = c;
                ctx.stat |= STS_RXR;
                self.istat |= ISTS_RAI;
                self.ivec |= RX_INT;
            } else {
                match &mut self.tx_callback {
                    Some(cb) => (cb)(c),
                    None => {}
                };
            }
        }
    }

    pub fn handle_keyboard(&mut self, val: u8) {
        let mut ctx = &mut self.ports[PORT_1];
        ctx.rx_data = val;
        ctx.stat |= STS_RXR;
        self.istat |= ISTS_RBI;
        self.ivec |= KEYBOARD_INT;
    }

    pub fn vertical_blank(&mut self) {
        self.ivec |= MOUSE_BLANK_INT;
        self.ipcr |= 0x40;
        self.istat |= ISTS_IPC;

        if self.inprt & 0x04 == 0 {
            self.ipcr |= 0x40;
        } else {
            self.inprt &= !0x04;
        }
    }

    pub fn mouse_down(&mut self, button: u8) {
        self.ipcr = 0;
        self.inprt |= 0xb;
        self.istat |= ISTS_IPC;
        self.ivec |= MOUSE_BLANK_INT;
        match button {
            0 => {
                self.ipcr |= 0x80;
                self.inprt &= !(0x08);
            }
            1 => {
                self.ipcr |= 0x20;
                self.inprt &= !(0x02);
            }
            2 => {
                self.ipcr |= 0x10;
                self.inprt &= !(0x01)
            }
            _ => {}
        }
    }

    pub fn mouse_up(&mut self, button: u8) {
        self.ipcr = 0;
        self.inprt |= 0xb;
        self.istat |= ISTS_IPC;
        self.ivec |= MOUSE_BLANK_INT;
        match button {
            0 => {
                self.ipcr |= 0x80;
            }
            1 => {
                self.ipcr |= 0x20;
            }
            2 => {
                self.ipcr |= 0x10;
            }
            _ => {}
        }
    }

    pub fn rx_ready(&self) -> bool {
        let ctx = &self.ports[PORT_0];

        return (ctx.stat & STS_RXR) != 0;
    }

    pub fn rx_char(&mut self, c: u8) -> Result<(), DuartError> {
        let mut ctx = &mut self.ports[PORT_0];

        if ctx.rx_pending {
            if Instant::now() > ctx.next_rx {
                if ctx.conf & CNF_ERX != 0 {
                    ctx.rx_pending = false;
                    ctx.rx_data = c;
                    ctx.stat |= STS_RXR;
                    self.istat |= ISTS_RAI;
                    self.ivec |= RX_INT;
                } else {
                    ctx.stat |= STS_OER;
                }
                Ok(())
            } else {
                Err(DuartError::ReceiverNotReady)
            }
        } else {
            ctx.next_rx = Instant::now() + ctx.char_delay;
            ctx.rx_pending = true;
            Err(DuartError::ReceiverNotReady)
        }
    }

    pub fn handle_command(&mut self, cmd: u8, port: usize) {
        if cmd == 0 {
            return;
        }

        let mut ctx = &mut self.ports[port];

        // Enable or disable transmitter
        if cmd & CMD_DTX != 0 {
            ctx.conf &= !CNF_ETX;
            ctx.stat &= !STS_TXR;
            ctx.stat &= !STS_TXE;
            if port == PORT_0 {
                self.ivec &= !TX_INT;
                self.istat &= !ISTS_TAI;
            }
        } else if cmd & CMD_ETX != 0 {
            ctx.conf |= CNF_ETX;
            ctx.stat |= STS_TXR;
            ctx.stat |= STS_TXE;
            if port == PORT_0 {
                self.istat |= ISTS_TAI;
                self.ivec |= TX_INT;
            }
        }

        // Enable or disable receiver
        if cmd & CMD_DRX != 0 {
            ctx.conf &= !CNF_ERX;
            ctx.stat &= !STS_RXR;
            if port == PORT_0 {
                self.ivec &= !RX_INT;
                self.istat &= !ISTS_RAI;
            } else {
                self.ivec &= !KEYBOARD_INT;
                self.istat &= !ISTS_RBI;
            }
        } else if cmd & CMD_ERX != 0 {
            ctx.conf |= CNF_ERX;
            ctx.stat |= STS_RXR;
        }

        // Extra commands
        match (cmd >> 4) & 7 {
            1 => ctx.mode_ptr = 0,
            2 => {
                ctx.stat |= STS_RXR;
                ctx.conf |= CNF_ERX;
            }
            3 => {
                ctx.stat |= STS_TXR;
                ctx.stat |= STS_TXE;
                ctx.conf &= !CNF_ETX;
            }
            4 => ctx.stat &= !(STS_FER|STS_PER|STS_OER),
            _ => {}
        }
    }
}

impl Debug for Duart {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "[DUART]")
    }
}

impl Device for Duart {
    fn address_range(&self) -> &Range<usize> {
        &ADDRESS_RANGE
    }

    fn name(&self) -> &str {
        "ACIA"
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn read_byte(&mut self, address: usize, _access: AccessCode) -> Result<u8, BusError> {
        match (address - START_ADDR) as u8 {
            MR12A => {
                let mut ctx = &mut self.ports[PORT_0];
                let val = ctx.mode[ctx.mode_ptr];
                ctx.mode_ptr = (ctx.mode_ptr + 1) % 2;
                Ok(val)
            }
            CSRA => {
                Ok(self.ports[PORT_0].stat)
            }
            THRA => {
                let mut ctx = &mut self.ports[PORT_0];
                ctx.stat &= !STS_RXR;
                self.istat &= !ISTS_RAI;
                self.ivec &= !RX_INT;
                Ok(ctx.rx_data)
            }
            IPCR_ACR => {
                let result = self.ipcr;
                self.ipcr &= !0x0f;
                self.ivec = 0;
                self.istat &= !ISTS_IPC;
                Ok(result)
            }
            ISR_MASK => {
                Ok(self.istat)
            }
            MR12B => {
                let mut ctx = &mut self.ports[PORT_1];
                let val = ctx.mode[ctx.mode_ptr];
                ctx.mode_ptr = (ctx.mode_ptr + 1) % 2;
                Ok(val)
            }
            CSRB => {
                Ok(self.ports[PORT_1].stat)
            }
            THRB => {
                let mut ctx = &mut self.ports[PORT_1];
                ctx.stat &= !STS_RXR;
                self.istat &= !ISTS_RBI;
                self.ivec &= !KEYBOARD_INT;
                Ok(ctx.rx_data)
            }
            IP_OPCR => {
                Ok(self.inprt)
            }
            _ => Ok(0),
        }
    }

    fn read_half(&mut self, _address: usize, _access: AccessCode) -> Result<u16, BusError> {
        unimplemented!()
    }

    fn read_word(&mut self, _address: usize, _access: AccessCode) -> Result<u32, BusError> {
        unimplemented!()
    }

    fn write_byte(&mut self, address: usize, val: u8, _access: AccessCode) -> Result<(), BusError> {
        match (address - START_ADDR) as u8 {
            MR12A => {
                let mut ctx = &mut self.ports[PORT_0];
                ctx.mode[ctx.mode_ptr] = val;
                ctx.mode_ptr = (ctx.mode_ptr + 1) % 2;
            }
            CSRA => {
                // Set the baud rate.
                let baud_bits: usize = ((val >> 4) & 0xf) as usize;
                let delay = if self.acr & 0x80 == 0 {
                    DELAY_RATES_A[baud_bits]
                } else {
                    DELAY_RATES_B[baud_bits]
                };
                let mut ctx = &mut self.ports[PORT_0];
                ctx.char_delay = Duration::new(0, delay);
            }
            CRA => {
                self.handle_command(val, PORT_0);
            }
            THRA => {
                let mut ctx = &mut self.ports[PORT_0];
                ctx.tx_data = val;
                // Update state. Since we're transmitting,
                // the transmitter buffer is not empty.
                // The actual transmit will happen in the 'service'
                // function.
                ctx.next_tx = Instant::now() + ctx.char_delay;
                ctx.tx_pending = true;
                ctx.stat &= !(STS_TXE | STS_TXR);
                self.ivec &= !TX_INT;
                self.istat &= !ISTS_TAI;
            }
            IPCR_ACR => {
                self.acr = val;
            }
            ISR_MASK => {
                self.imr = val;
            }
            MR12B => {
                let mut ctx = &mut self.ports[PORT_1];
                ctx.mode[ctx.mode_ptr] = val;
                ctx.mode_ptr = (ctx.mode_ptr + 1) % 2;
            }
            CRB => {
                self.handle_command(val, PORT_1);
            }
            THRB => {
                let mut ctx = &mut self.ports[PORT_1];
                ctx.tx_data = val;
                // Special case for status requests from the keyboard
                if val == 0x02 {
                    ctx.stat = STS_RXR | STS_PER;
                }
            }
            IP_OPCR => {
                // Not implemented
            }
            _ => {}
        };

        Ok(())
    }

    fn write_half(&mut self, _address: usize, _val: u16, _access: AccessCode) -> Result<(), BusError> {
        unimplemented!()
    }

    fn write_word(&mut self, _address: usize, _val: u32, _access: AccessCode) -> Result<(), BusError> {
        unimplemented!()
    }

    fn load(&mut self, _address: usize, _data: &[u8]) -> Result<(), BusError> {
        unimplemented!()
    }
}
