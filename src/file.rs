use alloc::{ffi::CString, vec::Vec};
use core::{convert::TryInto, mem::MaybeUninit};

use crate::bindings::*;

pub struct Ext4File(ext4_file);

impl Drop for Ext4File {
    fn drop(&mut self) {
        unsafe {
            ext4_fclose(&mut self.0);
        }
    }
}

impl Ext4File {
    pub fn open(path: &str, flags: i32) -> Result<Self, i32> {
        let c_path = CString::new(path).expect("CString::new failed");
        let mut file = MaybeUninit::uninit();
        let r = unsafe { ext4_fopen2(file.as_mut_ptr(), c_path.as_ptr(), flags) };
        match r {
            0 => unsafe { Ok(Self(file.assume_init())) },
            e => {
                error!("ext4_fopen: {}, rc = {}", path, r);
                Err(e)
            }
        }
    }

    pub fn seek(&mut self, offset: i64, seek_type: u32) -> Result<(), i32> {
        let mut offset = offset;
        let size = self.size() as i64;

        if offset > size {
            warn!("Seek beyond the end of the file");
            offset = size;
        }
        let r = unsafe { ext4_fseek(&mut self.0, offset, seek_type) };
        match r {
            0 => Ok(()),
            _ => {
                error!("ext4_fseek error: rc = {}", r);
                Err(r)
            }
        }
    }

    pub fn tell(&mut self) -> u64 {
        let r = unsafe { ext4_ftell(&mut self.0) };
        r
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, i32> {
        let mut r_cnt = 0;
        let r = unsafe { ext4_fread(&mut self.0, buf.as_mut_ptr() as _, buf.len(), &mut r_cnt) };

        match r {
            0 => Ok(r_cnt),
            e => {
                error!("ext4_fread: rc = {}", r);
                Err(e)
            }
        }
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize, i32> {
        let mut w_cnt = 0;
        let r = unsafe { ext4_fwrite(&mut self.0, buf.as_ptr() as _, buf.len(), &mut w_cnt) };

        match r {
            0 => Ok(w_cnt),
            e => {
                error!("ext4_fwrite: rc = {}", r);
                Err(e)
            }
        }
    }

    pub fn truncate(&mut self, size: u64) -> Result<(), i32> {
        let r = unsafe { ext4_ftruncate(&mut self.0, size) };
        match r {
            0 => Ok(()),
            e => {
                error!("ext4_ftruncate: rc = {}", r);
                Err(e)
            }
        }
    }

    pub fn size(&mut self) -> u64 {
        unsafe { ext4_fsize(&mut self.0) }
    }

    pub fn file_get_blk_idx(&mut self) -> Result<u64, i32> {
        let block_idx;
        unsafe {
            let mut inode_ref = ext4_inode_ref {
                block: ext4_block {
                    lb_id: 0,
                    buf: core::ptr::null_mut(),
                    data: core::ptr::null_mut(),
                },
                inode: core::ptr::null_mut(),
                fs: core::ptr::null_mut(),
                index: 0,
                dirty: false,
            };
            let r = ext4_fs_get_inode_ref(&mut (*self.0.mp).fs, self.0.inode, &mut inode_ref);
            if r != EOK as i32 {
                error!("ext4_fs_get_inode_ref: rc = {}", r);
                return Err(r);
            }
            let sb = (*self.0.mp).fs.sb;
            let block_size = 1024 << sb.log_block_size.to_le();
            let iblock_idx: ext4_lblk_t = ((self.0.fpos) / block_size).try_into().unwrap();
            let mut fblock = 0;
            let r = ext4_fs_get_inode_dblk_idx(&mut inode_ref, iblock_idx, &mut fblock, true);
            if r != EOK as i32 {
                error!("ext4_fs_get_inode_dblk_idx: rc = {}", r);
                return Err(r);
            }
            ext4_fs_put_inode_ref(&mut inode_ref);

            let unalg = (self.0.fpos) % block_size;
            let bdev = *(*self.0.mp).fs.bdev;
            let off = fblock * block_size + unalg;
            block_idx = (off + bdev.part_offset) / ((*(bdev.bdif)).ph_bsize as u64);
        }
        Ok(block_idx)
    }
}

// pub enum OpenFlags {
// O_RDONLY = 0,
// O_WRONLY = 0x1,
// O_RDWR = 0x2,
// O_CREAT = 0x40,
// O_TRUNC = 0x200,
// O_APPEND = 0x400,
// }

#[derive(PartialEq, Clone, Debug)]
pub enum InodeTypes {
    // Inode type, Directory entry types.
    EXT4_DE_UNKNOWN = 0,
    EXT4_DE_REG_FILE = 1,
    EXT4_DE_DIR = 2,
    EXT4_DE_CHRDEV = 3,
    EXT4_DE_BLKDEV = 4,
    EXT4_DE_FIFO = 5,
    EXT4_DE_SOCK = 6,
    EXT4_DE_SYMLINK = 7,

    // Inode mode
    EXT4_INODE_MODE_FIFO = 0x1000,
    EXT4_INODE_MODE_CHARDEV = 0x2000,
    EXT4_INODE_MODE_DIRECTORY = 0x4000,
    EXT4_INODE_MODE_BLOCKDEV = 0x6000,
    EXT4_INODE_MODE_FILE = 0x8000,
    EXT4_INODE_MODE_SOFTLINK = 0xA000,
    EXT4_INODE_MODE_SOCKET = 0xC000,
    EXT4_INODE_MODE_TYPE_MASK = 0xF000,
}

impl From<usize> for InodeTypes {
    fn from(num: usize) -> InodeTypes {
        match num {
            0 => InodeTypes::EXT4_DE_UNKNOWN,
            1 => InodeTypes::EXT4_DE_REG_FILE,
            2 => InodeTypes::EXT4_DE_DIR,
            3 => InodeTypes::EXT4_DE_CHRDEV,
            4 => InodeTypes::EXT4_DE_BLKDEV,
            5 => InodeTypes::EXT4_DE_FIFO,
            6 => InodeTypes::EXT4_DE_SOCK,
            7 => InodeTypes::EXT4_DE_SYMLINK,
            0x1000 => InodeTypes::EXT4_INODE_MODE_FIFO,
            0x2000 => InodeTypes::EXT4_INODE_MODE_CHARDEV,
            0x4000 => InodeTypes::EXT4_INODE_MODE_DIRECTORY,
            0x6000 => InodeTypes::EXT4_INODE_MODE_BLOCKDEV,
            0x8000 => InodeTypes::EXT4_INODE_MODE_FILE,
            0xA000 => InodeTypes::EXT4_INODE_MODE_SOFTLINK,
            0xC000 => InodeTypes::EXT4_INODE_MODE_SOCKET,
            0xF000 => InodeTypes::EXT4_INODE_MODE_TYPE_MASK,
            _ => {
                warn!("Unknown ext4 inode type: {}", num);
                InodeTypes::EXT4_DE_UNKNOWN
            }
        }
    }
}
