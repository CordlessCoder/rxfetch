use std::fmt::{Debug, Display};

// use pciutils_sys::{pci_alloc, pci_cleanup, pci_fill_info, pci_init, pci_scan_bus};

// Checks if given device_class looks like a GPU
#[inline(always)]
fn is_gpu(class: u16) -> bool {
    const HEX_DIGIT: u32 = 0xf_u32.count_ones();

    let id = class >> (HEX_DIGIT * 2);
    id == 0x03
}

pub struct PrettyDevice<'dev>(pub &'dev pci_ids::Device);
impl Display for PrettyDevice<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[inline(always)]
        fn get_pretty_name(long: &str) -> &str {
            let Some(start) = long.find('[') else {
                return long;
            };
            let name = &long[start + 1..];
            let Some(end) = name.find(']') else {
                return long;
            };
            &name[..end]
        }

        let card = self.0;

        let vendor = card.vendor().name();
        let name = card.name();

        // Get GPU pretty name
        let name = get_pretty_name(name);

        // Shorten GPU text
        let mut name = name;
        let mut laptop_prefix = "";
        if let Some(end) = name.find(" Laptop GPU") {
            laptop_prefix = "(Laptop)";
            name = &name[..end]
        };
        let laptop_suffix = laptop_prefix;
        let name = name;

        // Shorten vendor
        let vendor = vendor
            .find(' ')
            .map(|end| &vendor[..end])
            .and_then(|firstword| {
                firstword
                    .bytes()
                    .next()
                    .is_some_and(|b| b.is_ascii_uppercase())
                    .then_some(firstword)
            })
            .unwrap_or(vendor.trim());

        // Remove whitespace
        let name = name.trim();

        write!(f, "{vendor} {name}{laptop_suffix}")
    }
}
impl Debug for PrettyDevice<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let card = self.0;

        let vendor = card.vendor().name();
        let name = card.name();

        // Remove whitespace
        let name = name.trim();

        write!(f, "{vendor} {name}")
    }
}

// pub struct GPUIter(PCIDevIter);
// impl GPUIter {
//     #[inline]
//     pub fn new() -> Self {
//         Self(PCIDevIter::new())
//     }
// }
// impl Iterator for GPUIter {
//     type Item = &'static pci_ids::Device;
//
//     #[inline]
//     fn next(&mut self) -> Option<Self::Item> {
//         let devices = &mut self.0;
//         loop {
//             let dev = devices.next()?;
//             let class = unsafe {
//                 pci_fill_info(dev, pciutils_sys::PCI_FILL_CLASS);
//                 (*dev).device_class
//             };
//             if !is_gpu(class) {
//                 continue;
//             };
//             // Found GPU
//             unsafe {
//                 pci_fill_info(dev, pciutils_sys::PCI_FILL_IDENT);
//             }
//             let Some(dev) =
//                 pci_ids::Device::from_vid_pid(unsafe { *dev }.vendor_id, unsafe { *dev }.device_id)
//             else {
//                 continue;
//             };
//             return Some(dev);
//         }
//         // Obsolete code for checking device class with string comparisons
//         // (that paleofetch uses)
//         //
//         // let class = pci_lookup_name(
//         //     pci_access,
//         //     buf.as_mut_ptr(),
//         //     buf.len() as i32,
//         //     pciutils_sys::pci_lookup_mode_PCI_LOOKUP_CLASS as i32,
//         //     (*dev).device_class as c_uint,
//         // );
//         // let class = CStr::from_ptr(class);
//         // let class = std::str::from_utf8_unchecked(class.to_bytes());
//         // if !matches!(class, "VGA compatible controller" | "3D controller") {
//         //     continue;
//         // }
//     }
// }
//
// pub struct PCIDevIter {
//     pacc: *mut pciutils_sys::pci_access,
//     // Walk using a separate pointer to not affect the device pointer of the pci_access struct,
//     // doing that would leak memory
//     dev: *mut pciutils_sys::pci_dev,
// }
//
// impl PCIDevIter {
//     #[inline]
//     pub fn new() -> Self {
//         let pacc = unsafe {
//             let ptr = pci_alloc();
//             pci_init(ptr);
//             pci_scan_bus(ptr);
//             ptr
//         };
//         let dev = unsafe { *pacc }.devices;
//         Self { pacc, dev }
//     }
// }
//
// impl Iterator for PCIDevIter {
//     type Item = *mut pciutils_sys::pci_dev;
//
//     #[inline]
//     fn next(&mut self) -> Option<Self::Item> {
//         let dev = self.dev;
//         if dev.is_null() {
//             return None;
//         };
//         unsafe {
//             self.dev = (*dev).next;
//         }
//         Some(dev)
//     }
// }
//
// impl Drop for PCIDevIter {
//     fn drop(&mut self) {
//         unsafe { pci_cleanup(self.pacc) }
//     }
// }
