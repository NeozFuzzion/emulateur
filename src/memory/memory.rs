use core::panic;
use std::sync::mpsc::Sender;
use crate::cpu::clock::Clock;

use crate::gpu::gpu::GPU;
use crate::input::Input;
use crate::cartridge::MemoryBankController;


pub struct MemoryBus {
    pub(crate) rom: Box<dyn MemoryBankController>,
    pub(crate) wram: [u8; 0x2000], 
    pub(crate) hram: [u8; 0x80],
    pub(crate) gpu: GPU,
    pub(crate) screen_sender:Sender<[u32;23040]>,
    pub(crate) interrupt_flags: u8,
    pub(crate) interrupt_enabled: u8,
    pub(crate) input: Input,
    pub(crate) clock: Clock,
}

impl MemoryBus {
    pub fn read_byte(&self, address: u16) -> u8 {
        //println!("Read : {:x}",address);
        //println!("Hram : {:x}",self.hram[(address & 0x7F) as usize]);
        match address {
            0x0000..=0x7FFF | 0xA000..=0xBFFF => self.rom.read_byte(address), // ROM and cart RAM
            0x8000..=0x9FFF => self.gpu.read_vram(address),             // Load from gpu
            0xC000..=0xFDFF => self.wram[(address & 0x1FFF) as usize],        // Working RAM
            0xFE00..=0xFE9F =>  self.gpu.read_oam(address),                    // Graphics - sprite information
            0xFF00 => self.input.read(),                                   // Input read
            0xFF01..=0xFF02 => panic!("RSerial"),                     // Serial read
            0xFF04..=0xFF07 => self.clock.read(address),                 // read Clock values
            0xFF0F => self.interrupt_flags,                                // Interrupt flags
            //0xFF10..=0xFF26 => panic!("RSound"),                 // Sound control
            //0xFF30..=0xFF3F => panic!("RSound"),                 // Sound wave pattern RAM
            0xFF40..=0xFF4B => self.gpu.read_lcd_reg(address),
            0xFF4C..=0xFF7F => panic!("MMU ERROR: memory mapped I/O (read) (CGB only) not implemented"),
            0xFF80..=0xFFFE =>self.hram[(address & 0x7F) as usize], // High RAM
            0xFFFF => self.interrupt_enabled,                     // Interrupt enable
            _ => 0,
        }
    }

    pub fn write_byte(&mut self, address: u16, byte: u8) {
        //println!("{:x}",address);
        match address {
            0x0000..=0x7FFF | 0xA000..=0xBFFF => self.rom.write_byte(address,byte),                             // ROM and cart RAM
            0x8000..=0x9FFF => self.gpu.write_vram(address,byte),                                         // Write to gpu
            0xC000..=0xFDFF => self.wram[(address & 0x1FFF) as usize] = byte,                                   // Working RAM
            0xFE00..=0xFE9F => self.gpu.write_oam(address,byte),                                          // Graphics - sprite information
            0xFF00 => self.input.write(byte),                                                             // Input write
            //0xFF01..=0xFF02 => panic!("WSerial"),                                                             // Serial write
            0xFF04..=0xFF07 => self.clock.write(address,byte),                                            // write Clock values
            0xFF0F => self.interrupt_flags = byte,                                                              // Interrupt flags
            //0xFF10..=0xFF26 => panic!("WSound"),                                                              // Sound control
            //0xFF30..=0xFF3F => panic!("WSound"),                                                              // Sound wave pattern RAM
            0xFF46 => self.dma_into_oam(byte),
            0xFF40..=0xFF45 | 0xFF47..=0xFF4B => self.gpu.write_lcd_reg(address,byte),
            /*0xFF4C..=0xFF7F => panic!(
                "MMU ERROR: memory mapped I/O (write) (CGB only) not implemented. Addr: 0x{:X}",
                addr
            ),*/
            0xFF80..=0xFFFE =>self.hram[(address & 0x7F) as usize] = byte,                                      // High RAM
            0xFFFF => self.interrupt_enabled = byte,                                                            // Interrupt enable
            _ => (),
        }
    }

    pub fn init(&mut self){
        self.write_byte(0xFF05, 0);
        self.write_byte(0xFF06, 0);
        self.write_byte(0xFF07, 0);
        self.write_byte(0xFF10, 0x80);
        self.write_byte(0xFF11, 0xBF);
        self.write_byte(0xFF12, 0xF3);
        self.write_byte(0xFF14, 0xBF);
        self.write_byte(0xFF16, 0x3F);
        self.write_byte(0xFF16, 0x3F);
        self.write_byte(0xFF17, 0);
        self.write_byte(0xFF19, 0xBF);
        self.write_byte(0xFF1A, 0x7F);
        self.write_byte(0xFF1B, 0xFF);
        self.write_byte(0xFF1C, 0x9F);
        self.write_byte(0xFF1E, 0xFF);
        self.write_byte(0xFF20, 0xFF);
        self.write_byte(0xFF21, 0);
        self.write_byte(0xFF22, 0);
        self.write_byte(0xFF23, 0xBF);
        self.write_byte(0xFF24, 0x77);
        self.write_byte(0xFF25, 0xF3);
        self.write_byte(0xFF26, 0xF1);
        self.write_byte(0xFF40, 0x91);
        self.write_byte(0xFF42, 0);
        self.write_byte(0xFF43, 0);
        self.write_byte(0xFF45, 0);
        self.write_byte(0xFF47, 0xFC);
        self.write_byte(0xFF48, 0xFF);
        self.write_byte(0xFF49, 0xFF);
        self.write_byte(0xFF4A, 0);
        self.write_byte(0xFF4B, 0);
    }

    pub fn read_word(&self, address: u16) -> u16 {
        u16::from(self.read_byte(address)) | (u16::from(self.read_byte(address + 1)) << 8)
    }

    pub fn write_word(&mut self, addr: u16, word: u16) {
        self.write_byte(addr, (word & 0xFF) as u8);
        self.write_byte(addr + 1 ,((word >> 8) & 0xFF) as u8);
    }

    pub fn run(&mut self,cycle:u8){
        self.gpu.run(self.screen_sender.clone(),cycle);
        self.interrupt_flags |= self.gpu.interrupt;
        self.gpu.interrupt = 0;

        self.input.run();
        self.interrupt_flags |= self.input.interrupt;
        self.input.interrupt = 0;

        self.clock.run((cycle * 4) as u32);
        self.interrupt_flags |= self.clock.interrupt;
        self.clock.interrupt=0;
    }

    fn dma_into_oam(&mut self, dma_start: u8) {
        // DMA start can be addressed as 0x0000, 0x0100, 0x0200, etc
        let actual_dma_start = u16::from(dma_start) << 8; // turns 0x01 to 0x0100
        for i in 0..(0xA0_u16) {
            let value = self.read_byte(actual_dma_start + i);
            self.gpu.write_oam(i, value);
        }
    }
}
