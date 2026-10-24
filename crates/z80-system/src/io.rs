//! The I/O port space and the devices attached to it.
//!
//! A Z80 puts the full 16-bit address on the bus during an I/O cycle, but
//! most small systems decode only the low eight lines, so ports are indexed
//! by the low byte while the full address stays available to devices that
//! want it.

use std::collections::VecDeque;

pub const PORT_COUNT: usize = 0x100;

/// One recorded `IN` or `OUT`, for the I/O log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortEvent {
    pub address: u16,
    pub value: u8,
    pub is_input: bool,
}

/// A device that claims one or more ports.
pub trait Device: Send {
    /// Ports this device answers on, as low-byte addresses.
    fn ports(&self) -> Vec<u8>;

    /// Supply a byte for an `IN`. Returning `None` leaves the latched port
    /// value in place.
    fn input(&mut self, _address: u16) -> Option<u8> {
        None
    }

    /// Receive a byte from an `OUT`.
    fn output(&mut self, _address: u16, _value: u8) {}

    /// Advance any internal timing by the cycles the last instruction took.
    fn tick(&mut self, _tstates: u32) {}

    /// True while the device is requesting a maskable interrupt.
    fn interrupt_pending(&self) -> bool {
        false
    }

    /// The byte this device puts on the bus when its interrupt is
    /// acknowledged, used by interrupt modes 0 and 2.
    fn interrupt_vector(&self) -> u8 {
        0xFF
    }

    /// A short name for the user interface.
    fn name(&self) -> &str;
}

pub struct PortSpace {
    /// Last value seen on each port, which is what the ports editor shows
    /// and what an unclaimed `IN` returns.
    values: [u8; PORT_COUNT],
    access: [u8; PORT_COUNT],
    devices: Vec<Box<dyn Device>>,
    /// Which device, if any, owns each port.
    owners: [Option<usize>; PORT_COUNT],
    /// Values queued by the user for the "prompt before IN" option.
    pub input_queue: VecDeque<u8>,
    /// Recent port activity, newest last.
    pub log: VecDeque<PortEvent>,
    pub log_limit: usize,
    pub logging: bool,
}

impl Default for PortSpace {
    fn default() -> Self {
        Self::new()
    }
}

impl PortSpace {
    pub fn new() -> Self {
        Self {
            values: [0; PORT_COUNT],
            access: [0; PORT_COUNT],
            devices: Vec::new(),
            owners: [None; PORT_COUNT],
            input_queue: VecDeque::new(),
            log: VecDeque::new(),
            log_limit: 4096,
            logging: false,
        }
    }

    /// Attach a device, giving it ownership of the ports it claims. A port
    /// claimed twice goes to the device attached last.
    pub fn attach(&mut self, device: Box<dyn Device>) {
        let index = self.devices.len();
        for port in device.ports() {
            self.owners[port as usize] = Some(index);
        }
        self.devices.push(device);
    }

    pub fn devices(&self) -> &[Box<dyn Device>] {
        &self.devices
    }

    pub fn device_mut(&mut self, index: usize) -> Option<&mut Box<dyn Device>> {
        self.devices.get_mut(index)
    }

    pub fn detach_all(&mut self) {
        self.devices.clear();
        self.owners = [None; PORT_COUNT];
    }

    pub fn value(&self, port: u8) -> u8 {
        self.values[port as usize]
    }

    /// Set a port's value from the ports editor, without involving a device.
    pub fn set_value(&mut self, port: u8, value: u8) {
        self.values[port as usize] = value;
    }

    pub fn values(&self) -> &[u8; PORT_COUNT] {
        &self.values
    }

    pub fn access_flags(&self) -> &[u8; PORT_COUNT] {
        &self.access
    }

    pub fn clear_access(&mut self) {
        self.access.fill(0);
    }

    pub fn input(&mut self, address: u16) -> u8 {
        let port = address as u8;

        // A value the user queued takes priority over everything else.
        let value = if let Some(queued) = self.input_queue.pop_front() {
            queued
        } else {
            match self.owners[port as usize].and_then(|index| self.devices[index].input(address)) {
                Some(value) => value,
                None => self.values[port as usize],
            }
        };

        self.values[port as usize] = value;
        self.access[port as usize] |= crate::memory::access::READ;
        self.record(address, value, true);
        value
    }

    pub fn output(&mut self, address: u16, value: u8) {
        let port = address as u8;
        self.values[port as usize] = value;
        self.access[port as usize] |= crate::memory::access::WRITE;

        if let Some(index) = self.owners[port as usize] {
            self.devices[index].output(address, value);
        }
        self.record(address, value, false);
    }

    pub fn tick(&mut self, tstates: u32) {
        for device in &mut self.devices {
            device.tick(tstates);
        }
    }

    /// The first device asserting an interrupt, with its vector.
    pub fn pending_interrupt(&self) -> Option<u8> {
        self.devices
            .iter()
            .find(|device| device.interrupt_pending())
            .map(|device| device.interrupt_vector())
    }

    fn record(&mut self, address: u16, value: u8, is_input: bool) {
        if !self.logging {
            return;
        }
        if self.log.len() >= self.log_limit {
            self.log.pop_front();
        }
        self.log.push_back(PortEvent { address, value, is_input });
    }
}
