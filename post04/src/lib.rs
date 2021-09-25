#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use qemu_exit::QEMUExit;
use core::panic::PanicInfo;

pub mod uart;

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        print!("{}...\t", core::any::type_name::<T>());
        //- Run the function
        self();
        println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    //- In qemu 5.0, this address can be found in hw/riscv/virt.c (VIRT_TEST)
	let qemu_exit_handle = qemu_exit::RISCV64::new(0x100000);
	qemu_exit_handle.exit_success();
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    println!("[failed]\n");
    println!("Error: {}\n", info);
    let qemu_exit_handle = qemu_exit::RISCV64::new(0x100000);
	qemu_exit_handle.exit_failure();
}

/// Entry point for `cargo xtest`
#[cfg(test)]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    test_main();
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

pub fn init_os() {
    uart::Uart::new(0x1000_0000).init();
}



// #[cfg(test)]
// fn test_runner(tests: &[&dyn Fn()]) {
//     println!("Running {} tests", tests.len());
//     for test in tests {
//         test();
//     }

// }

// #[test_case]
// fn test_println_simple() {
//     println!("test_println_simple...");
// }