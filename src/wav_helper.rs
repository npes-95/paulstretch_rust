use hound;

use std::error::Error;

#[derive(Debug)]
pub enum Format {
    Float,
    Int,
}

#[derive(Debug)]
pub struct WaveHeader {
    pub channels: u16,
    pub sample_rate: u32,
    pub bit_depth: u16,
    pub format: Format,
}

#[derive(Debug)]
pub struct Wave {
    pub header: WaveHeader,
    pub data: Vec<Vec<f32>>,
}

pub fn load(path: &str) -> Result<Wave, Box<dyn Error>> {
    let mut r = hound::WavReader::open(path)?;
    let spec = r.spec();

    let header = WaveHeader {
        channels: spec.channels,
        sample_rate: spec.sample_rate,
        bit_depth: spec.bits_per_sample,
        format: match spec.sample_format {
            hound::SampleFormat::Float => Format::Float,
            hound::SampleFormat::Int => Format::Int,
        },
    };

    let interleaved_data: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|s| s.unwrap()).collect(),

        hound::SampleFormat::Int => match spec.bits_per_sample {
            8 => r
                .samples::<i8>()
                .map(|s| (s.unwrap() as f32 / i8::MAX as f32))
                .collect(),
            16 => r
                .samples::<i16>()
                .map(|s| s.unwrap() as f32 / i16::MAX as f32)
                .collect(),

            24 => r
                .samples::<i32>()
                .map(|s| s.unwrap() as f32 / 0x7FFFFF as f32)
                .collect(),

            32 => r
                .samples::<i32>()
                .map(|s| s.unwrap() as f32 / i32::MAX as f32)
                .collect(),
            _ => {
                return Err(format!("Unrecognised bit depth: got {}", spec.bits_per_sample).into())
            }
        },
    };

    Ok(Wave {
        header,
        data: uninterleave(interleaved_data, spec.channels)?,
    })
}

pub fn export(path: &str, wave: Wave) -> Result<(), Box<dyn Error>> {
    let spec = hound::WavSpec {
        channels: wave.header.channels,
        sample_rate: wave.header.sample_rate,
        bits_per_sample: wave.header.bit_depth,
        sample_format: match wave.header.format {
            Format::Float => hound::SampleFormat::Float,
            Format::Int => hound::SampleFormat::Int,
        },
    };

    let mut w = hound::WavWriter::create(path, spec)?;

    // TODO: figure out if there's a more efficient way to do this, not nice to have to match every sample
    for s in interleave(wave.data)? {
        match wave.header.format {
            Format::Float => w.write_sample(s)?,
            Format::Int => match wave.header.bit_depth {
                8 => w.write_sample((s * i8::MAX as f32) as i8)?,
                16 => w.write_sample((s * i16::MAX as f32) as i16)?,
                24 => w.write_sample((s * 0x7FFFFF as f32) as i32)?,
                32 => w.write_sample((s * i32::MAX as f32) as i32)?,
                _ => {
                    return Err(
                        format!("Unrecognised bit depth: got {}", spec.bits_per_sample).into(),
                    )
                }
            },
        };
    }

    w.finalize()?;
    Ok(())
}

fn interleave(input: Vec<Vec<f32>>) -> Result<Vec<f32>, Box<dyn Error>> {
    match input.len() {
        1 => Ok(input[0].clone()),
        2 => {
            let mut out = Vec::with_capacity(2 * input[0].len());
            for frame in input[0].iter().zip(input[1].iter()) {
                out.push(*frame.0);
                out.push(*frame.1);
            }
            Ok(out)
        }
        _ => return Err(format!("Unsupported number of channels ({})", input.len()).into()),
    }
}

fn uninterleave(input: Vec<f32>, channels: u16) -> Result<Vec<Vec<f32>>, Box<dyn Error>> {
    match channels {
        1 => Ok(vec![input]),
        2 => {
            let mut out = vec![
                Vec::with_capacity(input.len() / 2),
                Vec::with_capacity(input.len() / 2),
            ];
            for frame in input.chunks(2) {
                out[0].push(frame[0]);
                out[1].push(frame[1]);
            }
            Ok(out)
        }
        _ => return Err(format!("Unsupported number of channels ({})", input.len()).into()),
    }
}

#[cfg(test)]
mod tests {
    use super::{interleave, uninterleave};

    #[test]
    fn interleave_empty() {
        assert!(interleave(vec![]).is_err());
    }

    #[test]
    fn interleave_mono() {
        let v = vec![vec![0_f32, 1_f32, 2_f32]];
        let interleaved = interleave(v);
        assert!(interleaved.is_ok());
        assert_eq!(interleaved.unwrap(), vec![0_f32, 1_f32, 2_f32]);
    }

    #[test]
    fn interleave_stereo() {
        let v = vec![vec![0_f32, 1_f32, 2_f32], vec![3_f32, 4_f32, 5_f32]];
        let interleaved = interleave(v);
        assert!(interleaved.is_ok());
        assert_eq!(
            interleaved.unwrap(),
            vec![0_f32, 3_f32, 1_f32, 4_f32, 2_f32, 5_f32]
        );
    }

    #[test]
    fn interleave_multichannel() {
        assert!(interleave(vec![vec![], vec![], vec![]]).is_err());
    }

    #[test]
    fn uninterleave_empty() {
        assert!(uninterleave(vec![], 0).is_err());
    }

    #[test]
    fn uninterleave_mono() {
        let v = vec![0_f32, 1_f32, 2_f32];
        let uninterleaved = uninterleave(v, 1);
        assert!(uninterleaved.is_ok());
        assert_eq!(uninterleaved.unwrap(), vec![vec![0_f32, 1_f32, 2_f32]]);
    }
    #[test]
    fn uninterleave_stereo() {
        let v = vec![0_f32, 3_f32, 1_f32, 4_f32, 2_f32, 5_f32];
        let uninterleaved = uninterleave(v, 2);
        assert!(uninterleaved.is_ok());
        assert_eq!(
            uninterleaved.unwrap(),
            vec![vec![0_f32, 1_f32, 2_f32], vec![3_f32, 4_f32, 5_f32]]
        );
    }

    #[test]
    fn uninterleave_multichannel() {
        assert!(uninterleave(vec![], 3).is_err());
    }
}
