
use std::{
    fs::{
        read_dir,
        File
    },
    io::{
        Read,
        Seek,
        SeekFrom
    },
    path::Path
};
use super::{SECTOR_SIZE, DriveLoc};

/// Hold 1 second of data in the memory buffer.
const DISC_BUFFER_SIZE: u64 = 75 * SECTOR_SIZE;

/// This object represents the disc.
/// It manages file access and contains an
/// in-memory buffer.
pub struct Disc {
    track_files: Vec<File>,

    buffer: Vec<u8>,
    buffer_track: u8,
    buffer_file_offset: u64,
    sector_offset: u64,
}

// Constructors
impl Disc {
    /// Create a new disc from a filepath.
    /// 
    /// If the filepath points to a .bin file,
    /// it will be opened as a single-track CD.
    /// 
    /// If the filepath points to a .cue file,
    /// it will be opened as a multi-track CD.
    /// 
    /// If the filepath points to a directory,
    /// the contents will be scanned for first a .cue,
    /// then a .bin, and will be opened accordingly.
    pub fn new(path: &Path) -> std::io::Result<Self> {
        if path.is_dir() {
            let child_paths = read_dir(path)?;
            for child in child_paths {
                if let Some(ext) = child?.path().extension() {
                    if ext == "cue" {
                        return Self::new_from_cue(path);
                    }
                }
            }
            // TODO: only read directory once...
            if let Some(first_child) = read_dir(path)?.next() {
                Self::new_from_bin(&first_child?.path())
            } else {
                // TODO: error.
                panic!("no children in directory provided");
            }
        } else {
            if let Some(ext) = path.extension() {
                if ext == "cue" {
                    Self::new_from_cue(path)
                } else {
                    // TODO: only if .bin?
                    Self::new_from_bin(path)
                }
            } else {
                // TODO: error?
                Self::new_from_bin(path)
            }
        }
    }

    /// Open directly from a binary file.
    /// 
    /// This will assume a single track.
    fn new_from_bin(path: &Path) -> std::io::Result<Self> {
        let disc_file = File::open(path)?;
        Ok(Self {
            track_files: vec![disc_file],

            buffer_track: 0,
            buffer: vec![0; DISC_BUFFER_SIZE as usize],
            buffer_file_offset: u64::MAX,
            sector_offset: 0,
        })
    }

    fn new_from_cue(path: &Path) -> std::io::Result<Self> {
        unimplemented!("cue file");
    }
}

impl Disc {
    /// Read from disc file into buffer, if necessary.
    /// 
    /// It will read the sector pointed to by the offset, in addition to
    /// other nearby sectors.
    pub fn load_from_file(&mut self, track: u8, seek_offset: u64) {
        let chunk_num = seek_offset / DISC_BUFFER_SIZE;
        let target_file_offset = chunk_num * DISC_BUFFER_SIZE;
        if self.buffer_file_offset == target_file_offset && self.buffer_track == track {
            // No read necessary.
            return;
        }
        let disc_file = &mut self.track_files[track as usize];
        disc_file.seek(SeekFrom::Start(target_file_offset)).expect("could not seek in disc");
        disc_file.read(&mut self.buffer).expect("could not load disc data");
        self.buffer_track = track;
        self.buffer_file_offset = target_file_offset;
        println!("CD load from disc @ {:X}", self.buffer_file_offset);
    }

    /// Provide the offset into the file.
    pub fn set_sector_offset(&mut self, sector_offset: u64) {
        self.sector_offset = sector_offset - self.buffer_file_offset;
    }

    /// Adjust the sector offset by a relative amount.
    /// Used to skip header metadata bytes.
    pub fn adjust_sector_offset(&mut self, relative_offset: u64) {
        self.sector_offset += relative_offset;
    }

    /// Read a byte from the buffer, and increment the sector offset.
    pub fn read_byte(&mut self) -> u8 {
        let index = self.sector_offset as usize;
        self.sector_offset += 1;
        self.buffer[index]
    }

    /// After calling set_sector_offset, call this to get the sector data in a buffer.
    pub fn ref_sector_data<'a>(&'a self, relative_offset: u64, size: usize) -> &'a [u8] {
        let start = (self.sector_offset + relative_offset) as usize;
        &self.buffer[start..(start + size)]
    }

    // TODO: this is incorrect. should add together all track sizes..?
    pub fn get_end_pos(&mut self) -> DriveLoc {
        let track_file = self.track_files.last_mut().unwrap();
        let metadata = track_file.metadata().expect("could not get file metadata");
        let file_len = metadata.len();
        let sector_count = file_len / SECTOR_SIZE;
        let total_seconds = (sector_count / 75) + 2; // Round down to nearest second, and offset by 2.
        let minute = (total_seconds / 60) as u8;
        let second = (total_seconds % 60) as u8;
        DriveLoc { minute, second, sector: 0 }
    }
}