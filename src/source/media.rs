use std::io::{BufReader, Read, Seek, SeekFrom};

use sndfile_sys as sf;

pub struct Media<R> {
    info: sf::SF_INFO,
    _user: Box<BufReader<R>>,
    file: *mut sf::SNDFILE,
}

// libsndfile isn't doing anything sneaky
unsafe impl<R> Send for Media<R> where R: Send {}

impl<R> Media<R>
where
    R: Read + Seek,
{
    pub fn new(data: R) -> anyhow::Result<Self> {
        let mut fileio = sf::SF_VIRTUAL_IO {
            get_filelen: Self::vio_get_filelen,
            seek: Self::vio_seek,
            read: Self::vio_read,
            write: Self::vio_write,
            tell: Self::vio_tell,
        };
        let mut info = sf::SF_INFO {
            frames: 0,
            samplerate: 0,
            channels: 0,
            format: 0,
            sections: 0,
            seekable: 0,
        };
        let mut user = Box::new(BufReader::new(data));
        let userptr = (user.as_mut() as *mut _) as *mut libc::c_void;
        let file = unsafe { sf::sf_open_virtual(&mut fileio, sf::SFM_READ, &mut info, userptr) };
        if file.is_null() {
            anyhow::bail!("could not open media file");
        }
        Ok(Self {
            info,
            _user: user,
            file,
        })
    }

    extern "C" fn vio_get_filelen(user: *mut libc::c_void) -> sf::sf_count_t {
        unsafe {
            let user = (user as *mut BufReader<R>).as_mut().unwrap();
            if let Ok(cur) = user.seek(SeekFrom::Current(0)) {
                if let Ok(end) = user.seek(SeekFrom::End(0)) {
                    if let Ok(_) = user.seek(SeekFrom::Start(cur)) {
                        return end as sf::sf_count_t;
                    }
                }
            }
            -1
        }
    }

    extern "C" fn vio_seek(
        offset: sf::sf_count_t,
        whence: libc::c_int,
        user: *mut libc::c_void,
    ) -> sf::sf_count_t {
        unsafe {
            let user = (user as *mut BufReader<R>).as_mut().unwrap();
            (match whence {
                sf::SF_SEEK_CUR => user.seek(SeekFrom::Current(offset as i64)),
                sf::SF_SEEK_END => user.seek(SeekFrom::End(offset as i64)),
                sf::SF_SEEK_SET => user.seek(SeekFrom::Start(offset as u64)),
                _ => panic!("bad seek whence"),
            })
            .map(|v| v as sf::sf_count_t)
            .unwrap_or(-1)
        }
    }

    extern "C" fn vio_read(
        ptr: *mut libc::c_void,
        count: sf::sf_count_t,
        user: *mut libc::c_void,
    ) -> sf::sf_count_t {
        unsafe {
            let user = (user as *mut BufReader<R>).as_mut().unwrap();
            let buf = std::slice::from_raw_parts_mut(ptr as *mut u8, count as usize);
            user.read(buf).unwrap_or(0) as sf::sf_count_t
        }
    }

    extern "C" fn vio_write(
        _ptr: *const libc::c_void,
        _count: sf::sf_count_t,
        _user_data: *mut libc::c_void,
    ) -> sf::sf_count_t {
        panic!("libsndfile vio write");
    }

    extern "C" fn vio_tell(user: *mut libc::c_void) -> sf::sf_count_t {
        unsafe {
            let user = (user as *mut BufReader<R>).as_mut().unwrap();
            user.seek(SeekFrom::Current(0))
                .map(|v| v as sf::sf_count_t)
                .unwrap_or(-1)
        }
    }
}

impl<R> Drop for Media<R> {
    fn drop(&mut self) {
        unsafe {
            sf::sf_close(self.file);
        }
    }
}

impl<R> super::Source for Media<R>
where
    R: std::io::Read + std::io::Seek + std::marker::Send + 'static,
{
    fn samplerate(&self) -> f32 {
        self.info.samplerate as f32
    }

    fn channels(&self) -> u16 {
        self.info.channels as u16
    }

    fn len(&self) -> Option<u64> {
        Some(self.info.frames as u64)
    }

    fn fill(&mut self, buffer: &mut [f32]) -> usize {
        let ourlen = (buffer.len() / self.info.channels as usize) * self.info.channels as usize;
        let amt =
            unsafe { sf::sf_read_float(self.file, buffer.as_mut_ptr(), ourlen as sf::sf_count_t) };
        if amt < 0 {
            0
        } else {
            amt as usize
        }
    }

    fn seek(&mut self, frame: u64) -> anyhow::Result<()> {
        if self.info.seekable > 0 {
            unsafe {
                let result = sf::sf_seek(self.file, frame as sf::sf_count_t, sf::SF_SEEK_SET);
                if result < 0 {
                    anyhow::bail!("failed to seek media stream");
                }
                Ok(())
            }
        } else {
            anyhow::bail!("cannot seek media stream")
        }
    }
}
