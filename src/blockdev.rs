use crate::bindings::*;
use crate::types::ForeignOwnable;
use alloc::boxed::Box;
use alloc::ffi::CString;
use alloc::vec::Vec;
use core::ffi::{c_char, c_void};
use core::ptr::null_mut;
use core::slice::{from_raw_parts, from_raw_parts_mut};

/// Device block size.
const EXT4_DEV_BSIZE: u32 = 512;

pub trait KernelDevOp {
    type DevType: ForeignOwnable + Sized + Send + Sync = ();

    fn read(dev: &mut Self::DevType, buf: &mut [u8]) -> Result<usize, i32> where Self: Sized;
    fn write(dev: &mut Self::DevType, buf: &[u8]) -> Result<usize, i32> where Self: Sized;
    //fn write(dev: <Self::DevType as ForeignOwnable>::Borrowed<'_>, buf: &[u8]) -> Result<usize, i32>;
    fn seek(dev: &mut Self::DevType, off: i64, whence: i32) -> Result<i64, i32> where Self: Sized;
    fn flush(dev: &mut Self::DevType) -> Result<usize, i32> where Self: Sized;
}

pub struct Ext4BlockWrapper<K: KernelDevOp> {
    value: Box<ext4_blockdev>,
    block_dev: K::DevType,
    name: [u8; 16],
    mount_point: [u8; 32],
}

impl<K: KernelDevOp> Ext4BlockWrapper<K> {
    pub fn new(mut block_dev: K::DevType) -> Result<Self, i32> {
        //let devt = Box::into_raw(Box::new(block_dev)) as *mut c_void;
        //let devt_user = devt.as_mut() as *mut _ as *mut c_void;
        let devt_user = &mut block_dev as *mut _ as *mut c_void;

        // Block size buffer
        let mut bbuf: Vec<u8> = Vec::with_capacity(EXT4_DEV_BSIZE as usize);

        let ext4bdif: ext4_blockdev_iface = ext4_blockdev_iface {
            open: Some(Self::dev_open),
            bread: Some(Self::dev_bread),
            bwrite: Some(Self::dev_bwrite),
            close: Some(Self::dev_close),
            lock: None,
            unlock: None,
            ph_bsize: EXT4_DEV_BSIZE,
            ph_bcnt: 0,
            ph_bbuf: bbuf.as_mut_ptr(),
            ph_refctr: 0,
            bread_ctr: 0,
            bwrite_ctr: 0,
            p_user: devt_user,
        };

        let ext4dev = ext4_blockdev {
            bdif: Box::into_raw(Box::new(ext4bdif)),
            part_offset: 0,
            part_size: 0 * EXT4_DEV_BSIZE as u64,
            bc: null_mut(),
            lg_bsize: 0,
            lg_bcnt: 0,
            cache_write_back: 0,
            fs: null_mut(),
            journal: null_mut(),
        };

        let c_name = CString::new("ext4_fs").expect("CString::new ext4_fs failed");
        let c_name = c_name.as_bytes_with_nul(); // + '\0'
        let c_mountpoint = CString::new("/mp/").unwrap();
        let c_mountpoint = c_mountpoint.as_bytes_with_nul();

        let mut name: [u8; 16] = [0; 16];
        let mut mount_point: [u8; 32] = [0; 32];
        name[..c_name.len()].copy_from_slice(c_name);
        mount_point[..c_mountpoint.len()].copy_from_slice(c_mountpoint);

        let mut ext4bd = Self {
            value: Box::new(ext4dev),
            block_dev,
            name,
            mount_point,
        };

        info!("New an Ext4 Block Device");

        // ext4_blockdev into static instance
        // lwext4_mount
        // let c_mountpoint = c_mountpoint as *const _ as *const c_char;
        unsafe {
            ext4bd
                .lwext4_mount()
                .expect("Failed to mount the ext4 file system");
        }

        ext4bd.print_lwext4_mp_stats();
        ext4bd.print_lwext4_block_stats();

        Ok(ext4bd)
    }
    pub unsafe extern "C" fn dev_open(bdev: *mut ext4_blockdev) -> ::core::ffi::c_int {
        let mut devt = unsafe { K::DevType::from_foreign((*(*bdev).bdif).p_user) };

        // DevType: Disk
        if (*(*bdev).bdif).p_user as usize == 0 {
            return EIO as _;
        }

        // buffering at Disk
        // setbuf(dev_file, buffer);

        let seek_off = K::seek(&mut devt, 0, SEEK_END as i32);
        let cur = match seek_off {
            Ok(v) => v,
            Err(e) => return EFAULT as _,
        };

        (*bdev).part_offset = 0;
        (*bdev).part_size = cur as u64; //ftello()
        (*(*bdev).bdif).ph_bcnt = (*bdev).part_size / (*(*bdev).bdif).ph_bsize as u64;
        EOK as _
    }
    pub unsafe extern "C" fn dev_bread(
        bdev: *mut ext4_blockdev,
        buf: *mut ::core::ffi::c_void,
        blk_id: u64,
        blk_cnt: u32,
    ) -> ::core::ffi::c_int {
        let mut devt = unsafe { K::DevType::from_foreign((*(*bdev).bdif).p_user) };

        let seek_off = K::seek(
            &mut devt,
            (blk_id * ((*(*bdev).bdif).ph_bsize as u64)) as i64,
            SEEK_SET as i32,
        );
        match seek_off {
            Ok(v) => v,
            Err(e) => return EIO as _,
        };

        if blk_cnt == 0 {
            return EOK as _;
        }

        let buf_len = ((*(*bdev).bdif).ph_bsize * blk_cnt * 1) as usize;
        let buffer = unsafe { from_raw_parts_mut(buf as *mut u8, buf_len) };

        let read_cnt = K::read(&mut devt, buffer);
        match read_cnt {
            Ok(v) => v,
            Err(e) => return EIO as _,
        };

        EOK as _
    }
    pub unsafe extern "C" fn dev_bwrite(
        bdev: *mut ext4_blockdev,
        buf: *const ::core::ffi::c_void,
        blk_id: u64,
        blk_cnt: u32,
    ) -> ::core::ffi::c_int {
        //let devt = unsafe { &mut *((*(*bdev).bdif).p_user as *mut K::DevType) };
        //let devt = unsafe { K::DevType::borrow((*(*bdev).bdif).p_user) };
        //let mut devt = unsafe { K::DevType::borrow_mut((*(*bdev).bdif).p_user) };
        let mut devt = unsafe { K::DevType::from_foreign((*(*bdev).bdif).p_user) };

        let seek_off = K::seek(
            &mut devt,
            (blk_id * ((*(*bdev).bdif).ph_bsize as u64)) as i64,
            SEEK_SET as i32,
        );
        match seek_off {
            Ok(v) => v,
            Err(e) => return EIO as _,
        };

        if blk_cnt == 0 {
            return EOK as _;
        }

        let buf_len = ((*(*bdev).bdif).ph_bsize * blk_cnt * 1) as usize;
        let buffer = unsafe { from_raw_parts(buf as *const u8, buf_len) };
        let write_cnt = K::write(&mut devt, buffer);
        match write_cnt {
            Ok(v) => v,
            Err(e) => return EIO as _,
        };

        // drop_cache();
        // sync

        EOK as _
    }
    pub unsafe extern "C" fn dev_close(bdev: *mut ext4_blockdev) -> ::core::ffi::c_int {
        //fclose(dev_file);
        EOK as _
    }

    pub unsafe fn lwext4_mount(&mut self) -> Result<usize, i32> {
        let c_name = &self.name as *const _ as *const c_char;
        let c_mountpoint = &self.mount_point as *const _ as *const c_char;

        let r = ext4_device_register(self.value.as_mut(), c_name);
        if r != EOK as i32 {
            error!("ext4_device_register: rc = {:?}\n", r);
            return Err(r);
        }
        let r = ext4_mount(c_name, c_mountpoint, false);
        if r != EOK as i32 {
            error!("ext4_mount: rc = {:?}\n", r);
            return Err(r);
        }
        let r = ext4_recover(c_mountpoint);
        if (r != EOK as i32) && (r != ENOTSUP as i32) {
            error!("ext4_recover: rc = {:?}\n", r);
            return Err(r);
        }
        let r = ext4_journal_start(c_mountpoint);
        if r != EOK as i32 {
            error!("ext4_journal_start: rc = {:?}\n", r);
            return Err(r);
        }
        ext4_cache_write_back(c_mountpoint, true);
        // ext4_bcache

        info!("lwext4 mount Okay");
        Ok(0)
    }

    /// Call this when block device is being uninstalled
    pub fn lwext4_umount(&mut self) -> Result<usize, i32> {
        let c_name = &self.name as *const _ as *const c_char;
        let c_mountpoint = &self.mount_point as *const _ as *const c_char;

        unsafe {
            ext4_cache_write_back(c_mountpoint, false);

            let r = ext4_journal_stop(c_mountpoint);
            if r != EOK as i32 {
                error!("ext4_journal_stop: fail {}", r);
                return Err(r);
            }

            let r = ext4_umount(c_mountpoint);
            if r != EOK as i32 {
                error!("ext4_umount: fail {}", r);
                return Err(r);
            }

            let r = ext4_device_unregister(c_name);
            if r != EOK as i32 {
                error!("ext4_device_unregister: fail {}", r);
                return Err(r);
            }
        }

        info!("lwext4 umount Okay");
        Ok(0)
    }

    pub fn print_lwext4_mp_stats(&self) {
        //struct ext4_mount_stats stats;
        let mut stats: ext4_mount_stats = unsafe { core::mem::zeroed() };

        let c_mountpoint = &self.mount_point as *const _ as *const c_char;

        unsafe {
            ext4_mount_point_stats(c_mountpoint, &mut stats);
        }

        info!("********************");
        info!("ext4_mount_point_stats");
        info!("inodes_count = {:x?}", stats.inodes_count);
        info!("free_inodes_count = {:x?}", stats.free_inodes_count);
        info!("blocks_count = {:x?}", stats.blocks_count);
        info!("free_blocks_count = {:x?}", stats.free_blocks_count);
        info!("block_size = {:x?}", stats.block_size);
        info!("block_group_count = {:x?}", stats.block_group_count);
        info!("blocks_per_group= {:x?}", stats.blocks_per_group);
        info!("inodes_per_group = {:x?}", stats.inodes_per_group);
        info!("volume_name = {:?}", stats.volume_name);
        info!("********************\n");
    }

    pub fn print_lwext4_block_stats(&self) {
        let ext4dev = &(self.value);
        //if ext4dev.is_null { return; }

        info!("********************");
        info!("ext4 blockdev stats");
        unsafe {
            info!("bdev->bread_ctr = {:?}", (*ext4dev.bdif).bread_ctr);
            info!("bdev->bwrite_ctr = {:?}", (*ext4dev.bdif).bwrite_ctr);

            info!("bcache->ref_blocks = {:?}", (*ext4dev.bc).ref_blocks);
            info!(
                "bcache->max_ref_blocks = {:?}",
                (*ext4dev.bc).max_ref_blocks
            );
            info!("bcache->lru_ctr = {:?}", (*ext4dev.bc).lru_ctr);
        }
        info!("\n");

        info!("********************\n");
    }
}
