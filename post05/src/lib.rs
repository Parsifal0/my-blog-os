#![no_std]
#![feature(alloc_error_handler)]
#![feature(llvm_asm)]


use qemu_exit::QEMUExit;


pub mod cpu;
pub mod kmem;
pub mod page;
pub mod trap;
pub mod uart;

// ///////////////////////////////////
// / CONSTANTS
// ///////////////////////////////////
// const STR_Y: &str = "\x1b[38;2;79;221;13m✓\x1b[m";
// const STR_N: &str = "\x1b[38;2;221;41;13m✘\x1b[m";

// The following symbols come from asm/mem.S. We can use
// the symbols directly, but the address of the symbols
// themselves are their values, which can cause issues.
// Instead, I created doubleword values in mem.S in the .rodata and .data
// sections.
extern "C" {
	static TEXT_START: usize;
	static TEXT_END: usize;
	static DATA_START: usize;
	static DATA_END: usize;
	static RODATA_START: usize;
	static RODATA_END: usize;
	static BSS_START: usize;
	static BSS_END: usize;
	static KERNEL_STACK_START: usize;
	static KERNEL_STACK_END: usize;
	static HEAP_START: usize;
	static HEAP_SIZE: usize;
}

/// Identity map range
/// Takes a contiguous allocation of memory and maps it using PAGE_SIZE
/// This assumes that start <= end
pub fn id_map_range(root: &mut page::Table, start: usize, end: usize, bits: usize) {
    let mut memaddr = start & !(page::PAGE_SIZE - 1);
    let num_kb_pages = (page::align_val(end, 12) - memaddr) / page::PAGE_SIZE;

    // I named this num_kb_pages for future expansion when
    // I decide to allow for GiB (2^30) and 2MiB (2^21) page
    // sizes. However, the overlapping memory regions are causing
    // nightmares.
    for _ in 0..num_kb_pages {
        page::map(root, memaddr, memaddr, bits, 0);
        memaddr += 1 << 12;
    }
}

pub fn init_os() {
    uart::Uart::new(0x1000_0000).init();
    page::init();
    kmem::init();
    // Map heap allocations
    let root_ptr = kmem::get_page_table();
    let root_u = root_ptr as usize;
    let mut root = unsafe { root_ptr.as_mut().unwrap() };
    let kheap_head = kmem::get_head() as usize;
    let total_pages = kmem::get_num_allocations();
    println!();
    println!();
    unsafe {
        println!("TEXT:   0x{:x} -> 0x{:x}", TEXT_START, TEXT_END);
        println!("RODATA: 0x{:x} -> 0x{:x}", RODATA_START, RODATA_END);
        println!("DATA:   0x{:x} -> 0x{:x}", DATA_START, DATA_END);
        println!("BSS:    0x{:x} -> 0x{:x}", BSS_START, BSS_END);
        println!(
            "STACK:  0x{:x} -> 0x{:x}",
            KERNEL_STACK_START, KERNEL_STACK_END
        );
        println!(
            "HEAP:   0x{:x} -> 0x{:x}",
            kheap_head,
            kheap_head + total_pages * page::PAGE_SIZE
        );
    }
    id_map_range(
        &mut root,
        kheap_head,
        kheap_head + total_pages * page::PAGE_SIZE,
        page::EntryBits::ReadWrite.val(),
    );
    // Using statics is inherently unsafe.
    unsafe {
        // Map heap descriptors
        let num_pages = HEAP_SIZE / page::PAGE_SIZE;
        id_map_range(
            &mut root,
            HEAP_START,
            HEAP_START + num_pages,
            page::EntryBits::ReadWrite.val(),
        );
        // Map executable section
        id_map_range(
            &mut root,
            TEXT_START,
            TEXT_END,
            page::EntryBits::ReadExecute.val(),
        );
        // Map rodata section
        // We put the ROdata section into the text section, so they can
        // potentially overlap however, we only care that it's read
        // only.
        id_map_range(
            &mut root,
            RODATA_START,
            RODATA_END,
            page::EntryBits::ReadExecute.val(),
        );
        // Map data section
        id_map_range(
            &mut root,
            DATA_START,
            DATA_END,
            page::EntryBits::ReadWrite.val(),
        );
        // Map bss section
        id_map_range(
            &mut root,
            BSS_START,
            BSS_END,
            page::EntryBits::ReadWrite.val(),
        );
        // Map kernel stack
        id_map_range(
            &mut root,
            KERNEL_STACK_START,
            KERNEL_STACK_END,
            page::EntryBits::ReadWrite.val(),
        );
    }

    // UART
    id_map_range(
        &mut root,
        0x1000_0000,
        0x1000_0100,
        page::EntryBits::ReadWrite.val(),
    );

    // CLINT
    //  -> MSIP
    id_map_range(
        &mut root,
        0x0200_0000,
        0x0200_ffff,
        page::EntryBits::ReadWrite.val(),
    );
    // PLIC
    id_map_range(
        &mut root,
        0x0c00_0000,
        0x0c00_2000,
        page::EntryBits::ReadWrite.val(),
    );
    id_map_range(
        &mut root,
        0x0c20_0000,
        0x0c20_8000,
        page::EntryBits::ReadWrite.val(),
    );
    // When we return from here, we'll go back to boot.S and switch into
    // supervisor mode We will return the SATP register to be written when
    // we return. root_u is the root page table's address. When stored into
    // the SATP register, this is divided by 4 KiB (right shift by 12 bits).
    // We enable the MMU by setting mode 8. Bits 63, 62, 61, 60 determine
    // the mode.
    // 0 = Bare (no translation)
    // 8 = Sv39
    // 9 = Sv48
    // build_satp has these parameters: mode, asid, page table address.
    let satp_value = cpu::build_satp(cpu::SatpMode::Sv39, 0, root_u);
    unsafe {
        // We have to store the kernel's table. The tables will be moved
        // back and forth between the kernel's table and user
        // applicatons' tables. Note that we're writing the physical address
        // of the trap frame.
        cpu::mscratch_write((&mut cpu::KERNEL_TRAP_FRAME[0] as *mut cpu::TrapFrame) as usize);
        cpu::sscratch_write(cpu::mscratch_read());
        cpu::KERNEL_TRAP_FRAME[0].satp = satp_value;
        // Move the stack pointer to the very bottom. The stack is
        // actually in a non-mapped page. The stack is decrement-before
        // push and increment after pop. Therefore, the stack will be
        // allocated (decremented) before it is stored.
        cpu::KERNEL_TRAP_FRAME[0].trap_stack = page::zalloc(1).add(page::PAGE_SIZE);
        id_map_range(
            &mut root,
            cpu::KERNEL_TRAP_FRAME[0].trap_stack.sub(page::PAGE_SIZE) as usize,
            cpu::KERNEL_TRAP_FRAME[0].trap_stack as usize,
            page::EntryBits::ReadWrite.val(),
        );
        // The trap frame itself is stored in the mscratch register.
        id_map_range(
            &mut root,
            cpu::mscratch_read(),
            cpu::mscratch_read() + core::mem::size_of::<cpu::TrapFrame>(),
            page::EntryBits::ReadWrite.val(),
        );
        page::print_page_allocations();
        let p = cpu::KERNEL_TRAP_FRAME[0].trap_stack as usize - 1;
        let m = page::virt_to_phys(&root, p).unwrap_or(0);
        println!("Walk 0x{:x} = 0x{:x}", p, m);
    }
    // The following shows how we're going to walk to translate a virtual
    // address into a physical address. We will use this whenever a user
    // space application requires services. Since the user space application
    // only knows virtual addresses, we have to translate silently behind
    // the scenes.
    println!("Setting 0x{:x}", satp_value);
    println!("Scratch reg = 0x{:x}", cpu::mscratch_read());
    cpu::satp_write(satp_value);
    cpu::satp_fence_asid(0);
}

pub fn qemu_exit(success: bool) -> ! {
    let qemu_exit_handle = qemu_exit::RISCV64::new(0x100000);
    if success == true {
        qemu_exit_handle.exit_success();
    } else {
        qemu_exit_handle.exit_failure();
    }
}

pub fn test_mem() {
    page::zalloc(100);
    page::zalloc(100);
    page::print_page_allocations();
}
