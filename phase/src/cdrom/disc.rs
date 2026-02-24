
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

use super::{
    SECTOR_SIZE, DriveLoc,
    cue::*
};

/// Hold 1 second of data in the memory buffer.
const DISC_BUFFER_SIZE: u64 = 75 * SECTOR_SIZE;

struct Track {
    num:        u8,
    file:       File,
    indices:    Vec<DriveLoc>,
    start_pos:  DriveLoc,
    end_pos:    DriveLoc,
}

/// This object represents the disc.
/// It manages file access and contains an
/// in-memory buffer.
pub struct Disc {
    tracks: Vec<Track>,
    current_track: u8,

    buffer: Vec<u8>,
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
                let child_path = child?.path();
                if let Some(ext) = child_path.extension().and_then(|e| e.to_str()) {
                    if ext == "cue" {
                        return Self::new_from_cue(&child_path);
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
        let start_pos = DriveLoc {
            minute: 0x00,
            second: 0x00,
            sector: 0x00,
        };
        let end_pos = get_file_size(&disc_file).add(&DriveLoc { minute: 0, second: 2, sector: 0 });
        let track = Track {
            num: 0x01,
            file: disc_file,
            indices: vec![start_pos],
            start_pos,
            end_pos
        };
        Ok(Self {
            tracks: vec![track],
            current_track: 0,

            buffer: vec![0; DISC_BUFFER_SIZE as usize],
            buffer_file_offset: u64::MAX,
            sector_offset: 0,
        })
    }

    fn new_from_cue(path: &Path) -> std::io::Result<Self> {
        let cue_file_str = std::fs::read_to_string(path)?;
        let folder_path = path.parent().unwrap();
        let cue_file = CueFile::parse_from_str(&cue_file_str).expect("invalid cue file");
        let mut tracks = Vec::new();
        let mut current_pos = DriveLoc {
            minute: 0x00,
            second: 0x00,
            sector: 0x00,
        };
        for (i, track) in cue_file.tracks.iter().enumerate() {
            let path = folder_path.join(&track.file_name);
            let file = File::open(path)?;
            let start_pos = current_pos;
            let file_size = get_file_size(&file);
            let end_pos = start_pos.add(&DriveLoc { minute: 0, second: 2, sector: 0 }).add(&file_size);
            tracks.push(Track {
                num: track.num as u8,
                file,
                indices: track.indices.iter().map(|i| i.start).collect(),
                start_pos,
                end_pos
            });
            current_pos = end_pos;
            println!("Track {}: {} => {}", i + 1, start_pos, end_pos);
        }
        Ok(Self {
            tracks,
            current_track: 0,

            buffer: vec![0; DISC_BUFFER_SIZE as usize],
            buffer_file_offset: u64::MAX,
            sector_offset: 0,
        })
    }
}

impl Disc {
    /// Read from disc file into buffer, if necessary.
    /// 
    /// It will read the sector pointed to by the offset, in addition to
    /// other nearby sectors.
    pub fn load_from_file(&mut self, seek_loc: &DriveLoc) {
        let (track, track_pos) = self.calculate_track(seek_loc);
        if track_pos.in_pre_gap() {
            println!("Loading track {} | pos: {} | pre-gap", track, track_pos);
            self.buffer.fill(0);
            return;
        }
        const PRE_GAP_SIZE: u64 = SECTOR_SIZE * 75 * 2;
        let seek_offset = track_pos.byte_offset() - PRE_GAP_SIZE;
        println!("Loading track {} | pos: {} | offset: {:X}", track, track_pos, seek_offset);
        let chunk_num = seek_offset / DISC_BUFFER_SIZE;
        let target_file_offset = chunk_num * DISC_BUFFER_SIZE;
        if self.buffer_file_offset == target_file_offset && self.current_track == track {
            // No read necessary.
            self.sector_offset = seek_offset - self.buffer_file_offset;
            return;
        }
        let track_idx = (track - 1) as usize;
        let disc_file = &mut self.tracks[track_idx].file;
        disc_file.seek(SeekFrom::Start(target_file_offset)).expect("could not seek in disc");
        disc_file.read(&mut self.buffer).expect("could not load disc data");
        self.current_track = track;
        self.buffer_file_offset = target_file_offset;
        self.sector_offset = seek_offset - self.buffer_file_offset;
        println!("CD load from disc @ {:X}", self.buffer_file_offset);
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

    pub fn get_track_count(&self) -> u8 {
        self.tracks.len() as u8
    }

    /// Get track start pos.
    /// This is taken from index 01 (i.e. after the pre-gap)
    pub fn get_track_start_pos(&self, track: u8) -> DriveLoc {
        let track_idx = (track - 1) as usize;
        let index_1_pos = self.tracks[track_idx].start_pos.add(&DriveLoc { minute: 0, second: 2, sector: 0 });
        DriveLoc { minute: index_1_pos.minute, second: index_1_pos.second, sector: 0 }
    }

    pub fn get_track_end_pos(&self, track: u8) -> DriveLoc {
        let track_idx = (track - 1) as usize;
        self.tracks[track_idx].end_pos
    }

    pub fn get_current_track_end_pos(&self) -> DriveLoc {
        self.get_track_end_pos(self.current_track)
    }

    /// Calculate the track based on the drive location.
    /// Also returns the relative position in the track.
    pub fn calculate_track(&self, pos: &DriveLoc) -> (u8, DriveLoc) {
        // Assuming tracks are in chronological order...
        for track in self.tracks.iter().rev() {
            if let Some(pos) = pos.relative_to(&track.start_pos) {
                return (track.num, pos);
            }
        }
        // TODO: handle better?
        panic!("could not find valid track for requested pos");
    }
}

fn get_file_size(file: &File) -> DriveLoc {
    let metadata = file.metadata().expect("could not get file metadata");
    let file_len = metadata.len();
    let sector_count = file_len / SECTOR_SIZE;
    let total_seconds = sector_count / 75;
    let minute = (total_seconds / 60) as u8;
    let second = (total_seconds % 60) as u8;
    let sector = (sector_count % 75) as u8;
    DriveLoc { minute, second, sector }
}