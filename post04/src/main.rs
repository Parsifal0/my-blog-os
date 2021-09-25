#![no_std]
#![no_main]
#![feature(global_asm)]
#![feature(custom_test_frameworks)]
#![test_runner(myos::test_runner)]
#![reexport_test_harness_main = "test_main"]

use myos::init_os;
use myos::println;
use myos::print;
use core::panic::PanicInfo;
global_asm!(include_str!("asm/boot.S"));
global_asm!(include_str!("asm/trap.S"));



#[no_mangle]
extern "C" fn kinit() -> ! {
    // Main should initialize all sub-systems and get
    // ready to start scheduling. The last thing this
    // should do is start the timer.
    init_os();
    print!("This is my operating system! ");
    println!("Hello world!");

	#[cfg(test)]
    test_main();

    loop{}
}



/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Panic (Non-test): {}", info);
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    myos::test_panic_handler(info)
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}

#[no_mangle]
extern "C" fn kinit_hart(_hartid: usize) {
	// We aren't going to do anything here until we get SMP going.
	// All non-0 harts initialize here.
}