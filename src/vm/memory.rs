use gdbstub::target::ext::breakpoints::WatchKind;
use rrs_lib::MemAccessSize;
use std::collections::BTreeMap;

pub const GUEST_MIN_MEM: usize = 0x0000_0400;
pub const GUEST_MAX_MEM: usize = 0x0C00_0000;

#[derive(Default)]
pub struct Memory {
    pub map: BTreeMap<u32, [u32; 256]>,
    pub hw_watchpoints: Vec<(u32, u32, WatchKind)>,
    pub watch_trigger: Option<(WatchKind, u32)>,
}

impl Memory {
    fn check_watchpoints(&mut self, addr: u32, len: u32, is_write: bool) {
        if self.watch_trigger.is_some() {
            return;
        }
        for entry in self.hw_watchpoints.iter() {
            if is_write && entry.2 == WatchKind::Read {
                continue;
            }
            if !is_write && entry.2 == WatchKind::Write {
                continue;
            }

            let watch_start = entry.0;
            let watch_end = watch_start + entry.1;

            let action_start = addr;
            let action_end = addr + len;

            if action_start < watch_start && action_end >= watch_start {
                self.watch_trigger = Some((entry.2, addr));
                return;
            } else if action_start >= watch_start && action_start < watch_end {
                self.watch_trigger = Some((entry.2, addr));
                return;
            }
        }
    }
}

impl rrs_lib::Memory for Memory {
    fn read_mem(&mut self, addr: u32, size: MemAccessSize) -> Option<u32> {
        if (addr as usize) < GUEST_MIN_MEM || (addr as usize) > GUEST_MAX_MEM {
            return None;
        }

        let page_idx = addr >> 10;
        if !self.map.contains_key(&page_idx) {
            self.map.insert(page_idx, [0u32; 256]);
        }

        let page_offset = (addr & 0x3ff) as usize;

        return match size {
            MemAccessSize::Byte => {
                self.check_watchpoints(addr, 1, false);
                let word = self.map.get(&page_idx).unwrap()[page_offset / 4];

                if page_offset % 4 == 0 {
                    Some(word & 0xff)
                } else if page_offset % 4 == 1 {
                    Some((word >> 8) & 0xff)
                } else if page_offset % 4 == 2 {
                    Some((word >> 16) & 0xff)
                } else {
                    Some((word >> 24) & 0xff)
                }
            }
            MemAccessSize::HalfWord => {
                self.check_watchpoints(addr, 2, false);
                let word = self.map.get(&page_idx).unwrap()[page_offset / 4];

                if page_offset % 4 == 2 {
                    Some((word >> 16) & 0xffff)
                } else {
                    Some(word & 0xffff)
                }
            }
            MemAccessSize::Word => {
                self.check_watchpoints(addr, 4, false);
                Some(self.map.get(&page_idx).unwrap()[page_offset / 4])
            }
        };
    }

    fn write_mem(&mut self, addr: u32, size: MemAccessSize, store_data: u32) -> bool {
        if (addr as usize) < GUEST_MIN_MEM || (addr as usize) > GUEST_MAX_MEM {
            return false;
        }

        let page_idx = addr >> 10;
        if !self.map.contains_key(&page_idx) {
            self.map.insert(page_idx, [0u32; 256]);
        }

        let page_offset = (addr & 0x3ff) as usize;

        match size {
            MemAccessSize::Byte => {
                self.check_watchpoints(addr, 1, true);
                let word = self.map.get(&page_idx).unwrap()[page_offset / 4];

                let new_word = if page_offset % 4 == 0 {
                    (word & 0xffffff00) | (store_data & 0xff)
                } else if page_offset % 4 == 1 {
                    (word & 0xffff00ff) | ((store_data & 0xff) << 8)
                } else if page_offset % 4 == 2 {
                    (word & 0xff00ffff) | ((store_data & 0xff) << 16)
                } else {
                    (word & 0x00ffffff) | ((store_data & 0xff) << 24)
                };

                self.map.get_mut(&page_idx).unwrap()[page_offset / 4] = new_word;
            }
            MemAccessSize::HalfWord => {
                self.check_watchpoints(addr, 2, true);
                let word = self.map.get(&page_idx).unwrap()[page_offset / 4];

                let new_word = if page_offset % 4 == 2 {
                    (word & 0x0000ffff) | ((store_data & 0xffff) << 16)
                } else {
                    (word & 0xffff0000) | (store_data & 0xffff)
                };

                self.map.get_mut(&page_idx).unwrap()[page_offset / 4] = new_word;
            }
            MemAccessSize::Word => {
                self.check_watchpoints(addr, 4, true);
                self.map.get_mut(&page_idx).unwrap()[page_offset / 4] = store_data;
            }
        }

        true
    }
}
