pub enum InterruptType {
    NMI,
    BRK,
}

#[allow(dead_code)]
pub struct Interrupt {
    pub interrupt_type: InterruptType,
    pub vector: u16,
    pub mask: u8,
    pub cycles: u8,
}

pub const NMI: Interrupt = Interrupt {
    interrupt_type: InterruptType::NMI,
    vector: 0xFFFA,
    mask: 0b0010_0000,
    cycles: 2,
};

pub const BRK: Interrupt = Interrupt {
    interrupt_type: InterruptType::BRK,
    vector: 0xFFFE,
    mask: 0b0011_0000,
    cycles: 1,
};