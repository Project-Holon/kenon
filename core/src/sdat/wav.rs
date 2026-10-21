//! A minimal WAV writer: 16-bit PCM in a RIFF container.
//!
//! ```text
//! "RIFF" u32 size  "WAVE"
//! "fmt " u32 16  u16 format 1  u16 channels 2  u32 rate  u32 byte rate
//!        u16 block align 4  u16 bits 16
//! "data" u32 size  samples (little-endian i16, left then right)
//! ```

/// Wraps interleaved stereo 16-bit samples in a WAV file.
///
/// An odd trailing sample (half a frame) is dropped.
pub fn to_wav(samples: &[i16], rate: u32) -> Vec<u8> {
    let samples = &samples[..samples.len() & !1];
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 4).to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_valid_header() {
        let wav = to_wav(&[1, -1, 2, -2], 32768);
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes(wav[4..8].try_into().unwrap()), 36 + 8);
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 32768);
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 8);
        assert_eq!(wav.len(), 52);
        assert_eq!(i16::from_le_bytes([wav[48], wav[49]]), 2);
    }

    #[test]
    fn drops_half_a_frame_and_allows_empty() {
        assert_eq!(to_wav(&[1, 2, 3], 8000).len(), 44 + 4);
        assert_eq!(to_wav(&[], 8000).len(), 44);
    }
}
