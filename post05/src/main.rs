#![no_std]
#![no_main]
#![feature(global_asm)]

use core::panic::PanicInfo;

// "extern crate alloc" is necessary
extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;

use myos::print;
use myos::println;
global_asm!(include_str!("asm/boot.S"));
global_asm!(include_str!("asm/trap.S"));
global_asm!(include_str!("asm/mem.S"));

#[no_mangle]
extern "C" fn kinit() {
    // Main should initialize all sub-systems and get
    // ready to start scheduling. The last thing this
    // should do is start the timer.
    myos::init_os();
    print!("This is my operating system! ");
    println!("Hello world!");
}

#[no_mangle]
extern "C" fn kmain() {
    // kmain() starts in supervisor mode. So, we should have the trap
    // vector setup and the MMU turned on when we get here.

    // We initialized my_uart in machine mode under kinit for debugging
    // prints, but this just grabs a pointer to it.
    let mut my_uart = myos::uart::Uart::new(0x1000_0000);
    // Create a new scope so that we can test the global allocator and
    // deallocator
    {
        // We have the global allocator, so let's see if that works!
        let k = Box::<u32>::new(100);
        println!("Boxed value = {}", *k);
        // The following comes from the Rust documentation:
        // some bytes, in a vector
        let sparkle_heart = vec![240, 159, 146, 150];
        // We know these bytes are valid, so we'll use `unwrap()`.
        // This will MOVE the vector.
        let sparkle_heart = String::from_utf8(sparkle_heart).unwrap();
        println!("String = {}", sparkle_heart);
        println!("\n\nAllocations of a box, vector, and string");
        myos::kmem::print_table();
    }
    println!("\n\nEverything should now be free:");
    myos::kmem::print_table();

    unsafe {
        // Set the next machine timer to fire.
        let mtimecmp = 0x0200_4000 as *mut u64;
        let mtime = 0x0200_bff8 as *const u64;
        // The frequency given by QEMU is 10_000_000 Hz, so this sets
        // the next interrupt to fire one second from now.
        mtimecmp.write_volatile(mtime.read_volatile() + 10_000_000);

        // Let's cause a page fault and see what happens. This should trap
        // to m_trap under trap.rs
        let v = 0x0 as *mut u64;
        v.write_volatile(0);
    }
    // // If we get here, the Box, vec, and String should all be freed since
    // // they go out of scope. This calls their "Drop" trait.
    // // Now see if we can read stuff:
    // // Usually we can use #[test] modules in Rust, but it would convolute
    // // the task at hand, and it requires us to create the testing harness
    // // since the embedded testing system is part of the "std" library.
    // loop {
    //     if let Some(c) = my_uart.get() {
    //         match c {
    //             8 => {
    //                 // This is a backspace, so we
    //                 // essentially have to write a space and
    //                 // backup again:
    //                 print!("{} {}", 8 as char, 8 as char);
    //             }
    //             10 | 13 => {
    //                 // Newline or carriage-return
    //                 println!();
    //             }
    //             _ => {
    //                 print!("{}", c as char);
    //             }
    //         }
    //     }
    // }
}

/// This function is called on panic.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Panic (Non-test): {}", info);
    // loop {}
    myos::qemu_exit(false)
}

#[allow(dead_code)]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
