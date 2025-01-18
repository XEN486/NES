pub enum InterruptType {
    NMI,
    BRK,
}

pub struct Interrupt {
    pub interrupt_type: InterruptType,
    pub vector_address: u16,
    pub flag_mask: u8,
    pub cycles: u8,
}

pub const NMI: Interrupt = Interrupt {
    interrupt_type: InterruptType::NMI,
    vector_address: 0xFFFA,
    flag_mask: 0b0010_0000,
    cycles: 2,
};

pub const BRK: Interrupt = Interrupt {
    interrupt_type: InterruptType::BRK,
    vector_address: 0xFFFE,
    flag_mask: 0b0011_0000,
    cycles: 1,
};