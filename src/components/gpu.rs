use std::fmt::{Debug, Display};

// use pciutils_sys::{pci_alloc, pci_cleanup, pci_fill_info, pci_init, pci_scan_bus};

// Checks if given device_class looks like a GPU
#[inline(always)]
fn is_gpu(class: u16) -> bool {
    const HEX_DIGIT: u32 = 0xf_u32.count_ones();

    let id = class >> (2 * HEX_DIGIT);
    id == 0x03
}

pub struct PrettyDevice<'dev>(pub &'dev pci_ids::Device);
impl Display for PrettyDevice<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[inline(always)]
        fn get_pretty_name(long: &str) -> &str {
            let (Some(start), Some(end)) = (long.find('['), long.find(']')) else {
                return long;
            };
            &long[start + 1..end]
        }

        let card = self.0;

        let vendor = card.vendor().name();
        let name = card.name();

        let name = {
            if let (Some(start), Some(end)) = (name.find('['), name.find(']')) {
                &name[start + 1..end]
            } else {
                name
            }
        };

        // Shorten GPU text
        let (name, suffix) = name
            .find(" Laptop GPU")
            .map(|end| (&name[..end], "(Laptop)"))
            .or_else(|| name.find(" Integrated").map(|end| (&name[..end], " iGPU")))
            .unwrap_or((name, ""));

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

        write!(f, "{vendor} {name}{suffix}")
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
