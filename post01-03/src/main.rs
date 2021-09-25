#![no_std]
#![no_main]
#![feature(global_asm)]

use core::panic::PanicInfo;
pub mod uart;
global_asm!(include_str!("asm/boot.S"));
global_asm!(include_str!("asm/trap.S"));

#[macro_export]
macro_rules! print
{
	($($args:tt)*) => ({
			use core::fmt::Write;
			let _ = write!(crate::uart::Uart::new(0x1000_0000), $($args)*);
			});
}
#[macro_export]
macro_rules! println
{
	() => ({
		   print!("\r\n")
		   });
	($fmt:expr) => ({
			print!(concat!($fmt, "\r\n"))
			});
	($fmt:expr, $($args:tt)*) => ({
			print!(concat!($fmt, "\r\n"), $($args)*)
			});
}

#[no_mangle]
extern "C" fn kinit() -> ! {
    // Main should initialize all sub-systems and get
    // ready to start scheduling. The last thing this
    // should do is start the timer.
    uart::Uart::new(0x1000_0000).init();
    println!("This is my operating system!");
    println!("Hello world!");
    loop{}
}

#[no_mangle]
extern "C" fn kinit_hart(_hartid: usize) {
	// We aren't going to do anything here until we get SMP going.
	// All non-0 harts initialize here.
}

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    println!("no information available.");
    loop {}
}
