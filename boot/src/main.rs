#![no_std]
#![no_main]
use core::{ffi::c_void, panic::PanicInfo};
#[repr(C)]
struct TableHeader { signature:u64, revision:u32, header_size:u32, crc32:u32, reserved:u32 }
#[repr(C)]
struct TextOut { reset:usize, output_string:extern "efiapi" fn(*mut TextOut,*const u16)->usize }
#[repr(C)]
pub struct SystemTable {
    header:TableHeader, firmware_vendor:*const u16, firmware_revision:u32,
    console_in_handle:*mut c_void, con_in:*mut c_void,
    console_out_handle:*mut c_void, con_out:*mut TextOut,
}
#[no_mangle]
pub extern "efiapi" fn efi_main(_image:*mut c_void, table:*mut SystemTable)->usize {
    if table.is_null(){return 2usize | (1usize << (usize::BITS-1));}
    let message:[u16;17]=[67,79,71,78,73,88,95,79,83,32,98,111,111,116,13,10,0];
    unsafe {
        let out=(*table).con_out;
        if out.is_null(){return 2usize | (1usize << (usize::BITS-1));}
        ((*out).output_string)(out,message.as_ptr())
    }
}
#[panic_handler]
fn panic(_: &PanicInfo)->!{loop{core::hint::spin_loop();}}
