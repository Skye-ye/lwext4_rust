use alloc::{ffi::CString, vec::Vec};
use core::mem::MaybeUninit;

use crate::{
    bindings::{ext4_dir, ext4_dir_close, ext4_dir_entry_next, ext4_dir_mk, ext4_dir_open},
    InodeTypes,
};

pub struct Ext4Dir(ext4_dir);

impl Drop for Ext4Dir {
    fn drop(&mut self) {
        unsafe {
            ext4_dir_close(&mut self.0);
        }
    }
}

pub struct Ext4DirEntry<'a> {
    pub inode: u32,
    pub name: &'a str,
    pub type_: u8,
}

impl Ext4Dir {
    pub fn open(path: &str) -> Result<Self, i32> {
        let c_path = CString::new(path).expect("CString::new failed");
        let mut dir = MaybeUninit::uninit();
        let r = unsafe { ext4_dir_open(dir.as_mut_ptr(), c_path.as_ptr()) };
        match r {
            0 => unsafe { Ok(Self(dir.assume_init())) },
            e => {
                error!("ext4_dir_open: {}, rc = {}", path, r);
                Err(e)
            }
        }
    }

    pub fn create(path: &str) -> Result<Self, i32> {
        let c_path = CString::new(path).expect("CString::new failed");
        let r = unsafe { ext4_dir_mk(c_path.as_ptr()) };
        match r {
            0 => {}
            e => {
                error!("ext4_dir_mk: {}, rc = {}", path, r);
                return Err(e);
            }
        }
        let mut dir = MaybeUninit::uninit();
        let r = unsafe { ext4_dir_open(dir.as_mut_ptr(), c_path.as_ptr()) };
        match r {
            0 => unsafe { Ok(Self(dir.assume_init())) },
            e => {
                error!("ext4_dir_open: {}, rc = {}", path, r);
                Err(e)
            }
        }
    }
}

impl Ext4Dir {
    pub fn next(&mut self) -> Option<Ext4DirEntry> {
        unsafe {
            if ext4_dir_entry_next(&mut self.0).is_null() {
                return None;
            }
        };
        let name_buf = &self.0.de.name[..self.0.de.name_length as usize];
        Some(Ext4DirEntry {
            inode: self.0.de.inode,
            name: core::str::from_utf8(&name_buf).unwrap(),
            type_: self.0.de.inode_type,
        })
    }

    pub fn lwext4_dir_entries(&self, path: &str) -> Result<(Vec<Vec<u8>>, Vec<InodeTypes>), i32> {
        let c_path = CString::new(path).unwrap();
        let mut d: ext4_dir = unsafe { core::mem::zeroed() };

        let mut name: Vec<Vec<u8>> = Vec::new();
        let mut inode_type: Vec<InodeTypes> = Vec::new();

        // info!("ls {}", str::from_utf8(path).unwrap());
        unsafe {
            ext4_dir_open(&mut d, c_path.as_ptr());

            let mut de = ext4_dir_entry_next(&mut d);
            while !de.is_null() {
                let dentry = &(*de);
                let len = dentry.name_length as usize;

                let mut sss: [u8; 255] = [0; 255];
                sss[..len].copy_from_slice(&dentry.name[..len]);
                sss[len] = 0;

                debug!(
                    "  {} {}",
                    dentry.inode_type,
                    core::str::from_utf8(&sss).unwrap()
                );
                name.push(sss[..(len + 1)].to_vec());
                inode_type.push((dentry.inode_type as usize).into());

                de = ext4_dir_entry_next(&mut d);
            }
            ext4_dir_close(&mut d);
        }

        Ok((name, inode_type))
    }
}
