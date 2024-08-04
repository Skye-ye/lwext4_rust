//! Adapted from `MinotaurOS`, with some modification.

#![no_std]
#![feature(linkage)]
#![feature(c_variadic, c_size_t)]
#![feature(associated_type_defaults)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

extern crate alloc;

#[macro_use]
extern crate log;

mod ulibc;

pub mod bindings;
pub mod blockdev;
pub mod dir;
pub mod file;

use alloc::ffi::CString;

use bindings::{
    ext4_dir_mv, ext4_dir_rm, ext4_fremove, ext4_frename, ext4_inode_exist, ext4_mode_get, EOK,
};
pub use blockdev::*;
pub use dir::Ext4Dir;
pub use file::{Ext4File, InodeTypes};

use crate::bindings::ext4_readlink;

/// Check if inode exists.
///
/// Inode types:
/// EXT4_DIRENTRY_UNKNOWN
/// EXT4_DE_REG_FILE
/// EXT4_DE_DIR
/// EXT4_DE_CHRDEV
/// EXT4_DE_BLKDEV
/// EXT4_DE_FIFO
/// EXT4_DE_SOCK
/// EXT4_DE_SYMLINK
pub fn lwext4_check_inode_exist(path: &str, types: InodeTypes) -> bool {
    let c_path = CString::new(path).expect("CString::new failed");
    let r = unsafe { ext4_inode_exist(c_path.as_ptr(), types as i32) }; // eg: types: EXT4_DE_REG_FILE
    r == EOK as i32
}

/// Rename directory
pub fn lwext4_mvdir(path: &str, new_path: &str) -> Result<(), i32> {
    let c_path = CString::new(path).expect("CString::new failed");
    let c_new_path = CString::new(new_path).expect("CString::new failed");
    let r = unsafe { ext4_dir_mv(c_path.as_ptr(), c_new_path.as_ptr()) };
    match r {
        0 => Ok(()),
        _ => {
            error!("ext4_dir_mv error: rc = {r}, path = {path}");
            Err(r)
        }
    }
}

/// Rename file
pub fn lwext4_mvfile(path: &str, new_path: &str) -> Result<(), i32> {
    let c_path = CString::new(path).expect("CString::new failed");
    let c_new_path = CString::new(new_path).expect("CString::new failed");
    let r = unsafe { ext4_frename(c_path.as_ptr(), c_new_path.as_ptr()) };
    match r {
        0 => Ok(()),
        _ => {
            error!("ext4_frename error: rc = {r}, path = {path}");
            Err(r)
        }
    }
}

/// Recursive directory remove
pub fn lwext4_rmdir(path: &str) -> Result<(), i32> {
    let c_path = CString::new(path).expect("CString::new failed");
    let r = unsafe { ext4_dir_rm(c_path.as_ptr()) };
    match r {
        0 => Ok(()),
        e => {
            error!("ext4_dir_rm: rc = {r}, path = {path}");
            Err(e)
        }
    }
}

/// Remove file by path.
pub fn lwext4_rmfile(path: &str) -> Result<(), i32> {
    let c_path = CString::new(path).expect("CString::new failed");
    let r = unsafe { ext4_fremove(c_path.as_ptr()) };
    match r {
        0 => Ok(()),
        _ => {
            error!("ext4_fremove error: rc = {r}, path = {path}");
            Err(r)
        }
    }
}

pub fn lwext4_readlink(path: &str, buf: &mut [u8]) -> Result<usize, i32> {
    let c_path = CString::new(path).expect("CString::new failed");
    let mut r_cnt = 0;
    let r = unsafe {
        ext4_readlink(
            c_path.as_ptr(),
            buf.as_mut_ptr() as _,
            buf.len(),
            &mut r_cnt,
        )
    };

    match r {
        0 => Ok(r_cnt),
        _ => {
            error!("ext4_readlink: rc = {r}, path = {path}");
            Err(r)
        }
    }
}
